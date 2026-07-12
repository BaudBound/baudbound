use std::{collections::BTreeMap, path::PathBuf};

use crate::{RuntimeCancellationToken, RuntimeSecretDeclaration, RuntimeStateStore};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

pub const SUPPORTED_CONTROL_ACTION_TYPES: &[&str] = &[
    "control.for_each",
    "control.if",
    "control.loop",
    "control.switch",
    "control.while",
];

pub const SUPPORTED_INTERNAL_ACTION_TYPES: &[&str] = &[
    "action.calculate",
    "action.delay",
    "action.log",
    "runtime.set_variable",
];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum RunnerMode {
    DesktopAgent,
    Headless,
    Service,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RunIdentity {
    pub run_id: String,
    pub script_id: String,
    pub trigger_node_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeContext {
    #[serde(skip, default)]
    pub cancellation: RuntimeCancellationToken,
    pub identity: RunIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_path: Option<PathBuf>,
    pub trigger_payload: Value,
    pub variables: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeLogEntry {
    pub level: String,
    pub message: String,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunReport {
    pub identity: RunIdentity,
    pub logs: Vec<RuntimeLogEntry>,
    pub variables: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct RuntimeActionRequest {
    pub action: Option<String>,
    pub action_type: String,
    pub config: Map<String, Value>,
    pub node_id: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeActionResult {
    pub output_data: Map<String, Value>,
}

#[derive(Debug, Error)]
pub enum RuntimeActionError {
    #[error("action {0} is not supported by this runner")]
    Unsupported(String),
    #[error("action was cancelled")]
    Cancelled,
    #[error("action {action_type} failed: {message}")]
    Failed {
        action_type: String,
        message: String,
    },
}

pub trait RuntimeActionHandler: Send + Sync {
    fn execute_action(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;
}

#[derive(Debug, Default)]
pub struct UnsupportedActionHandler;

impl RuntimeActionHandler for UnsupportedActionHandler {
    fn execute_action(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        Err(RuntimeActionError::Unsupported(request.action_type.clone()))
    }
}

pub struct RuntimeExecutionResources<'a> {
    pub(super) package_path: Option<PathBuf>,
    pub(super) action_handler: &'a dyn RuntimeActionHandler,
    pub(super) cancellation: RuntimeCancellationToken,
    pub(super) state_store: Option<&'a dyn RuntimeStateStore>,
    pub(super) secrets: &'a [RuntimeSecretDeclaration],
}

impl<'a> RuntimeExecutionResources<'a> {
    #[must_use]
    pub fn new(action_handler: &'a dyn RuntimeActionHandler) -> Self {
        Self {
            package_path: None,
            action_handler,
            cancellation: RuntimeCancellationToken::new(),
            state_store: None,
            secrets: &[],
        }
    }

    #[must_use]
    pub fn with_package_path(mut self, package_path: PathBuf) -> Self {
        self.package_path = Some(package_path);
        self
    }

    #[must_use]
    pub fn with_cancellation(mut self, cancellation: RuntimeCancellationToken) -> Self {
        self.cancellation = cancellation;
        self
    }

    #[must_use]
    pub fn with_state(
        mut self,
        state_store: &'a dyn RuntimeStateStore,
        secrets: &'a [RuntimeSecretDeclaration],
    ) -> Self {
        self.state_store = Some(state_store);
        self.secrets = secrets;
        self
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeNode {
    pub id: String,
    pub action_type: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub config: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeEdge {
    pub source: String,
    pub source_handle: String,
    pub target: String,
    pub target_handle: String,
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("program graph is invalid: {0}")]
    InvalidGraph(String),
    #[error("action failed for node {node_id}: {message}")]
    Action { node_id: String, message: String },
    #[error("calculation failed for node {node_id}: {message}")]
    Calculation { node_id: String, message: String },
    #[error("control flow failed for node {node_id}: {message}")]
    ControlFlow { node_id: String, message: String },
    #[error("runtime execution is not implemented for {action_type} node {node_id}")]
    UnsupportedStep {
        action_type: String,
        node_id: String,
    },
    #[error("runtime variable operation failed for node {node_id}: {message}")]
    VariableOperation { node_id: String, message: String },
    #[error("runtime state failed: {0}")]
    State(String),
    #[error("runtime execution failed: {0}")]
    Redacted(String),
    #[error("runtime was cancelled")]
    Cancelled,
}
