//! Trigger adapter contracts for runner implementations.

mod network_admission;
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

pub use network_admission::{
    ConnectionGate, ConnectionPermit, PreAuthRateLimit, PreAuthRateLimiter,
};
pub use services::{
    DueScheduleBatch, FileWatchService, HotkeyService, NativeHotkeyService, ProcessStartedService,
    ScheduleService, SerialDeviceConfig, SerialInputService, SerialReaderStatus, StartupService,
    WebSocketConnectionRegistry, WebSocketService, WebSocketServiceConfig, WebhookDispatch,
    WebhookRequest, WebhookResponse, WebhookRouteTarget, WebhookService, normalize_windows_hotkey,
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

/// What a trigger's overlap option asks for when its script is already running.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TriggerOverlap {
    /// Wait for the active run, then start. The behaviour before this option
    /// existed, and the default for every trigger that does not say otherwise.
    #[default]
    Queue,
    /// Drop the activation. Nothing starts and nothing queues.
    Skip,
    /// Cancel the active run and start nothing. One trigger toggling its own
    /// long-running loop is the reason this exists.
    Stop,
    /// Cancel the active run, then start a fresh one.
    Restart,
}

impl TriggerOverlap {
    /// Reads the option from a trigger node's config.
    ///
    /// An absent or unrecognised value is `Queue`, so a package written before
    /// the option existed keeps its behaviour rather than being refused.
    #[must_use]
    pub fn from_config(config: &serde_json::Value) -> Self {
        match config.get("overlap").and_then(serde_json::Value::as_str) {
            Some("skip") => Self::Skip,
            Some("stop") => Self::Stop,
            Some("restart") => Self::Restart,
            _ => Self::Queue,
        }
    }
}

/// What a trigger activation actually did.
///
/// A stopped or skipped activation never becomes a run, so it has no report to
/// give. Reporting it as a failed run would be wrong: it did exactly what the
/// trigger asked.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum TriggerActivation {
    Started {
        /// Flattened so the run report keeps the shape callers already parse.
        /// The outcome is added beside its fields rather than nesting them.
        #[serde(flatten)]
        report: Box<RunReport>,
    },
    /// Cancelled this many in-flight runs and started nothing.
    Stopped {
        cancelled: usize,
    },
    Skipped,
}

impl TriggerActivation {
    #[must_use]
    pub fn outcome_name(&self) -> &'static str {
        match self {
            Self::Started { .. } => "started",
            Self::Stopped { .. } => "stopped",
            Self::Skipped => "skipped",
        }
    }

    /// The run report, when the activation actually ran one.
    #[must_use]
    pub fn report(self) -> Option<RunReport> {
        match self {
            Self::Started { report } => Some(*report),
            Self::Stopped { .. } | Self::Skipped => None,
        }
    }
}

pub trait TriggerDispatcher: Send + Sync {
    fn dispatch(&self, event: TriggerEvent) -> Result<TriggerActivation, TriggerError>;
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

#[cfg(test)]
mod activation_json_tests {
    use baudbound_runtime::RunIdentity;

    use super::*;

    fn report() -> RunReport {
        RunReport {
            identity: RunIdentity {
                run_id: "run-1".to_owned(),
                script_id: "script-1".to_owned(),
                trigger_node_id: "n-hotkey".to_owned(),
            },
            logs: Vec::new(),
            variable_scopes: Default::default(),
            variables: Default::default(),
        }
    }

    #[test]
    fn a_started_activation_keeps_the_report_shape_callers_parse() {
        // The CLI prints this with --json. Nesting the report under a key would
        // silently break anyone reading identity or variables from it, so the
        // outcome is added beside those fields rather than wrapping them.
        let value = serde_json::to_value(TriggerActivation::Started {
            report: Box::new(report()),
        })
        .expect("an activation should serialize");

        assert_eq!(value["outcome"], "started");
        assert_eq!(value["identity"]["script_id"], "script-1");
        assert_eq!(value["identity"]["trigger_node_id"], "n-hotkey");
        assert!(value.get("variables").is_some());
        assert!(
            value.get("report").is_none(),
            "the report must stay flattened, not nested"
        );
    }

    #[test]
    fn an_activation_that_did_not_run_names_what_it_did() {
        let stopped = serde_json::to_value(TriggerActivation::Stopped { cancelled: 2 })
            .expect("an activation should serialize");
        assert_eq!(stopped["outcome"], "stopped");
        assert_eq!(stopped["cancelled"], 2);
        assert!(stopped.get("identity").is_none(), "no run, so no report");

        let skipped = serde_json::to_value(TriggerActivation::Skipped)
            .expect("an activation should serialize");
        assert_eq!(skipped["outcome"], "skipped");
    }
}
