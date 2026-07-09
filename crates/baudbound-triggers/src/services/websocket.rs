use std::{
    collections::BTreeMap,
    io,
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime},
};

use baudbound_actions::WebSocketMessageSink;
use serde_json::{Value, json};
use tungstenite::{
    Error as WebSocketError, Message, WebSocket, accept_hdr,
    handshake::server::{
        Request as WebSocketHandshakeRequest, Response as WebSocketHandshakeResponse,
    },
};

use crate::{
    TriggerError, TriggerEvent, TriggerRegistration, TriggerServiceDiagnostics,
    split_path_and_query, unix_timestamp_millis,
};

#[derive(Debug, Default)]
pub struct WebSocketConnectionRegistry {
    connections: Mutex<BTreeMap<String, Arc<Mutex<WebSocket<TcpStream>>>>>,
}

impl WebSocketConnectionRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn insert(&self, connection_id: String, websocket: WebSocket<TcpStream>) {
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        connections.insert(connection_id, Arc::new(Mutex::new(websocket)));
    }

    fn remove(&self, connection_id: &str) {
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        connections.remove(connection_id);
    }
}

impl WebSocketMessageSink for WebSocketConnectionRegistry {
    fn send_text(&self, connection_id: &str, message: &str) -> Result<usize, String> {
        let connection = {
            let connections = self
                .connections
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            connections.get(connection_id).cloned()
        }
        .ok_or_else(|| format!("unknown WebSocket connection id {connection_id:?}"))?;

        let bytes = message.len();
        connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .send(Message::Text(message.to_owned().into()))
            .map_err(|source| format!("failed to write WebSocket message: {source}"))?;
        Ok(bytes)
    }
}

pub struct WebSocketService {
    handle: Option<JoinHandle<()>>,
    route_count: usize,
    running: Arc<AtomicBool>,
}

impl WebSocketService {
    #[must_use]
    pub fn empty(_registry: Arc<WebSocketConnectionRegistry>) -> Self {
        Self {
            handle: None,
            route_count: 0,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        bind: &str,
        port: u16,
        max_message_bytes: usize,
        sender: Sender<TriggerEvent>,
        registry: Arc<WebSocketConnectionRegistry>,
    ) -> Result<Self, TriggerError> {
        let routes = registrations
            .into_iter()
            .filter(|registration| registration.action_type == "trigger.websocket")
            .map(WebSocketRoute::from_registration)
            .collect::<Result<Vec<_>, _>>()?;

        if routes.is_empty() {
            return Ok(Self::empty(registry));
        }

        let address = format!("{bind}:{port}");
        let listener = TcpListener::bind(&address).map_err(|source| {
            TriggerError::Failed(
                "trigger.websocket".to_owned(),
                format!("failed to bind WebSocket listener on {address}: {source}"),
            )
        })?;
        listener.set_nonblocking(true).map_err(|source| {
            TriggerError::Failed(
                "trigger.websocket".to_owned(),
                format!("failed to configure WebSocket listener: {source}"),
            )
        })?;

        let running = Arc::new(AtomicBool::new(true));
        let thread_running = Arc::clone(&running);
        let thread_registry = Arc::clone(&registry);
        let route_count = routes.len();
        let handle = thread::spawn(move || {
            run_websocket_listener(
                listener,
                routes,
                max_message_bytes,
                sender,
                thread_registry,
                thread_running,
            );
        });

        Ok(Self {
            handle: Some(handle),
            route_count,
            running,
        })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.route_count == 0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.route_count
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::thread_backed(
            self.running.load(Ordering::Relaxed),
            self.len(),
            "WebSocket route",
        )
    }
}

impl Drop for WebSocketService {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WebSocketRoute {
    pub(crate) path: String,
    pub(crate) registration: TriggerRegistration,
}

impl WebSocketRoute {
    pub(crate) fn from_registration(
        registration: TriggerRegistration,
    ) -> Result<Self, TriggerError> {
        let path = registration
            .config
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    "WebSocket trigger must define path".to_owned(),
                )
            })?
            .to_owned();
        if !path.starts_with('/') {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "WebSocket path must start with '/'".to_owned(),
            ));
        }

        Ok(Self { path, registration })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WebSocketHandshake {
    pub(crate) headers: BTreeMap<String, String>,
    pub(crate) path: String,
    pub(crate) query: BTreeMap<String, String>,
}

fn run_websocket_listener(
    listener: TcpListener,
    routes: Vec<WebSocketRoute>,
    max_message_bytes: usize,
    sender: Sender<TriggerEvent>,
    registry: Arc<WebSocketConnectionRegistry>,
    running: Arc<AtomicBool>,
) {
    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, remote_address)) => {
                let routes = routes.clone();
                let sender = sender.clone();
                let registry = Arc::clone(&registry);
                let running = Arc::clone(&running);
                thread::spawn(move || {
                    handle_websocket_connection(
                        stream,
                        remote_address.to_string(),
                        routes,
                        max_message_bytes,
                        sender,
                        registry,
                        running,
                    );
                });
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => {
                tracing::warn!("WebSocket listener accept failed: {error}");
                thread::sleep(Duration::from_millis(250));
            }
        }
    }
}

#[allow(clippy::result_large_err)] // tungstenite's handshake callback uses http::Response as its error type.
fn handle_websocket_connection(
    stream: TcpStream,
    remote_address: String,
    routes: Vec<WebSocketRoute>,
    max_message_bytes: usize,
    sender: Sender<TriggerEvent>,
    registry: Arc<WebSocketConnectionRegistry>,
    running: Arc<AtomicBool>,
) {
    let handshake = Arc::new(Mutex::new(None::<WebSocketHandshake>));
    let handshake_capture = Arc::clone(&handshake);
    let Ok(mut websocket) = accept_hdr(
        stream,
        move |request: &WebSocketHandshakeRequest, response: WebSocketHandshakeResponse| {
            let (path, query) = split_path_and_query(
                request
                    .uri()
                    .path_and_query()
                    .map_or("/", |value| value.as_str()),
            );
            let headers = request
                .headers()
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (name.as_str().to_ascii_lowercase(), value.to_owned()))
                })
                .collect();
            *handshake_capture
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(WebSocketHandshake {
                headers,
                path,
                query,
            });
            Ok(response)
        },
    ) else {
        return;
    };
    let _ = websocket
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(250)));
    let _ = websocket
        .get_mut()
        .set_write_timeout(Some(Duration::from_secs(5)));
    let handshake = handshake
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
        .unwrap_or_else(|| WebSocketHandshake {
            headers: BTreeMap::new(),
            path: "/".to_owned(),
            query: BTreeMap::new(),
        });
    let Some(route) = routes
        .iter()
        .find(|route| route.path == handshake.path)
        .cloned()
    else {
        let _ = websocket.close(None);
        return;
    };

    let connection_id = format!(
        "{}-{}",
        route.registration.node_id,
        unix_timestamp_millis(SystemTime::now())
    );
    registry.insert(connection_id.clone(), websocket);

    while running.load(Ordering::Relaxed) {
        let connection = {
            let connections = registry
                .connections
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            connections.get(&connection_id).cloned()
        };
        let Some(connection) = connection else {
            break;
        };
        let result = connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .read();
        match result {
            Ok(message) if message.is_text() || message.is_binary() => {
                let payload = websocket_payload(
                    &route,
                    &handshake,
                    &connection_id,
                    &remote_address,
                    message,
                    max_message_bytes,
                );
                match payload {
                    Ok(payload) => {
                        let _ = sender.send(TriggerEvent {
                            node_id: route.registration.node_id.clone(),
                            payload,
                            script_id: route.registration.script_id.clone(),
                        });
                    }
                    Err(error) => {
                        tracing::warn!("WebSocket message rejected: {error}");
                    }
                }
            }
            Ok(message) if message.is_close() => break,
            Ok(_) => {}
            Err(WebSocketError::Io(error))
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.kind() == io::ErrorKind::TimedOut => {}
            Err(WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed) => break,
            Err(error) => {
                tracing::warn!("WebSocket connection {connection_id} failed: {error}");
                break;
            }
        }
    }

    registry.remove(&connection_id);
}

pub(crate) fn websocket_payload(
    route: &WebSocketRoute,
    handshake: &WebSocketHandshake,
    connection_id: &str,
    remote_address: &str,
    message: Message,
    max_message_bytes: usize,
) -> Result<Value, String> {
    let bytes = match message {
        Message::Text(value) => value.to_string().into_bytes(),
        Message::Binary(value) => value.to_vec(),
        _ => Vec::new(),
    };
    if bytes.len() > max_message_bytes {
        return Err(format!(
            "message from {connection_id} exceeded {max_message_bytes} bytes"
        ));
    }
    let message = String::from_utf8_lossy(&bytes).into_owned();
    let json_body = serde_json::from_str::<Value>(&message).unwrap_or_else(|_| json!({}));

    Ok(json!({
        "connection_id": connection_id,
        "headers": handshake.headers,
        "json": json_body,
        "message": message,
        "path": handshake.path,
        "query": handshake.query,
        "remote_address": remote_address,
        "trigger_id": route.registration.node_id,
    }))
}
