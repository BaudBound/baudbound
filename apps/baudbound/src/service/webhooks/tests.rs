use std::{
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
use baudbound_runtime::RunIdentity;
use serde_json::{Value, json};

use super::*;
use crate::service::{executor::TriggerRunner, ipc::ServiceControlServer};

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
    let request = host
        .server
        .recv_timeout(Duration::from_secs(1))
        .expect("server receive should succeed")
        .expect("request should arrive");

    host.accept_request(request, 1024);

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
    let request = host
        .server
        .recv_timeout(Duration::from_secs(1))
        .expect("server receive should succeed")
        .expect("request should arrive");
    host.accept_request(request, 1024);

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
    let request = host
        .server
        .recv_timeout(Duration::from_secs(1))
        .expect("server receive should succeed")
        .expect("request should arrive");
    host.accept_request(request, 1024);

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
    let old_request = host
        .server
        .recv_timeout(Duration::from_secs(1))
        .expect("server receive should succeed")
        .expect("old route request should arrive");
    host.accept_request(old_request, 1024);

    host.service = webhook_service_for("new", "n-new", false, 1.0);
    let new_response = send_request(&host, "POST", "/events/new", "{}");
    let new_request = host
        .server
        .recv_timeout(Duration::from_secs(1))
        .expect("server receive should succeed")
        .expect("new route request should arrive");
    host.accept_request(new_request, 1024);

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
    let runner = Arc::new(|event: TriggerEvent| -> Result<RunReport, String> {
        panic!("rejected request unexpectedly dispatched event {event:?}")
    }) as Arc<TriggerRunner>;
    let mut host = test_host(webhook_service(false, 1.0), runner);

    let oversized_response = send_request(&host, "POST", "/events/test", "12345");
    let oversized_request = host
        .server
        .recv_timeout(Duration::from_secs(1))
        .expect("server receive should succeed")
        .expect("oversized request should arrive");
    host.accept_request(oversized_request, 4);
    let oversized_response = oversized_response
        .recv_timeout(Duration::from_secs(1))
        .expect("oversized request should receive a response");
    assert!(
        oversized_response.starts_with("HTTP/1.1 413"),
        "{oversized_response}"
    );

    let wrong_method_response = send_request(&host, "GET", "/events/test", "");
    let wrong_method_request = host
        .server
        .recv_timeout(Duration::from_secs(1))
        .expect("server receive should succeed")
        .expect("wrong-method request should arrive");
    host.accept_request(wrong_method_request, 4);
    let wrong_method_response = wrong_method_response
        .recv_timeout(Duration::from_secs(1))
        .expect("wrong-method request should receive a response");
    assert!(
        wrong_method_response.starts_with("HTTP/1.1 404"),
        "{wrong_method_response}"
    );
}

fn test_host(service: WebhookService, runner: Arc<TriggerRunner>) -> WebhookHost {
    WebhookHost {
        executor: TriggerExecutor::with_runner(2, 4, "webhook-test", runner)
            .expect("test webhook executor should start"),
        pending: BTreeMap::new(),
        server: Server::http("127.0.0.1:0").expect("test webhook server should bind"),
        service,
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
    let address = host
        .server
        .server_addr()
        .to_ip()
        .expect("test server should have an IP address");
    let (sender, receiver) = mpsc::channel();
    let method = method.to_owned();
    let path = path.to_owned();
    let body = body.to_owned();
    thread::spawn(move || {
        sender
            .send(http_request(address, &method, &path, &body))
            .expect("HTTP response should send to test");
    });
    receiver
}

fn http_request(address: SocketAddr, method: &str, path: &str, body: &str) -> String {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(1))
        .expect("test client should connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("test client read timeout should configure");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("test request should write");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("test response should read");
    response
}

fn wait_for_host_completion(host: &mut WebhookHost, status: &mut ServeStatusTracker) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if host.poll(status) {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("webhook execution did not complete before test deadline");
}

fn status_tracker() -> ServeStatusTracker {
    let server = ServiceControlServer::bind().expect("test IPC server should bind");
    ServeStatusTracker::start(server.descriptor().clone())
}

fn report(event: &TriggerEvent, variables: std::collections::BTreeMap<String, Value>) -> RunReport {
    RunReport {
        identity: RunIdentity {
            run_id: format!("run-{}", event.node_id),
            script_id: event.script_id.clone(),
            trigger_node_id: event.node_id.clone(),
        },
        logs: Vec::new(),
        variables,
    }
}
