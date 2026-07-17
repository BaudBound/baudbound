use std::collections::{BTreeMap, btree_map::Entry};

use serde_json::{Value, json};
use tungstenite::Message;

use crate::{TriggerError, TriggerRegistration, split_path_and_query};

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct WebSocketRouteKey {
    pub(super) node_id: String,
    pub(super) path: String,
    pub(super) script_id: String,
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

    pub(super) fn key(&self) -> WebSocketRouteKey {
        WebSocketRouteKey {
            node_id: self.registration.node_id.clone(),
            path: self.path.clone(),
            script_id: self.registration.script_id.clone(),
        }
    }
}

pub(super) fn parse_routes(
    registrations: impl IntoIterator<Item = TriggerRegistration>,
) -> Result<Vec<WebSocketRoute>, TriggerError> {
    let mut routes = BTreeMap::new();
    for registration in registrations
        .into_iter()
        .filter(|registration| registration.action_type == "trigger.websocket")
    {
        let route = WebSocketRoute::from_registration(registration)?;
        match routes.entry(route.path.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(route);
            }
            Entry::Occupied(entry) => {
                return Err(TriggerError::Failed(
                    route.registration.node_id,
                    format!(
                        "WebSocket path {:?} is already registered by node {}",
                        route.path,
                        entry.get().registration.node_id
                    ),
                ));
            }
        }
    }
    Ok(routes.into_values().collect())
}

#[derive(Debug, Clone)]
pub(crate) struct WebSocketHandshake {
    pub(crate) headers: BTreeMap<String, String>,
    pub(crate) path: String,
    pub(crate) query: BTreeMap<String, String>,
}

impl WebSocketHandshake {
    pub(super) fn from_request(request: &tungstenite::handshake::server::Request) -> Self {
        let (path, query) = split_path_and_query(
            request
                .uri()
                .path_and_query()
                .map_or("/", |value| value.as_str()),
        );
        let headers = request
            .headers()
            .iter()
            .filter(|(name, _)| {
                name.as_str() != "x-baudbound-token" && name.as_str() != "sec-websocket-protocol"
            })
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_ascii_lowercase(), value.to_owned()))
            })
            .collect();
        Self {
            headers,
            path,
            query,
        }
    }
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
