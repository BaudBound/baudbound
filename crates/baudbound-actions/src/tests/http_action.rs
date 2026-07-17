use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

use serde_json::{Value, json};

use super::{execute, execute_with_handler};
use crate::{ActionLimits, HeadlessActionHandler};

#[test]
fn supports_every_editor_http_method_and_body_policy() {
    for method in ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
        let response = if method == "HEAD" {
            b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
        } else {
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_vec()
        };
        let server = LoopbackHttpServer::start(response, Duration::ZERO);
        let result = execute(
            "action.http",
            json!({
                "method": method,
                "url": server.url("/method"),
                "headers": {"X-Test": "method"},
                "timeoutSeconds": 2,
                "body": "request-body"
            }),
        )
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

    let result = execute(
        "action.http",
        json!({
            "method": "GET",
            "url": server.url("/large"),
            "timeoutSeconds": 5
        }),
    )
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
    let handler = HeadlessActionHandler::default().with_limits(ActionLimits {
        max_http_response_bytes: 4,
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
}

#[test]
fn enforces_request_timeout() {
    let server = LoopbackHttpServer::start(
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_vec(),
        Duration::from_millis(150),
    );

    let error = execute(
        "action.http",
        json!({
            "method": "GET",
            "url": server.url("/slow"),
            "timeoutSeconds": 0.02
        }),
    )
    .expect_err("slow response must time out");
    server.join();

    assert!(error.to_string().contains("HTTP request GET"));
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
        let error = execute("action.http", config).expect_err("invalid HTTP config must fail");
        assert!(!error.to_string().trim().is_empty());
    }

    let listener = TcpListener::bind("127.0.0.1:0").expect("test port should bind");
    let address = listener.local_addr().expect("test address should resolve");
    drop(listener);
    let error = execute(
        "action.http",
        json!({
            "method": "GET",
            "url": format!("http://{address}/closed"),
            "timeoutSeconds": 1
        }),
    )
    .expect_err("connection failure must be surfaced");
    assert!(error.to_string().contains("HTTP request GET"));
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
