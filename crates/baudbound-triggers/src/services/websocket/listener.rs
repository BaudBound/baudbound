use std::{
    collections::BTreeSet,
    io::{self, Write},
    net::{Shutdown, TcpListener, TcpStream},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{
    ConnectionGate, NetworkTriggerAuthenticator, PreAuthRateLimit, PreAuthRateLimiter, TriggerEvent,
};

use super::{
    connection::{WebSocketConnectionContext, handle_connection},
    registry::WebSocketConnectionRegistry,
    route::WebSocketRoute,
};

const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(25);
const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(250);

pub(super) struct WebSocketListenerContext {
    pub(super) allow_browser_origins: Arc<BTreeSet<String>>,
    pub(super) authenticated_connections: Arc<ConnectionGate>,
    pub(super) authenticator: Arc<dyn NetworkTriggerAuthenticator>,
    pub(super) handshake_timeout: Option<Duration>,
    pub(super) generation: Arc<str>,
    pub(super) max_message_bytes: usize,
    pub(super) pre_auth_rate_limiter: Arc<PreAuthRateLimiter>,
    pub(super) registry: Arc<WebSocketConnectionRegistry>,
    pub(super) routes: Arc<RwLock<Vec<WebSocketRoute>>>,
    pub(super) running: Arc<AtomicBool>,
    pub(super) sender: SyncSender<TriggerEvent>,
    pub(super) unauthenticated_connections: Arc<ConnectionGate>,
}

pub(super) fn run_listener(listener: TcpListener, context: WebSocketListenerContext) {
    let mut handles = Vec::new();
    while context.running.load(Ordering::Acquire) {
        reap_finished(&mut handles);
        match listener.accept() {
            Ok((stream, remote_address)) => {
                if let Err(limit) = context.pre_auth_rate_limiter.check(remote_address.ip()) {
                    reject_rate_limited(stream, limit);
                    continue;
                }
                let Some(permit) = context.unauthenticated_connections.try_acquire() else {
                    reject_http(
                        stream,
                        503,
                        "Service Unavailable",
                        "WebSocket pre-authentication limit reached.",
                    );
                    continue;
                };
                let spawn_result = thread::Builder::new()
                    .name("baudbound-websocket-connection".to_owned())
                    .spawn({
                        let connection_context = WebSocketConnectionContext {
                            allow_browser_origins: Arc::clone(&context.allow_browser_origins),
                            authenticated_connections: Arc::clone(
                                &context.authenticated_connections,
                            ),
                            authenticator: Arc::clone(&context.authenticator),
                            handshake_timeout: context.handshake_timeout,
                            generation: Arc::clone(&context.generation),
                            max_message_bytes: context.max_message_bytes,
                            registry: Arc::clone(&context.registry),
                            routes: Arc::clone(&context.routes),
                            running: Arc::clone(&context.running),
                            sender: context.sender.clone(),
                        };
                        move || {
                            handle_connection(stream, remote_address, connection_context, permit);
                        }
                    });
                match spawn_result {
                    Ok(handle) => handles.push(handle),
                    Err(error) => {
                        tracing::warn!("failed to start WebSocket connection thread: {error}")
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(error) => {
                tracing::warn!("WebSocket listener accept failed: {error}");
                thread::sleep(ACCEPT_ERROR_BACKOFF);
            }
        }
    }

    context.registry.close_all();
    for handle in handles {
        let _ = handle.join();
    }
    context.running.store(false, Ordering::Release);
}

fn reap_finished(handles: &mut Vec<JoinHandle<()>>) {
    let mut index = 0;
    while index < handles.len() {
        if handles[index].is_finished() {
            let handle = handles.swap_remove(index);
            let _ = handle.join();
        } else {
            index += 1;
        }
    }
}

fn reject_rate_limited(stream: TcpStream, limit: PreAuthRateLimit) {
    let body = match limit {
        PreAuthRateLimit::Address => {
            "WebSocket pre-authentication rate limit reached for this address."
        }
        PreAuthRateLimit::Global => "WebSocket pre-authentication rate limit reached.",
    };
    reject_http(stream, 429, "Too Many Requests", body);
}

fn reject_http(mut stream: TcpStream, status: u16, reason: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.shutdown(Shutdown::Both);
}
