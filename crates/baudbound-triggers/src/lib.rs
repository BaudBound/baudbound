//! Trigger adapter contracts for runner implementations.

mod services;

use std::{
    collections::BTreeMap,
    sync::mpsc::{SyncSender, TrySendError},
    time::{SystemTime, UNIX_EPOCH},
};

use baudbound_runtime::RunReport;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub use services::{
    FileWatchService, HotkeyService, NativeHotkeyService, ProcessStartedService, ScheduleService,
    SerialDeviceConfig, SerialInputService, SerialReaderStatus, StartupService,
    WebSocketConnectionRegistry, WebSocketService, WebSocketServiceConfig, WebhookDispatch,
    WebhookRequest, WebhookResponse, WebhookService,
};

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct TriggerRegistration {
    pub action_type: String,
    pub config: Value,
    pub node_id: String,
    pub runner_type: String,
    pub script_id: String,
    pub script_name: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct TriggerEvent {
    pub action_type: String,
    pub node_id: String,
    pub payload: Value,
    pub script_id: String,
}

#[derive(Debug, Error)]
pub enum TriggerError {
    #[error("trigger {0} is not supported by this runner")]
    Unsupported(String),
    #[error("trigger {0} failed: {1}")]
    Failed(String, String),
}

pub trait TriggerHandler: Send + Sync {
    fn register(&self, registration: &TriggerRegistration) -> Result<(), TriggerError>;
}

pub trait TriggerDispatcher: Send + Sync {
    fn dispatch(&self, event: TriggerEvent) -> Result<RunReport, TriggerError>;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NetworkTriggerKind {
    Webhook,
    WebSocket,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NetworkTriggerAuthenticationError {
    InvalidToken,
    MissingToken,
    Unavailable(String),
}

pub trait NetworkTriggerAuthenticator: Send + Sync {
    fn authenticate(
        &self,
        script_id: &str,
        node_id: &str,
        trigger_kind: NetworkTriggerKind,
        provided_token: Option<&str>,
    ) -> Result<(), NetworkTriggerAuthenticationError>;
}

pub trait SerialPortRebindSink: Send + Sync {
    fn update_serial_device_port(&self, device_id: &str, port: &str) -> Result<(), String>;
}

pub(crate) fn try_send_trigger_event(
    sender: &SyncSender<TriggerEvent>,
    event: TriggerEvent,
    source: &str,
) -> bool {
    match sender.try_send(event) {
        Ok(()) => true,
        Err(TrySendError::Full(event)) => {
            tracing::warn!(
                "{source} trigger {} for script {} was rejected because the listener event channel is at capacity",
                event.node_id,
                event.script_id
            );
            false
        }
        Err(TrySendError::Disconnected(event)) => {
            tracing::warn!(
                "{source} trigger {} for script {} was rejected because the listener event channel is closed",
                event.node_id,
                event.script_id
            );
            false
        }
    }
}

pub const SUPPORTED_SERVICE_TRIGGER_ACTION_TYPES: &[&str] = &[
    "trigger.file_watch",
    "trigger.hotkey",
    "trigger.process_started",
    "trigger.schedule",
    "trigger.serial_input",
    "trigger.startup",
    "trigger.webhook",
    "trigger.websocket",
];

#[derive(Debug, Clone, Serialize)]
pub struct TriggerServiceDiagnostics {
    pub running: bool,
    pub state: &'static str,
    pub summary: String,
}

impl TriggerServiceDiagnostics {
    pub(crate) fn active(registrations: usize, label: &str) -> Self {
        Self {
            running: registrations > 0,
            state: if registrations > 0 { "active" } else { "idle" },
            summary: format!("{registrations} {label} registered"),
        }
    }

    pub(crate) fn thread_backed(running: bool, registrations: usize, label: &str) -> Self {
        let active = running && registrations > 0;
        Self {
            running: active,
            state: if active {
                "active"
            } else if registrations > 0 {
                "stopped"
            } else {
                "idle"
            },
            summary: format!("{registrations} {label} registered"),
        }
    }
}

pub(crate) fn split_path_and_query(path_and_query: &str) -> (String, BTreeMap<String, String>) {
    let (path, query) = path_and_query
        .split_once('?')
        .unwrap_or((path_and_query, ""));
    (path.to_owned(), parse_query(query))
}

fn parse_query(query: &str) -> BTreeMap<String, String> {
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            (decode_url_component(key), decode_url_component(value))
        })
        .collect()
}

fn decode_url_component(value: &str) -> String {
    let plus_normalized = value.replace('+', " ");
    urlencoding::decode(&plus_normalized)
        .map(|value| value.into_owned())
        .unwrap_or(plus_normalized)
}

pub(crate) fn value_object_to_string_map(value: Option<&Value>) -> BTreeMap<String, String> {
    value
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|fields| fields.iter())
        .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_owned())))
        .collect()
}

pub(crate) fn config_string(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

pub(crate) fn required_config_string(
    registration: &TriggerRegistration,
    key: &str,
) -> Result<String, TriggerError> {
    config_string(&registration.config, key)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            TriggerError::Failed(
                registration.node_id.clone(),
                format!("trigger must define {key}"),
            )
        })
}

pub(crate) fn config_u16(config: &Value, key: &str, fallback: u16) -> u16 {
    match config.get(key) {
        Some(Value::Number(value)) => value
            .as_u64()
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| (100..=599).contains(value))
            .unwrap_or(fallback),
        Some(Value::String(value)) => value
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|value| (100..=599).contains(value))
            .unwrap_or(fallback),
        _ => fallback,
    }
}

pub(crate) fn config_bool(config: &Value, key: &str) -> bool {
    match config.get(key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => value.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

pub(crate) fn is_supported_http_method(method: &str) -> bool {
    matches!(
        method,
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    )
}

pub(crate) fn unix_timestamp(timestamp: SystemTime) -> u64 {
    timestamp
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

pub(crate) fn unix_timestamp_millis(timestamp: SystemTime) -> u128 {
    timestamp
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
