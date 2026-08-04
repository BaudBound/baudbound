use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

use serde_json::{Value, json};

use baudbound_runtime::RuntimeActionError;

use super::execute_with_handler;
use crate::{ActionLimits, ActionSecurityPolicy, HeadlessActionHandler};

#[test]
fn http_results_follow_the_shared_action_contract() {
    let server = LoopbackHttpServer::start(
        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}".to_vec(),
        Duration::ZERO,
    );
    let result = execute_http(json!({
        "method": "GET",
        "url": server.url("/contract"),
        "timeoutSeconds": 2
    }))
    .expect("contract response should succeed");
    server.join();

    let contract: Value = serde_json::from_str(include_str!(
        "../../../../contracts/network-action-conformance.json"
    ))
    .expect("network action contract should parse");
    let required_fields = contract["success"]["required_fields"]
        .as_object()
        .expect("success field contract should be an object");
    for (field, expected_type) in required_fields {
        let value = result
            .output_data
            .get(field)
            .unwrap_or_else(|| panic!("HTTP output is missing {field}"));
        let matches = match expected_type.as_str().unwrap() {
            "number" => value.is_number(),
            "object" => value.is_object(),
            "string" => value.is_string(),
            other => panic!("unsupported HTTP output contract type {other}"),
        };
        assert!(matches, "HTTP output {field} has the wrong type: {value}");
    }
    assert_eq!(result.output_data["json"], json!({"ok": true}));
}

#[test]
fn supports_every_editor_http_method_and_body_policy() {
    for method in ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
        let response = if method == "HEAD" {
            b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
        } else {
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_vec()
        };
        let server = LoopbackHttpServer::start(response, Duration::ZERO);
        let result = execute_http(json!({
            "method": method,
            "url": server.url("/method"),
            "headers": {"X-Test": "method"},
            "timeoutSeconds": 2,
            "body": "request-body"
        }))
        .unwrap_or_else(|error| panic!("{method} request should succeed: {error}"));
        let request = server.join();

        assert!(request.starts_with(&format!("{method} /method HTTP/1.1")));
        assert!(request.to_ascii_lowercase().contains("x-test: method"));
        let should_have_body = !matches!(method, "GET" | "HEAD");
        assert_eq!(
            request.ends_with("request-body"),
            should_have_body,
            "unexpected body policy for {method}"
        );
        assert_eq!(
            result.output_data.get("status_code"),
            Some(&json!(if method == "HEAD" { 204 } else { 200 }))
        );
    }
}

#[test]
fn preserves_large_response_bodies_without_losing_output_metadata() {
    let body = "x".repeat(2 * 1024 * 1024);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nX-Large: yes\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes();
    let server = LoopbackHttpServer::start(response, Duration::ZERO);

    let result = execute_http(json!({
        "method": "GET",
        "url": server.url("/large"),
        "timeoutSeconds": 5
    }))
    .expect("large response should succeed");
    server.join();

    assert_eq!(
        result
            .output_data
            .get("body")
            .and_then(Value::as_str)
            .map(str::len),
        Some(body.len())
    );
    assert_eq!(result.output_data["headers"]["x-large"], json!("yes"));
    assert!(result.output_data["duration_ms"].as_u64().is_some());
    assert!(!result.output_data.contains_key("json"));
}

#[test]
fn rejects_response_bodies_that_exceed_the_configured_limit() {
    let server = LoopbackHttpServer::start(
        b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\ntoo-large".to_vec(),
        Duration::ZERO,
    );
    let handler = loopback_http_handler().with_limits(ActionLimits {
        max_http_response_bytes: baudbound_runtime::ResourceLimit::limited(4),
        ..ActionLimits::default()
    });

    let error = execute_with_handler(
        &handler,
        "action.http",
        json!({
            "method": "GET",
            "url": server.url("/bounded"),
            "timeoutSeconds": 2
        }),
        Value::Null,
    )
    .expect_err("oversized response body must fail");
    server.join();

    assert!(error.to_string().contains("configured limit of 4 bytes"));
    assert_http_error(&error, "RESPONSE_BODY_LIMIT", false);
}

#[test]
fn enforces_request_timeout() {
    let server = LoopbackHttpServer::start(
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_vec(),
        Duration::from_millis(150),
    );

    let error = execute_http(json!({
        "method": "GET",
        "url": server.url("/slow"),
        "timeoutSeconds": 0.02
    }))
    .expect_err("slow response must time out");
    server.join();

    assert!(error.to_string().contains("HTTP request GET"));
    assert_http_error(&error, "HTTP_TIMEOUT", true);
}

#[test]
fn rejects_invalid_http_configuration_and_connection_failures() {
    let invalid_configs = [
        json!({"method": "BAD METHOD", "url": "http://127.0.0.1", "timeoutSeconds": 1}),
        json!({"method": "GET", "url": "not a URL", "timeoutSeconds": 1}),
        json!({"method": "GET", "url": "http://127.0.0.1", "timeoutSeconds": 0}),
        json!({"method": "GET", "url": "http://127.0.0.1", "timeoutSeconds": "NaN"}),
        json!({"method": "GET", "url": "http://127.0.0.1", "timeoutSeconds": 1e308}),
        json!({"method": "GET", "url": "http://127.0.0.1", "timeoutSeconds": 1, "headers": "invalid"}),
        json!({"method": "GET", "url": "http://127.0.0.1", "timeoutSeconds": 1, "headers": {"bad header": "value"}}),
        json!({"method": "GET", "url": "http://127.0.0.1", "timeoutSeconds": 1, "headers": {"X-Test": "line\nbreak"}}),
    ];

    for config in invalid_configs {
        let error = execute_http(config).expect_err("invalid HTTP config must fail");
        assert!(!error.to_string().trim().is_empty());
    }

    let listener = TcpListener::bind("127.0.0.1:0").expect("test port should bind");
    let address = listener.local_addr().expect("test address should resolve");
    drop(listener);
    let error = execute_http(json!({
        "method": "GET",
        "url": format!("http://{address}/closed?token=private-value"),
        "timeoutSeconds": 1
    }))
    .expect_err("connection failure must be surfaced");
    assert!(error.to_string().contains("HTTP request GET"));
    assert!(!error.to_string().contains("private-value"));
    assert!(error.to_string().contains("token=[REDACTED]"));
}

#[test]
fn refuses_to_send_malformed_json_request_bodies() {
    let error = execute_http(json!({
        "method": "POST",
        "url": "http://127.0.0.1:1/not-reached",
        "headers": {"Content-Type": "application/json"},
        "timeoutSeconds": 1,
        "body": "{\"data\":\"scanner\r\"}"
    }))
    .expect_err("raw control characters must not be sent as JSON");

    assert!(
        error
            .to_string()
            .contains("HTTP request body is not valid JSON")
    );
}

#[test]
fn blocks_private_http_destinations_by_default() {
    for url in [
        "http://127.0.0.1:9/blocked",
        "http://[::1]:9/blocked",
        "http://[::ffff:127.0.0.1]:9/blocked",
        "http://169.254.169.254/latest/meta-data",
    ] {
        let error = execute_with_handler(
            &HeadlessActionHandler::default(),
            "action.http",
            json!({
                "method": "GET",
                "url": url,
                "timeoutSeconds": 2
            }),
            Value::Null,
        )
        .expect_err("private and mapped HTTP destinations should be blocked");

        assert!(
            error
                .to_string()
                .contains("allow_private_http_requests is false"),
            "unexpected error for {url}: {error}"
        );
        assert_http_error(&error, "PRIVATE_ADDRESS_BLOCKED", false);
    }
}

#[test]
fn rejects_url_credentials_before_connecting() {
    let error = execute_http(json!({
        "method": "GET",
        "url": "http://user:password@127.0.0.1:9/not-reached",
        "timeoutSeconds": 1
    }))
    .expect_err("URL credentials must be rejected");

    assert!(
        error
            .to_string()
            .contains("URL credentials are not allowed")
    );
    assert_http_error(&error, "URL_CREDENTIALS_BLOCKED", false);
    assert!(!error.to_string().contains("password"));
}

#[test]
fn cross_origin_redirect_drops_caller_headers_and_user_agent() {
    let destination = LoopbackHttpServer::start(
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_vec(),
        Duration::ZERO,
    );
    let redirect = LoopbackHttpServer::start(
        format!(
            "HTTP/1.1 302 Found\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            destination.url("/destination")
        )
        .into_bytes(),
        Duration::ZERO,
    );

    execute_http(json!({
        "method": "POST",
        "url": redirect.url("/source"),
        "headers": {
            "Authorization": "Bearer redirect-secret",
            "Cookie": "session=redirect-secret",
            "X-Private": "redirect-secret"
        },
        "userAgent": "redirect-secret-agent",
        "body": "request-body",
        "timeoutSeconds": 2
    }))
    .expect("302 redirect should switch to a header-free GET");

    let source_request = redirect.join();
    let destination_request = destination.join();
    assert!(source_request.contains("redirect-secret"));
    assert!(destination_request.starts_with("GET /destination HTTP/1.1"));
    assert!(!destination_request.contains("redirect-secret"));
    assert!(!destination_request.ends_with("request-body"));
}

#[test]
fn cross_origin_redirect_refuses_to_forward_a_request_body() {
    let unused_destination = TcpListener::bind("127.0.0.1:0").expect("test port should bind");
    let destination = unused_destination
        .local_addr()
        .expect("test address should resolve");
    drop(unused_destination);
    let redirect = LoopbackHttpServer::start(
        format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{destination}/destination\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
        .into_bytes(),
        Duration::ZERO,
    );

    let error = execute_http(json!({
        "method": "POST",
        "url": redirect.url("/source"),
        "headers": {"Authorization": "Bearer redirect-secret"},
        "body": "request-body",
        "timeoutSeconds": 2
    }))
    .expect_err("307 redirect must not forward a body across origins");
    redirect.join();

    assert!(
        error
            .to_string()
            .contains("refused to forward a request body")
    );
    assert!(!error.to_string().contains("redirect-secret"));
}

#[test]
fn same_origin_redirect_preserves_headers_method_and_body() {
    let server = LoopbackHttpSequenceServer::start(2, |url| {
        vec![
            format!(
                "HTTP/1.1 307 Temporary Redirect\r\nLocation: {url}/destination\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .into_bytes(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_vec(),
        ]
    });

    execute_http(json!({
        "method": "POST",
        "url": server.url("/source"),
        "headers": {"Authorization": "Bearer same-origin", "X-Request": "preserved"},
        "body": "request-body",
        "timeoutSeconds": 2
    }))
    .expect("same-origin 307 redirect should preserve the request");

    let requests = server.join();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].starts_with("POST /destination HTTP/1.1"));
    assert!(requests[1].contains("authorization: Bearer same-origin"));
    assert!(requests[1].contains("x-request: preserved"));
    assert!(requests[1].ends_with("request-body"));
}

#[test]
fn redirect_loop_is_rejected_before_repeating_the_request() {
    let server = LoopbackHttpServer::start(
        b"HTTP/1.1 302 Found\r\nLocation: /loop\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_vec(),
        Duration::ZERO,
    );

    let error = execute_http(json!({
        "method": "GET",
        "url": server.url("/loop"),
        "timeoutSeconds": 2
    }))
    .expect_err("redirect loops must fail before a repeated request");
    server.join();

    assert!(error.to_string().contains("redirect loop"));
}

fn execute_http(
    config: Value,
) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
    execute_with_handler(&loopback_http_handler(), "action.http", config, Value::Null)
}

fn assert_http_error(error: &RuntimeActionError, code: &str, retryable: bool) {
    let RuntimeActionError::StructuredFailure { failure, .. } = error else {
        panic!("expected structured HTTP error, found {error}");
    };
    assert_eq!(failure.code(), code);
    assert_eq!(failure.error_type(), "http");
    assert_eq!(failure.retryable(), retryable);
}

fn loopback_http_handler() -> HeadlessActionHandler {
    HeadlessActionHandler::default().with_security_policy(ActionSecurityPolicy {
        allow_private_http_requests: true,
        ..ActionSecurityPolicy::default()
    })
}

struct LoopbackHttpServer {
    join_handle: thread::JoinHandle<String>,
    url: String,
}

impl LoopbackHttpServer {
    fn start(response: Vec<u8>, response_delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let address = listener.local_addr().expect("test address should resolve");
        let join_handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test server should accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("read timeout should configure");
            let request = read_http_request(&mut stream);
            thread::sleep(response_delay);
            let _ = stream.write_all(&response);
            request
        });

        Self {
            join_handle,
            url: format!("http://{address}"),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.url, path)
    }

    fn join(self) -> String {
        self.join_handle
            .join()
            .expect("test server thread should finish")
    }
}

struct LoopbackHttpSequenceServer {
    join_handle: thread::JoinHandle<Vec<String>>,
    url: String,
}

impl LoopbackHttpSequenceServer {
    fn start(count: usize, responses: impl FnOnce(&str) -> Vec<Vec<u8>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let address = listener.local_addr().expect("test address should resolve");
        let url = format!("http://{address}");
        let responses = responses(&url);
        assert_eq!(
            responses.len(),
            count,
            "every accepted request needs a response"
        );
        let join_handle = thread::spawn(move || {
            responses
                .into_iter()
                .map(|response| {
                    let (mut stream, _) = listener.accept().expect("test server should accept");
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .expect("read timeout should configure");
                    let request = read_http_request(&mut stream);
                    stream
                        .write_all(&response)
                        .expect("test response should write");
                    request
                })
                .collect()
        });

        Self { join_handle, url }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.url, path)
    }

    fn join(self) -> Vec<String> {
        self.join_handle
            .join()
            .expect("test server thread should finish")
    }
}

fn read_http_request(stream: &mut impl Read) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = stream.read(&mut buffer).unwrap_or_default();
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
        if request_is_complete(&request) {
            break;
        }
    }
    String::from_utf8_lossy(&request).into_owned()
}

fn request_is_complete(request: &[u8]) -> bool {
    let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let header_length = header_end + 4;
    let headers = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
    let content_length = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    request.len() >= header_length + content_length
}
