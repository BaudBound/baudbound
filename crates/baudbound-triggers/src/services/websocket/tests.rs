use std::{
    collections::BTreeSet,
    net::TcpStream,
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

use baudbound_actions::WebSocketMessageSink;
use serde_json::json;
use tungstenite::{
    Error as WebSocketError, HandshakeError, Message, WebSocket, client, client::IntoClientRequest,
    handshake::client::ClientHandshake, http::HeaderValue,
};

use super::{WebSocketConnectionRegistry, WebSocketService, WebSocketServiceConfig};
use crate::{TriggerEvent, TriggerRegistration};

const TEST_TIMEOUT: Duration = Duration::from_secs(2);
type ClientConnectError = Box<HandshakeError<ClientHandshake<TcpStream>>>;

#[test]
fn dispatches_concurrent_clients_and_writes_to_the_originating_connections() {
    let registry = Arc::new(WebSocketConnectionRegistry::new());
    let (sender, receiver) = mpsc::sync_channel(32);
    let service = start_service(
        [registration("script-1", "n-websocket", "/events")],
        8,
        1024,
        sender,
        Arc::clone(&registry),
        None,
    );
    let address = service.bound_address().expect("service should be bound");
    let mut first = connect(address, "/events?source=first", Some(("x-client", "one")))
        .expect("first client should connect");
    let mut second =
        connect(address, "/events?source=second", None).expect("second client should connect");

    first
        .send(Message::Text(r#"{"client":1}"#.into()))
        .expect("first message should send");
    second
        .send(Message::Text(r#"{"client":2}"#.into()))
        .expect("second message should send");
    let events = [
        receiver
            .recv_timeout(TEST_TIMEOUT)
            .expect("first event should arrive"),
        receiver
            .recv_timeout(TEST_TIMEOUT)
            .expect("second event should arrive"),
    ];
    let connection_ids = events
        .iter()
        .map(|event| {
            assert_eq!(event.script_id, "script-1");
            assert_eq!(event.node_id, "n-websocket");
            event.payload["connection_id"]
                .as_str()
                .expect("connection id should be a string")
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        connection_ids.len(),
        2,
        "connection ids must be collision-free"
    );
    assert!(
        events
            .iter()
            .any(|event| event.payload["query"]["source"] == "first")
    );
    assert!(
        events
            .iter()
            .any(|event| event.payload["headers"]["x-client"] == "one")
    );
    let diagnostics = service.diagnostics();
    assert!(diagnostics.running);
    assert!(diagnostics.summary.contains("2 of 8 connections active"));

    let first_id = events
        .iter()
        .find(|event| event.payload["query"]["source"] == "first")
        .and_then(|event| event.payload["connection_id"].as_str())
        .expect("first connection id should exist");
    registry
        .send_text(first_id, "server-response")
        .expect("server write should succeed");
    assert_eq!(
        first.read().expect("first client should receive response"),
        Message::Text("server-response".into())
    );
    first
        .send(Message::Binary(b"binary-message".to_vec().into()))
        .expect("binary message should send");
    let binary_event = receiver
        .recv_timeout(TEST_TIMEOUT)
        .expect("binary event should arrive");
    assert_eq!(binary_event.payload["message"], "binary-message");
    assert_eq!(binary_event.payload["json"], json!({}));

    first.close(None).expect("first client should close");
    second.close(None).expect("second client should close");
    wait_for_connection_count(&registry, 0);
    assert!(registry.send_text(first_id, "after-close").is_err());
    drop(service);
}

#[test]
fn rejects_unknown_routes_and_connections_above_the_configured_limit() {
    let registry = Arc::new(WebSocketConnectionRegistry::new());
    let (sender, _receiver) = mpsc::sync_channel(32);
    let service = start_service(
        [registration("script-1", "n-websocket", "/events")],
        1,
        1024,
        sender,
        Arc::clone(&registry),
        None,
    );
    let address = service.bound_address().expect("service should be bound");

    let unknown = connect(address, "/missing", None).expect_err("unknown route must fail");
    assert_http_status(&unknown, 404);

    let mut active = connect(address, "/events", None).expect("first client should connect");
    wait_for_connection_count(&registry, 1);
    let overloaded = connect(address, "/events", None).expect_err("limit must reject client");
    assert_http_status(&overloaded, 503);

    active.close(None).expect("active client should close");
    wait_for_connection_count(&registry, 0);
    drop(service);
}

#[test]
fn protocol_limit_disconnects_oversized_messages_before_dispatch() {
    let registry = Arc::new(WebSocketConnectionRegistry::new());
    let (sender, receiver) = mpsc::sync_channel(32);
    let service = start_service(
        [registration("script-1", "n-websocket", "/events")],
        2,
        4,
        sender,
        Arc::clone(&registry),
        None,
    );
    let address = service.bound_address().expect("service should be bound");
    let mut client = connect(address, "/events", None).expect("client should connect");
    client
        .send(Message::Text("12345".into()))
        .expect("oversized frame should reach the server");

    assert!(receiver.recv_timeout(Duration::from_millis(250)).is_err());
    wait_for_connection_count(&registry, 0);
    assert!(
        client.read().is_err(),
        "server should disconnect oversized sender"
    );
    drop(service);
}

#[test]
fn reload_preserves_unchanged_routes_and_disconnects_changed_routes() {
    let registry = Arc::new(WebSocketConnectionRegistry::new());
    let (sender, receiver) = mpsc::sync_channel(32);
    let old_registration = registration("script-1", "n-old", "/events");
    let service = start_service(
        [old_registration.clone()],
        4,
        1024,
        sender.clone(),
        Arc::clone(&registry),
        None,
    );
    let address = service.bound_address().expect("service should be bound");
    let mut client = connect(address, "/events", None).expect("client should connect");
    send_and_expect(&mut client, &receiver, "before", "script-1", "n-old");

    let service = start_service(
        [old_registration],
        4,
        1024,
        sender.clone(),
        Arc::clone(&registry),
        Some(service),
    );
    assert_eq!(service.bound_address(), Some(address));
    send_and_expect(&mut client, &receiver, "unchanged", "script-1", "n-old");
    drop(WebSocketService::empty(Arc::clone(&registry)));
    send_and_expect(
        &mut client,
        &receiver,
        "after-placeholder-drop",
        "script-1",
        "n-old",
    );

    let service = start_service(
        [registration("script-2", "n-new", "/events")],
        4,
        1024,
        sender,
        Arc::clone(&registry),
        Some(service),
    );
    assert_eq!(service.bound_address(), Some(address));
    wait_for_connection_count(&registry, 0);
    assert!(
        client.read().is_err(),
        "changed route must close stale connection"
    );

    let mut replacement = connect(address, "/events", None).expect("new route should connect");
    send_and_expect(&mut replacement, &receiver, "after", "script-2", "n-new");
    replacement.close(None).expect("replacement should close");
    drop(service);
}

#[test]
fn rejects_duplicate_routes_before_binding_and_cleans_up_on_shutdown() {
    let registry = Arc::new(WebSocketConnectionRegistry::new());
    let (sender, _receiver) = mpsc::sync_channel(32);
    let error = match WebSocketService::start_or_reconfigure(
        [
            registration("script-1", "n-one", "/events"),
            registration("script-2", "n-two", "/events"),
        ],
        service_config(4, 1024),
        sender.clone(),
        Arc::clone(&registry),
        None,
    ) {
        Ok(_) => panic!("duplicate path must fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("already registered"), "{error}");

    let service = start_service(
        [registration("script-1", "n-websocket", "/events")],
        4,
        1024,
        sender,
        Arc::clone(&registry),
        None,
    );
    let address = service.bound_address().expect("service should be bound");
    let mut client = connect(address, "/events", None).expect("client should connect");
    wait_for_connection_count(&registry, 1);
    drop(service);
    wait_for_connection_count(&registry, 0);
    assert!(
        client.read().is_err(),
        "service shutdown must disconnect clients"
    );
}

fn start_service(
    registrations: impl IntoIterator<Item = TriggerRegistration>,
    max_connections: usize,
    max_message_bytes: usize,
    sender: SyncSender<TriggerEvent>,
    registry: Arc<WebSocketConnectionRegistry>,
    previous: Option<WebSocketService>,
) -> WebSocketService {
    WebSocketService::start_or_reconfigure(
        registrations,
        service_config(max_connections, max_message_bytes),
        sender,
        registry,
        previous,
    )
    .expect("test WebSocket service should start")
}

fn service_config(max_connections: usize, max_message_bytes: usize) -> WebSocketServiceConfig {
    WebSocketServiceConfig {
        bind: "127.0.0.1".to_owned(),
        max_connections,
        max_message_bytes,
        port: 0,
    }
}

fn connect(
    address: std::net::SocketAddr,
    path: &str,
    header: Option<(&str, &str)>,
) -> Result<WebSocket<TcpStream>, ClientConnectError> {
    let stream = TcpStream::connect_timeout(&address, TEST_TIMEOUT)
        .expect("test client should connect to listener");
    stream
        .set_read_timeout(Some(TEST_TIMEOUT))
        .expect("client read timeout should configure");
    stream
        .set_write_timeout(Some(TEST_TIMEOUT))
        .expect("client write timeout should configure");
    let mut request = format!("ws://{address}{path}")
        .into_client_request()
        .expect("test WebSocket request should build");
    if let Some((name, value)) = header {
        request.headers_mut().insert(
            name.parse::<tungstenite::http::HeaderName>()
                .expect("test header name should parse"),
            HeaderValue::from_str(value).expect("test header value should parse"),
        );
    }
    client(request, stream)
        .map(|(socket, _)| socket)
        .map_err(Box::new)
}

fn send_and_expect(
    client: &mut WebSocket<TcpStream>,
    receiver: &Receiver<TriggerEvent>,
    message: &str,
    script_id: &str,
    node_id: &str,
) {
    client
        .send(Message::Text(message.to_owned().into()))
        .expect("client message should send");
    let event = receiver
        .recv_timeout(TEST_TIMEOUT)
        .expect("trigger event should arrive");
    assert_eq!(event.script_id, script_id);
    assert_eq!(event.node_id, node_id);
    assert_eq!(event.payload["message"], message);
}

fn wait_for_connection_count(registry: &WebSocketConnectionRegistry, expected: usize) {
    let deadline = Instant::now() + TEST_TIMEOUT;
    while Instant::now() < deadline {
        if registry.connection_count() == expected {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(registry.connection_count(), expected);
}

fn assert_http_status(error: &ClientConnectError, expected: u16) {
    match error.as_ref() {
        HandshakeError::Failure(WebSocketError::Http(response)) => {
            assert_eq!(response.status().as_u16(), expected);
        }
        other => panic!("expected HTTP {expected}, found {other}"),
    }
}

fn registration(script_id: &str, node_id: &str, path: &str) -> TriggerRegistration {
    TriggerRegistration {
        action_type: "trigger.websocket".to_owned(),
        config: json!({ "path": path }),
        node_id: node_id.to_owned(),
        runner_type: "websocket".to_owned(),
        script_id: script_id.to_owned(),
        script_name: script_id.to_owned(),
    }
}
