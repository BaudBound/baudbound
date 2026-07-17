use std::{
    collections::BTreeSet,
    io::{self, Write},
    net::{Shutdown, TcpListener, TcpStream},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::SyncSender,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{NetworkTriggerAuthenticator, TriggerEvent};

use super::{
    connection::{WebSocketConnectionContext, handle_connection},
    registry::WebSocketConnectionRegistry,
    route::WebSocketRoute,
};

const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(25);
const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(250);

pub(super) struct WebSocketListenerContext {
    pub(super) allow_browser_origins: Arc<BTreeSet<String>>,
    pub(super) authenticator: Arc<dyn NetworkTriggerAuthenticator>,
    pub(super) max_connections: usize,
    pub(super) max_message_bytes: usize,
    pub(super) registry: Arc<WebSocketConnectionRegistry>,
    pub(super) routes: Arc<RwLock<Vec<WebSocketRoute>>>,
    pub(super) running: Arc<AtomicBool>,
    pub(super) sender: SyncSender<TriggerEvent>,
}

pub(super) fn run_listener(listener: TcpListener, context: WebSocketListenerContext) {
    let active_connections = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    while context.running.load(Ordering::Acquire) {
        reap_finished(&mut handles);
        match listener.accept() {
            Ok((stream, remote_address)) => {
                let Some(permit) = ConnectionPermit::acquire(
                    Arc::clone(&active_connections),
                    context.max_connections,
                ) else {
                    reject_at_capacity(stream);
                    continue;
                };
                let spawn_result = thread::Builder::new()
                    .name("baudbound-websocket-connection".to_owned())
                    .spawn({
                        let connection_context = WebSocketConnectionContext {
                            allow_browser_origins: Arc::clone(&context.allow_browser_origins),
                            authenticator: Arc::clone(&context.authenticator),
                            max_message_bytes: context.max_message_bytes,
                            registry: Arc::clone(&context.registry),
                            routes: Arc::clone(&context.routes),
                            running: Arc::clone(&context.running),
                            sender: context.sender.clone(),
                        };
                        move || {
                            let _permit = permit;
                            handle_connection(stream, remote_address, connection_context);
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

struct ConnectionPermit {
    active: Arc<AtomicUsize>,
}

impl ConnectionPermit {
    fn acquire(active: Arc<AtomicUsize>, maximum: usize) -> Option<Self> {
        active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < maximum).then_some(current + 1)
            })
            .ok()?;
        Some(Self { active })
    }
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

fn reject_at_capacity(mut stream: TcpStream) {
    let body = "WebSocket connection limit reached.";
    let response = format!(
        "HTTP/1.1 503 Service Unavailable\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.shutdown(Shutdown::Both);
}
