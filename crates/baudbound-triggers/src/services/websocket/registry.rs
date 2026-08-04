use std::{
    collections::{BTreeMap, BTreeSet},
    net::{Shutdown, TcpStream},
    sync::{Arc, Mutex},
};

use baudbound_actions::WebSocketMessageSink;
use tungstenite::{Message, WebSocket};

use super::route::WebSocketRouteKey;

struct RegisteredConnection {
    route_key: WebSocketRouteKey,
    shutdown_stream: TcpStream,
    socket: Arc<Mutex<WebSocket<TcpStream>>>,
}

pub struct WebSocketConnectionRegistry {
    connections: Mutex<BTreeMap<String, Arc<RegisteredConnection>>>,
}

impl Default for WebSocketConnectionRegistry {
    fn default() -> Self {
        Self {
            connections: Mutex::new(BTreeMap::new()),
        }
    }
}

impl std::fmt::Debug for WebSocketConnectionRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WebSocketConnectionRegistry")
            .field("connection_count", &self.connection_count())
            .finish_non_exhaustive()
    }
}

impl WebSocketConnectionRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(super) fn insert(
        &self,
        route_key: WebSocketRouteKey,
        websocket: WebSocket<TcpStream>,
    ) -> Result<String, String> {
        let shutdown_stream = websocket
            .get_ref()
            .try_clone()
            .map_err(|source| format!("failed to prepare WebSocket shutdown handle: {source}"))?;
        let connection = Arc::new(RegisteredConnection {
            route_key,
            shutdown_stream,
            socket: Arc::new(Mutex::new(websocket)),
        });
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for _ in 0..4 {
            let connection_id = random_connection_id()?;
            if !connections.contains_key(&connection_id) {
                connections.insert(connection_id.clone(), Arc::clone(&connection));
                return Ok(connection_id);
            }
        }
        Err("failed to allocate a unique WebSocket connection id".to_owned())
    }

    pub(super) fn socket(&self, connection_id: &str) -> Option<Arc<Mutex<WebSocket<TcpStream>>>> {
        self.connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(connection_id)
            .map(|connection| Arc::clone(&connection.socket))
    }

    pub(super) fn remove(&self, connection_id: &str) {
        let connection = self
            .connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(connection_id);
        if let Some(connection) = connection {
            let _ = connection.shutdown_stream.shutdown(Shutdown::Both);
        }
    }

    pub(super) fn close_except(&self, route_keys: &BTreeSet<WebSocketRouteKey>) {
        let removed = {
            let mut connections = self
                .connections
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let stale_ids = connections
                .iter()
                .filter(|(_, connection)| !route_keys.contains(&connection.route_key))
                .map(|(connection_id, _)| connection_id.clone())
                .collect::<Vec<_>>();
            stale_ids
                .into_iter()
                .filter_map(|connection_id| connections.remove(&connection_id))
                .collect::<Vec<_>>()
        };
        for connection in removed {
            let _ = connection.shutdown_stream.shutdown(Shutdown::Both);
        }
    }

    pub(super) fn close_all(&self) {
        let connections = std::mem::take(
            &mut *self
                .connections
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        for (_, connection) in connections {
            let _ = connection.shutdown_stream.shutdown(Shutdown::Both);
        }
    }

    pub(super) fn connection_count(&self) -> usize {
        self.connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}

impl WebSocketMessageSink for WebSocketConnectionRegistry {
    fn send_text(
        &self,
        script_id: &str,
        trigger_node_id: &str,
        connection_id: &str,
        message: &str,
    ) -> Result<usize, String> {
        let socket = self
            .connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(connection_id)
            .filter(|connection| {
                connection.route_key.script_id == script_id
                    && connection.route_key.node_id == trigger_node_id
            })
            .map(|connection| Arc::clone(&connection.socket))
            .ok_or_else(|| "WebSocket connection is not available to this run".to_owned())?;
        let result = socket
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .send(Message::Text(message.to_owned().into()));
        if let Err(source) = result {
            self.remove(connection_id);
            return Err(format!("failed to write WebSocket message: {source}"));
        }
        Ok(message.len())
    }
}

fn random_connection_id() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|source| format!("failed to generate WebSocket connection id: {source}"))?;
    let mut id = String::with_capacity(3 + bytes.len() * 2);
    id.push_str("ws-");
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut id, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(id)
}
