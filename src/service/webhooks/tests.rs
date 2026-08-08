use std::{
    collections::BTreeSet,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    sync::{
        Arc, Barrier,
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

use baudbound_core::{RunReport, TriggerEvent, TriggerRegistration};
use baudbound_runtime::{ResourceLimit, RunIdentity};
use baudbound_triggers::NetworkTriggerAuthenticationError;
use serde_json::{Value, json};

use super::*;
use crate::service::{executor::TriggerRunner, ipc::ServiceControlServer};

struct AllowAllAuthenticator;

#[test]
fn unlimited_webhook_header_limit_removes_the_library_default_cap() {
    assert_eq!(
        listener::http1_header_buffer_size(ResourceLimit::Unlimited)
            .expect("unlimited header setting should be supported"),
        usize::MAX
    );
    assert!(listener::http1_header_buffer_size(ResourceLimit::limited(8 * 1024 - 1)).is_err());
}

impl NetworkTriggerAuthenticator for AllowAllAuthenticator {
    fn authenticate(
        &self,
        _script_id: &str,
        _node_id: &str,
        _trigger_kind: NetworkTriggerKind,
        _provided_token: Option<&str>,
    ) -> Result<(), NetworkTriggerAuthenticationError> {
        Ok(())
    }
}

struct ExpectedTokenAuthenticator(&'static str);

struct SlowAuthenticator(Duration);

impl NetworkTriggerAuthenticator for SlowAuthenticator {
    fn authenticate(
        &self,
        _script_id: &str,
        _node_id: &str,
        _trigger_kind: NetworkTriggerKind,
        _provided_token: Option<&str>,
    ) -> Result<(), NetworkTriggerAuthenticationError> {
        thread::sleep(self.0);
        Ok(())
    }
}

impl NetworkTriggerAuthenticator for ExpectedTokenAuthenticator {
    fn authenticate(
        &self,
        _script_id: &str,
        _node_id: &str,
        _trigger_kind: NetworkTriggerKind,
        provided_token: Option<&str>,
    ) -> Result<(), NetworkTriggerAuthenticationError> {
        match provided_token {
            None => Err(NetworkTriggerAuthenticationError::MissingToken),
            Some(token) if token == self.0 => Ok(()),
            Some(_) => Err(NetworkTriggerAuthenticationError::InvalidToken),
        }
    }
}

#[test]
fn immediate_webhook_response_does_not_wait_for_execution() {
    let release = Arc::new(Barrier::new(2));
    let runner = {
        let release = Arc::clone(&release);
        Arc::new(move |event: TriggerEvent| {
            release.wait();
            Ok(report(&event, Default::default()))
        }) as Arc<TriggerRunner>
    };
    let mut host = test_host(webhook_service(false, 1.0), runner);
    let response = send_request(&host, "POST", "/events/test", "{}");
    accept_next(&mut host);

    let response = response
        .recv_timeout(Duration::from_secs(1))
        .expect("immediate webhook response should not wait for execution");
    assert!(response.starts_with("HTTP/1.1 202"), "{response}");
    assert!(response.ends_with("fallback"), "{response}");

    release.wait();
    let mut status = status_tracker();
    wait_for_host_completion(&mut host, &mut status);
}

#[test]
fn waiting_webhook_returns_response_node_result_before_deadline() {
    let runner = Arc::new(|event: TriggerEvent| {
        Ok(report(
            &event,
            [
                ("n-response.sent".to_owned(), Value::Bool(true)),
                ("n-response.status_code".to_owned(), json!(201)),
                (
                    "n-response.content_type".to_owned(),
                    Value::String("application/json".to_owned()),
                ),
                (
                    "n-response.body".to_owned(),
                    Value::String(r#"{"created":true}"#.to_owned()),
                ),
                (
                    "n-response.headers".to_owned(),
                    json!({ "x-baudbound-test": "present" }),
                ),
                (
                    "n-response.trigger_id".to_owned(),
                    Value::String("n-webhook".to_owned()),
                ),
            ]
            .into_iter()
            .collect(),
        ))
    }) as Arc<TriggerRunner>;
    let mut host = test_host(webhook_service(true, 1.0), runner);
    let response = send_request(&host, "POST", "/events/test", "{}");
    accept_next(&mut host);

    let mut status = status_tracker();
    wait_for_host_completion(&mut host, &mut status);
    let response = response
        .recv_timeout(Duration::from_secs(1))
        .expect("response-node result should reach client");
    assert!(response.starts_with("HTTP/1.1 201"), "{response}");
    assert!(
        response
            .to_ascii_lowercase()
            .contains("x-baudbound-test: present"),
        "{response}"
    );
    assert!(response.ends_with(r#"{"created":true}"#), "{response}");
}

#[test]
fn waiting_webhook_uses_fallback_at_deadline_while_execution_continues() {
    let release = Arc::new(Barrier::new(2));
    let runner = {
        let release = Arc::clone(&release);
        Arc::new(move |event: TriggerEvent| {
            release.wait();
            Ok(report(&event, Default::default()))
        }) as Arc<TriggerRunner>
    };
    let mut host = test_host(webhook_service(true, 0.05), runner);
    let response = send_request(&host, "POST", "/events/test", "{}");
    accept_next(&mut host);

    thread::sleep(Duration::from_millis(70));
    host.expire_pending();
    let response = response
        .recv_timeout(Duration::from_secs(1))
        .expect("fallback should be returned at the configured deadline");
    assert!(response.starts_with("HTTP/1.1 202"), "{response}");
    assert!(response.ends_with("fallback"), "{response}");

    release.wait();
    let mut status = status_tracker();
    wait_for_host_completion(&mut host, &mut status);
}

#[test]
fn route_reload_preserves_in_flight_execution_and_accepts_new_routes() {
    let release = Arc::new(Barrier::new(2));
    let runner = {
        let release = Arc::clone(&release);
        Arc::new(move |event: TriggerEvent| {
            if event.node_id == "n-old" {
                release.wait();
            }
            Ok(report(&event, Default::default()))
        }) as Arc<TriggerRunner>
    };
    let mut host = test_host(webhook_service_for("old", "n-old", true, 1.0), runner);

    let old_response = send_request(&host, "POST", "/events/old", "{}");
    accept_next(&mut host);

    let new_service = webhook_service_for("new", "n-new", false, 1.0);
    host.listener.replace_admission(
        new_service.clone(),
        Arc::new(AllowAllAuthenticator),
        BTreeSet::new(),
    );
    host.service = new_service;
    let new_response = send_request(&host, "POST", "/events/new", "{}");
    accept_next(&mut host);

    let new_response = new_response
        .recv_timeout(Duration::from_secs(1))
        .expect("reloaded route should respond while the old route is still running");
    assert!(new_response.starts_with("HTTP/1.1 202"), "{new_response}");

    release.wait();
    let mut status = status_tracker();
    wait_for_host_completion(&mut host, &mut status);
    let old_response = old_response
        .recv_timeout(Duration::from_secs(1))
        .expect("in-flight request should survive route reload");
    assert!(old_response.starts_with("HTTP/1.1 202"), "{old_response}");
}

#[test]
fn http_bridge_rejects_oversized_bodies_and_wrong_methods_before_dispatch() {
    let runner = Arc::new(
        |event: TriggerEvent| -> Result<baudbound_core::TriggerActivation, String> {
            panic!("rejected request unexpectedly dispatched event {event:?}")
        },
    ) as Arc<TriggerRunner>;
    let mut host = test_host_with_limits(webhook_service(false, 1.0), runner, 8, 4);

    let oversized_response = send_request(&host, "POST", "/events/test", "12345");
    let oversized_response = oversized_response
        .recv_timeout(Duration::from_secs(1))
        .expect("oversized request should receive a response");
    assert!(
        oversized_response.starts_with("HTTP/1.1 413"),
        "{oversized_response}"
    );

    let wrong_method_response = send_request(&host, "GET", "/events/test", "");
    accept_next(&mut host);
    let wrong_method_response = wrong_method_response
        .recv_timeout(Duration::from_secs(1))
        .expect("wrong-method request should receive a response");
    assert!(
        wrong_method_response.starts_with("HTTP/1.1 404"),
        "{wrong_method_response}"
    );
}

#[test]
fn webhook_authentication_and_browser_origin_checks_happen_before_dispatch() {
    let (event_sender, event_receiver) = mpsc::channel();
    let runner = Arc::new(move |event: TriggerEvent| {
        event_sender
            .send(event.clone())
            .expect("captured event should send");
        Ok(report(&event, Default::default()))
    }) as Arc<TriggerRunner>;
    let mut host = test_host(webhook_service(false, 1.0), runner);
    let authenticator: Arc<dyn NetworkTriggerAuthenticator> =
        Arc::new(ExpectedTokenAuthenticator("correct-token"));
    let allow_browser_origins = BTreeSet::from(["https://allowed.example".to_owned()]);
    host.listener
        .replace_admission(host.service.clone(), authenticator, allow_browser_origins);

    let missing = send_request(&host, "POST", "/events/test", "{}");
    accept_next(&mut host);
    assert!(
        missing
            .recv_timeout(Duration::from_secs(1))
            .expect("missing-token response should arrive")
            .starts_with("HTTP/1.1 401")
    );

    let wrong = send_request_with_headers(
        &host,
        "POST",
        "/events/test",
        "{}",
        &[("Authorization", "Bearer wrong-token")],
    );
    accept_next(&mut host);
    assert!(
        wrong
            .recv_timeout(Duration::from_secs(1))
            .expect("wrong-token response should arrive")
            .starts_with("HTTP/1.1 403")
    );

    for headers in [
        vec![("Authorization", "Basic correct-token")],
        vec![("X-BaudBound-Token", "correct-token")],
        vec![
            ("Authorization", "Bearer correct-token"),
            ("Authorization", "Bearer correct-token"),
        ],
    ] {
        let malformed = send_request_with_headers(&host, "POST", "/events/test", "{}", &headers);
        accept_next(&mut host);
        assert!(
            malformed
                .recv_timeout(Duration::from_secs(1))
                .expect("malformed authorization response should arrive")
                .starts_with("HTTP/1.1 401")
        );
    }

    let preflight = send_request_with_headers(
        &host,
        "OPTIONS",
        "/events/test",
        "",
        &[
            ("Origin", "https://allowed.example"),
            ("Access-Control-Request-Method", "POST"),
            (
                "Access-Control-Request-Headers",
                "Content-Type, Authorization",
            ),
        ],
    );
    accept_next(&mut host);
    let preflight = preflight
        .recv_timeout(Duration::from_secs(1))
        .expect("preflight response should arrive");
    assert!(preflight.starts_with("HTTP/1.1 204"), "{preflight}");
    assert!(
        preflight
            .to_ascii_lowercase()
            .contains("access-control-allow-origin: https://allowed.example"),
        "{preflight}"
    );

    let accepted = send_request_with_headers(
        &host,
        "POST",
        "/events/test",
        "{}",
        &[
            ("Authorization", "bEaReR correct-token"),
            ("Origin", "https://allowed.example"),
        ],
    );
    accept_next(&mut host);
    let accepted = accepted
        .recv_timeout(Duration::from_secs(1))
        .expect("authenticated response should arrive");
    assert!(accepted.starts_with("HTTP/1.1 202"), "{accepted}");
    assert!(
        accepted
            .to_ascii_lowercase()
            .contains("access-control-allow-origin: https://allowed.example"),
        "{accepted}"
    );
    let event = event_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("authenticated request should dispatch");
    assert!(event.payload["headers"].get("authorization").is_none());
    assert!(event_receiver.try_recv().is_err());
}

#[test]
fn incomplete_webhook_headers_are_disconnected_after_the_header_timeout() {
    let runner = Arc::new(
        |event: TriggerEvent| -> Result<baudbound_core::TriggerActivation, String> {
            panic!("incomplete request unexpectedly dispatched event {event:?}")
        },
    ) as Arc<TriggerRunner>;
    let host = test_host(webhook_service(false, 1.0), runner);
    let mut stream = TcpStream::connect(host.listener.local_addr())
        .expect("partial-header client should connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test client read timeout should configure");
    stream
        .write_all(b"POST /events/test HTTP/1.1\r\nHost: localhost")
        .expect("partial headers should write");

    let mut response = Vec::new();
    let result = stream.read_to_end(&mut response);

    assert!(
        result.is_ok() || !response.is_empty(),
        "listener should close or answer the incomplete request"
    );
}

#[test]
fn incomplete_webhook_bodies_receive_a_request_timeout() {
    let runner = Arc::new(
        |event: TriggerEvent| -> Result<baudbound_core::TriggerActivation, String> {
            panic!("incomplete request unexpectedly dispatched event {event:?}")
        },
    ) as Arc<TriggerRunner>;
    let host = test_host(webhook_service(false, 1.0), runner);
    let mut stream =
        TcpStream::connect(host.listener.local_addr()).expect("partial-body client should connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test client read timeout should configure");
    stream
        .write_all(
            b"POST /events/test HTTP/1.1\r\nHost: localhost\r\nContent-Length: 10\r\nConnection: close\r\n\r\n{}",
        )
        .expect("partial body should write");

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("timeout response should read");

    assert!(response.starts_with("HTTP/1.1 408"), "{response}");
}

#[test]
fn webhook_connection_limit_rejects_excess_clients_and_recovers() {
    let runner = Arc::new(|event: TriggerEvent| Ok(report(&event, Default::default())))
        as Arc<TriggerRunner>;
    let mut host = test_host_with_limits(webhook_service(false, 1.0), runner, 1, 1024);
    let mut occupying_client = TcpStream::connect(host.listener.local_addr())
        .expect("capacity-occupying client should connect");
    occupying_client
        .write_all(b"POST /events/test HTTP/1.1\r\nHost: localhost")
        .expect("partial headers should write");
    thread::sleep(Duration::from_millis(50));

    let mut rejected_client = TcpStream::connect(host.listener.local_addr())
        .expect("excess client should reach the listening socket");
    rejected_client
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test client read timeout should configure");
    rejected_client
        .write_all(
            b"POST /events/test HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
        )
        .expect("excess request should write before the socket is closed");
    let mut rejected_response = Vec::new();
    let rejected_result = rejected_client.read_to_end(&mut rejected_response);
    assert!(
        rejected_result.is_ok(),
        "capacity response should be readable"
    );
    assert!(
        String::from_utf8_lossy(&rejected_response).starts_with("HTTP/1.1 503"),
        "excess connection should receive an explicit capacity response: {}",
        String::from_utf8_lossy(&rejected_response)
    );

    drop(occupying_client);
    thread::sleep(Duration::from_millis(300));
    let recovered = send_request(&host, "POST", "/events/test", "{}");
    accept_next(&mut host);
    let recovered = recovered
        .recv_timeout(Duration::from_secs(1))
        .expect("listener should accept requests after capacity is released");
    assert!(recovered.starts_with("HTTP/1.1 202"), "{recovered}");
}

#[test]
fn timed_out_authentication_jobs_remain_bounded_until_they_exit() {
    let mut config = listener_config("127.0.0.1:0", 8, 1024);
    config.max_unauthenticated_connections = ResourceLimit::limited(1);
    config.pre_auth_timeout_ms = ResourceLimit::limited(20);
    let listener = WebhookListener::bind(
        config,
        webhook_service(false, 1.0),
        Arc::new(SlowAuthenticator(Duration::from_millis(200))),
        BTreeSet::new(),
    )
    .expect("webhook listener should bind");
    let address = listener.local_addr();

    let first = http_request(address, "POST", "/events/test", "{}", &[]);
    assert!(first.starts_with("HTTP/1.1 408"), "{first}");
    let second = http_request(address, "POST", "/events/test", "{}", &[]);
    assert!(second.starts_with("HTTP/1.1 503"), "{second}");

    thread::sleep(Duration::from_millis(225));
    let recovered = http_request(address, "POST", "/events/test", "{}", &[]);
    assert!(recovered.starts_with("HTTP/1.1 408"), "{recovered}");
}

#[test]
fn webhook_listener_stops_promptly_while_a_client_is_stalled() {
    let service = webhook_service(false, 1.0);
    let listener = WebhookListener::bind(
        listener_config("127.0.0.1:0", 1, 1024),
        service,
        Arc::new(AllowAllAuthenticator),
        BTreeSet::new(),
    )
    .expect("webhook listener should bind");
    let mut stalled_client =
        TcpStream::connect(listener.local_addr()).expect("stalled client should connect");
    stalled_client
        .write_all(b"POST /events/test HTTP/1.1\r\nHost: localhost")
        .expect("partial headers should write");
    thread::sleep(Duration::from_millis(50));

    let started = Instant::now();
    drop(listener);

    assert!(
        started.elapsed() < Duration::from_secs(1),
        "listener shutdown should not wait for the header timeout"
    );
}

#[test]
fn webhook_listener_restart_applies_updated_limits() {
    let runner = Arc::new(|event: TriggerEvent| Ok(report(&event, Default::default())))
        as Arc<TriggerRunner>;
    let mut host = test_host_with_limits(webhook_service(false, 1.0), runner, 8, 1024);

    let next_config = listener_config("127.0.0.1:0", 4, 2);
    host.listener
        .restart(
            next_config.clone(),
            host.service.clone(),
            Arc::new(AllowAllAuthenticator),
            BTreeSet::new(),
        )
        .expect("listener should restart with updated limits");
    assert!(host.listener.matches_configuration(&next_config));

    let accepted = send_request(&host, "POST", "/events/test", "{}");
    accept_next(&mut host);
    assert!(
        accepted
            .recv_timeout(Duration::from_secs(1))
            .expect("request at the new body limit should complete")
            .starts_with("HTTP/1.1 202")
    );

    let rejected = send_request(&host, "POST", "/events/test", "abc");
    assert!(
        rejected
            .recv_timeout(Duration::from_secs(1))
            .expect("request above the new body limit should complete")
            .starts_with("HTTP/1.1 413")
    );
}

#[test]
fn failed_webhook_listener_rebind_preserves_the_live_listener() {
    let runner = Arc::new(|event: TriggerEvent| Ok(report(&event, Default::default())))
        as Arc<TriggerRunner>;
    let mut host = test_host(webhook_service(false, 1.0), runner);
    let occupied =
        std::net::TcpListener::bind("127.0.0.1:0").expect("conflicting listener should bind");
    let occupied_address = occupied
        .local_addr()
        .expect("conflicting address should resolve")
        .to_string();

    host.listener
        .restart(
            listener_config(&occupied_address, 4, 512),
            host.service.clone(),
            Arc::new(AllowAllAuthenticator),
            BTreeSet::new(),
        )
        .expect_err("conflicting rebind must fail");

    let response = send_request(&host, "POST", "/events/test", "{}");
    accept_next(&mut host);
    assert!(
        response
            .recv_timeout(Duration::from_secs(1))
            .expect("original listener should remain available")
            .starts_with("HTTP/1.1 202")
    );
}

fn test_host(service: WebhookService, runner: Arc<TriggerRunner>) -> WebhookHost {
    test_host_with_limits(service, runner, 8, 1024)
}

fn test_host_with_limits(
    service: WebhookService,
    runner: Arc<TriggerRunner>,
    max_connections: usize,
    max_body_bytes: usize,
) -> WebhookHost {
    let authenticator: Arc<dyn NetworkTriggerAuthenticator> = Arc::new(AllowAllAuthenticator);
    WebhookHost {
        executor: TriggerExecutor::with_runner(2, 4, "webhook-test", runner)
            .expect("test webhook executor should start"),
        listener: WebhookListener::bind(
            listener_config("127.0.0.1:0", max_connections, max_body_bytes),
            service.clone(),
            authenticator,
            BTreeSet::new(),
        )
        .expect("test webhook listener should bind"),
        pending: BTreeMap::new(),
        service,
    }
}

fn listener_config(
    bind_address: &str,
    max_connections: usize,
    max_body_bytes: usize,
) -> WebhookListenerConfig {
    WebhookListenerConfig {
        bind_address: bind_address.to_owned(),
        body_read_progress_timeout_ms: ResourceLimit::limited(250),
        body_read_timeout_ms: ResourceLimit::limited(250),
        header_read_timeout_ms: ResourceLimit::limited(250),
        max_body_bytes,
        max_connections,
        max_header_bytes: ResourceLimit::limited(32 * 1024),
        max_unauthenticated_connections: ResourceLimit::limited(
            u64::try_from(max_connections).unwrap(),
        ),
        pre_auth_requests_per_minute_global: ResourceLimit::limited(10_000),
        pre_auth_requests_per_minute_per_address: ResourceLimit::limited(10_000),
        pre_auth_timeout_ms: ResourceLimit::limited(250),
    }
}

fn webhook_service(wait_for_response: bool, timeout_seconds: f64) -> WebhookService {
    webhook_service_for("test", "n-webhook", wait_for_response, timeout_seconds)
}

fn webhook_service_for(
    hook_name: &str,
    node_id: &str,
    wait_for_response: bool,
    timeout_seconds: f64,
) -> WebhookService {
    WebhookService::from_registrations([TriggerRegistration {
        action_type: "trigger.webhook".to_owned(),
        config: json!({
            "hookName": hook_name,
            "method": "POST",
            "responseTimeoutSeconds": timeout_seconds,
            "timeoutResponseBody": "fallback",
            "timeoutResponseContentType": "text/plain",
            "timeoutResponseStatus": 202,
            "waitForResponse": wait_for_response,
        }),
        node_id: node_id.to_owned(),
        runner_type: "webhook".to_owned(),
        script_id: "script-1".to_owned(),
        script_name: "Script One".to_owned(),
    }])
    .expect("test webhook service should register")
}

fn send_request(host: &WebhookHost, method: &str, path: &str, body: &str) -> Receiver<String> {
    send_request_with_headers(host, method, path, body, &[])
}

fn send_request_with_headers(
    host: &WebhookHost,
    method: &str,
    path: &str,
    body: &str,
    headers: &[(&str, &str)],
) -> Receiver<String> {
    let address = host.listener.local_addr();
    let (sender, receiver) = mpsc::channel();
    let method = method.to_owned();
    let path = path.to_owned();
    let body = body.to_owned();
    let headers = headers
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect::<Vec<_>>();
    thread::spawn(move || {
        sender
            .send(http_request(address, &method, &path, &body, &headers))
            .expect("HTTP response should send to test");
    });
    receiver
}

fn http_request(
    address: SocketAddr,
    method: &str,
    path: &str,
    body: &str,
    headers: &[(String, String)],
) -> String {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(1))
        .expect("test client should connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("test client read timeout should configure");
    let headers = headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect::<String>();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("test request should write");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("test response should read");
    response
}

fn accept_next(host: &mut WebhookHost) {
    host.accept_next(Duration::from_secs(1))
        .expect("webhook request should arrive");
}

fn wait_for_host_completion(host: &mut WebhookHost, status: &mut ServeStatusTracker) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        host.poll(status);
        if !host.has_pending_execution() {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("webhook executions did not complete before test deadline");
}

fn status_tracker() -> ServeStatusTracker {
    let server = ServiceControlServer::bind().expect("test IPC server should bind");
    ServeStatusTracker::start(server.descriptor().clone(), None, 0)
}

/// A started activation, which is what a runner closure returns for a trigger
/// that actually ran.
fn report(
    event: &TriggerEvent,
    variables: std::collections::BTreeMap<String, Value>,
) -> baudbound_core::TriggerActivation {
    baudbound_core::TriggerActivation::Started {
        report: Box::new(RunReport {
            identity: RunIdentity {
                run_id: format!("run-{}", event.node_id),
                script_id: event.script_id.clone(),
                trigger_node_id: event.node_id.clone(),
            },
            logs: Vec::new(),
            variable_scopes: Default::default(),
            variables,
        }),
    }
}
