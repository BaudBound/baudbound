use std::{
    collections::BTreeSet,
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

use crate::{NetworkTriggerAuthenticationError, NetworkTriggerAuthenticator, NetworkTriggerKind};
use crate::{TriggerEvent, try_send_trigger_event};

use super::{
    registry::WebSocketConnectionRegistry,
    route::{WebSocketHandshake, WebSocketRoute, WebSocketRouteKey, websocket_payload},
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const READ_POLL_INTERVAL: Duration = Duration::from_millis(100);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub(super) struct WebSocketConnectionContext {
    pub(super) allow_browser_origins: Arc<BTreeSet<String>>,
    pub(super) authenticator: Arc<dyn NetworkTriggerAuthenticator>,
    pub(super) max_message_bytes: usize,
    pub(super) registry: Arc<WebSocketConnectionRegistry>,
    pub(super) routes: Arc<RwLock<Vec<WebSocketRoute>>>,
    pub(super) running: Arc<AtomicBool>,
    pub(super) sender: SyncSender<TriggerEvent>,
}

#[allow(clippy::result_large_err)] // Tungstenite fixes the handshake callback's response error type.
pub(super) fn handle_connection(
    stream: TcpStream,
    remote_address: SocketAddr,
    context: WebSocketConnectionContext,
) {
    if let Err(error) = configure_handshake_stream(&stream) {
        tracing::warn!("failed to configure WebSocket handshake socket: {error}");
        return;
    }

    let selected = Arc::new(Mutex::new(None::<(WebSocketHandshake, WebSocketRoute)>));
    let selected_capture = Arc::clone(&selected);
    let route_capture = Arc::clone(&context.routes);
    let allow_browser_origins = Arc::clone(&context.allow_browser_origins);
    let authenticator = Arc::clone(&context.authenticator);
    let config = WebSocketConfig::default()
        .max_frame_size(Some(context.max_message_bytes))
        .max_message_size(Some(context.max_message_bytes));
    let mut websocket = match accept_hdr_with_config(
        stream,
        move |request: &tungstenite::handshake::server::Request,
              mut response: WebSocketHandshakeResponse| {
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
            if let Some(origin) = request
                .headers()
                .get("origin")
                .and_then(|value| value.to_str().ok())
                && !allow_browser_origins.contains(origin)
            {
                return Err(rejection_response(
                    StatusCode::FORBIDDEN,
                    "Browser origin is not allowed.",
                ));
            }
            let token = match handshake_token(request) {
                Ok(token) => token,
                Err(HandshakeTokenError::MultipleProtocolTokens) => {
                    return Err(rejection_response(
                        StatusCode::BAD_REQUEST,
                        "Provide exactly one WebSocket token.",
                    ));
                }
                Err(HandshakeTokenError::ConflictingLocations) => {
                    return Err(rejection_response(
                        StatusCode::BAD_REQUEST,
                        "WebSocket token headers disagree.",
                    ));
                }
            };
            if let Err(error) = authenticator.authenticate(
                &route.registration.script_id,
                &route.registration.node_id,
                NetworkTriggerKind::WebSocket,
                token.as_deref(),
            ) {
                return Err(authentication_rejection(error));
            }
            if offered_protocol(request, "baudbound.v1") {
                response.headers_mut().insert(
                    "sec-websocket-protocol",
                    "baudbound.v1"
                        .parse()
                        .expect("static WebSocket protocol header must be valid"),
                );
            }
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
    let connection_id = match context.registry.insert(route_key.clone(), websocket) {
        Ok(connection_id) => connection_id,
        Err(error) => {
            tracing::warn!("failed to register WebSocket connection: {error}");
            return;
        }
    };

    while context.running.load(Ordering::Acquire) {
        let Some(socket) = context.registry.socket(&connection_id) else {
            break;
        };
        let result = socket
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .read();
        match result {
            Ok(message) if message.is_text() || message.is_binary() => {
                let Some(route) = current_route(&context.routes, &route_key) else {
                    break;
                };
                match websocket_payload(
                    &route,
                    &handshake,
                    &connection_id,
                    &remote_address.to_string(),
                    message,
                    context.max_message_bytes,
                ) {
                    Ok(payload) => {
                        if !try_send_trigger_event(
                            &context.sender,
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
                if let Some(socket) = context.registry.socket(&connection_id) {
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

    context.registry.remove(&connection_id);
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

fn authentication_rejection(error: NetworkTriggerAuthenticationError) -> ErrorResponse {
    match error {
        NetworkTriggerAuthenticationError::MissingToken => {
            rejection_response(StatusCode::UNAUTHORIZED, "WebSocket token is required.")
        }
        NetworkTriggerAuthenticationError::InvalidToken => {
            rejection_response(StatusCode::FORBIDDEN, "WebSocket token is invalid.")
        }
        NetworkTriggerAuthenticationError::Unavailable(error) => {
            tracing::error!("WebSocket authentication state is unavailable: {error}");
            rejection_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "WebSocket authentication is unavailable.",
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandshakeTokenError {
    ConflictingLocations,
    MultipleProtocolTokens,
}

fn handshake_token(
    request: &tungstenite::handshake::server::Request,
) -> Result<Option<String>, HandshakeTokenError> {
    let header_token = request
        .headers()
        .get("x-baudbound-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let protocol_tokens = offered_protocols(request)
        .filter_map(|protocol| protocol.strip_prefix("bbtoken."))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if protocol_tokens.len() > 1 {
        return Err(HandshakeTokenError::MultipleProtocolTokens);
    }
    let protocol_token = protocol_tokens.first().copied();
    if let (Some(header), Some(protocol)) = (header_token, protocol_token)
        && header != protocol
    {
        return Err(HandshakeTokenError::ConflictingLocations);
    }
    Ok(header_token.or(protocol_token).map(str::to_owned))
}

fn offered_protocol(request: &tungstenite::handshake::server::Request, expected: &str) -> bool {
    offered_protocols(request).any(|protocol| protocol == expected)
}

fn offered_protocols(
    request: &tungstenite::handshake::server::Request,
) -> impl Iterator<Item = &str> {
    request
        .headers()
        .get("sec-websocket-protocol")
        .and_then(|value| value.to_str().ok())
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn rejection_response(status: StatusCode, message: &str) -> ErrorResponse {
    WebSocketHandshakeResponse::builder()
        .status(status)
        .body(Some(message.to_owned()))
        .expect("static WebSocket rejection response must be valid")
}

fn trace_handshake_failure(error: &impl std::fmt::Debug) {
    tracing::debug!("WebSocket handshake ended without a connection: {error:?}");
}
