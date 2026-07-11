use std::{
    io,
    net::{SocketAddr, TcpStream},
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
    },
    time::Duration,
};

use tungstenite::{
    Error as WebSocketError, Message, accept_hdr_with_config,
    handshake::server::{ErrorResponse, Response as WebSocketHandshakeResponse},
    http::StatusCode,
    protocol::WebSocketConfig,
};

use crate::{TriggerEvent, try_send_trigger_event};

use super::{
    registry::WebSocketConnectionRegistry,
    route::{WebSocketHandshake, WebSocketRoute, WebSocketRouteKey, websocket_payload},
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const READ_POLL_INTERVAL: Duration = Duration::from_millis(100);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

#[allow(clippy::result_large_err)] // Tungstenite's handshake callback requires its HTTP response error type.
pub(super) fn handle_connection(
    stream: TcpStream,
    remote_address: SocketAddr,
    routes: Arc<RwLock<Vec<WebSocketRoute>>>,
    max_message_bytes: usize,
    sender: SyncSender<TriggerEvent>,
    registry: Arc<WebSocketConnectionRegistry>,
    running: Arc<AtomicBool>,
) {
    if let Err(error) = configure_handshake_stream(&stream) {
        tracing::warn!("failed to configure WebSocket handshake socket: {error}");
        return;
    }

    let selected = Arc::new(Mutex::new(None::<(WebSocketHandshake, WebSocketRoute)>));
    let selected_capture = Arc::clone(&selected);
    let route_capture = Arc::clone(&routes);
    let config = WebSocketConfig::default()
        .max_frame_size(Some(max_message_bytes))
        .max_message_size(Some(max_message_bytes));
    let mut websocket = match accept_hdr_with_config(
        stream,
        move |request: &tungstenite::handshake::server::Request,
              response: WebSocketHandshakeResponse| {
            let handshake = WebSocketHandshake::from_request(request);
            let route = route_capture
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .find(|route| route.path == handshake.path)
                .cloned();
            let Some(route) = route else {
                return Err(route_not_found_response(&handshake.path));
            };
            *selected_capture
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((handshake, route));
            Ok(response)
        },
        Some(config),
    ) {
        Ok(websocket) => websocket,
        Err(error) => {
            trace_handshake_failure(&error);
            return;
        }
    };

    if let Err(error) = configure_connected_stream(websocket.get_mut()) {
        tracing::warn!("failed to configure WebSocket connection socket: {error}");
        return;
    }
    let Some((handshake, initial_route)) = selected
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
    else {
        tracing::warn!("WebSocket handshake completed without selecting a route");
        return;
    };
    let route_key = initial_route.key();
    let connection_id = match registry.insert(route_key.clone(), websocket) {
        Ok(connection_id) => connection_id,
        Err(error) => {
            tracing::warn!("failed to register WebSocket connection: {error}");
            return;
        }
    };

    while running.load(Ordering::Acquire) {
        let Some(socket) = registry.socket(&connection_id) else {
            break;
        };
        let result = socket
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .read();
        match result {
            Ok(message) if message.is_text() || message.is_binary() => {
                let Some(route) = current_route(&routes, &route_key) else {
                    break;
                };
                match websocket_payload(
                    &route,
                    &handshake,
                    &connection_id,
                    &remote_address.to_string(),
                    message,
                    max_message_bytes,
                ) {
                    Ok(payload) => {
                        if !try_send_trigger_event(
                            &sender,
                            TriggerEvent {
                                node_id: route.registration.node_id.clone(),
                                payload,
                                script_id: route.registration.script_id.clone(),
                            },
                            "WebSocket",
                        ) {
                            break;
                        }
                    }
                    Err(error) => {
                        tracing::warn!("WebSocket message rejected: {error}");
                        break;
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(_)) => {
                if let Some(socket) = registry.socket(&connection_id) {
                    let _ = socket
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .flush();
                }
            }
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

fn current_route(
    routes: &RwLock<Vec<WebSocketRoute>>,
    route_key: &WebSocketRouteKey,
) -> Option<WebSocketRoute> {
    routes
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .find(|route| route.key() == *route_key)
        .cloned()
}

fn configure_handshake_stream(stream: &TcpStream) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
    stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))
}

fn configure_connected_stream(stream: &TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(READ_POLL_INTERVAL))?;
    stream.set_write_timeout(Some(WRITE_TIMEOUT))
}

fn route_not_found_response(path: &str) -> ErrorResponse {
    WebSocketHandshakeResponse::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Some(format!("WebSocket route {path:?} was not found.")))
        .expect("static WebSocket route rejection response must be valid")
}

fn trace_handshake_failure(error: &impl std::fmt::Debug) {
    tracing::debug!("WebSocket handshake ended without a connection: {error:?}");
}
