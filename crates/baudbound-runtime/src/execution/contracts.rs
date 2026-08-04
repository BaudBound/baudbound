use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    ResourceLimit, RuntimeCancellationToken, RuntimeDefaultVariable, RuntimeScriptSettings,
    RuntimeSecretDeclaration, RuntimeStateStore,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

pub const SUPPORTED_CONTROL_ACTION_TYPES: &[&str] = &[
    "control.color_match",
    "control.break_loop",
    "control.continue_loop",
    "control.for_each",
    "control.if",
    "control.repeat",
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
    #[serde(skip, default)]
    pub package_bytes: Option<Arc<[u8]>>,
    /// Location of the package archive being executed.
    ///
    /// This is a staged copy rather than the installed package, so it must not
    /// be used to derive any other location. Use `workspace_path` for the
    /// script workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_path: Option<PathBuf>,
    /// Directory that bounded relative file paths resolve inside.
    ///
    /// Supplied explicitly by the caller that owns the runner home. Deriving it
    /// from `package_path` produced a workspace under the system temp directory
    /// once packages began executing from a staged copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<PathBuf>,
    pub trigger_payload: Value,
    pub variables: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeLogEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_type: Option<String>,
    pub level: String,
    pub message: String,
    pub node_id: Option<String>,
    pub timestamp_unix_ms: u64,
}

#[must_use]
pub fn unix_timestamp_millis_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunReport {
    pub identity: RunIdentity,
    pub logs: Vec<RuntimeLogEntry>,
    #[serde(default)]
    pub variable_scopes: BTreeMap<String, RunVariableScope>,
    pub variables: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunVariableScope {
    Global,
    Metadata,
    NodeOutput,
    Persistent,
    Runtime,
    Secret,
    Setting,
}

impl RunVariableScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Metadata => "metadata",
            Self::NodeOutput => "node_output",
            Self::Persistent => "persistent",
            Self::Runtime => "runtime",
            Self::Secret => "secret",
            Self::Setting => "setting",
        }
    }
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
    pub sensitive_output_keys: BTreeSet<String>,
}

impl RuntimeActionResult {
    #[must_use]
    pub fn new(output_data: Map<String, Value>) -> Self {
        Self {
            output_data,
            sensitive_output_keys: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn with_sensitive_output(mut self, key: impl Into<String>) -> Self {
        self.sensitive_output_keys.insert(key.into());
        self
    }

    #[must_use]
    pub fn with_sensitive_output_path(
        mut self,
        output_key: &str,
        path: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Self {
        let mut reference = output_key.to_owned();
        for segment in path {
            reference.push('.');
            reference.push_str(segment.as_ref());
        }
        self.sensitive_output_keys.insert(reference);
        self
    }
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
    #[error("action {action_type} failed: {failure}")]
    StructuredFailure {
        action_type: String,
        failure: RuntimeActionFailure,
    },
}

#[derive(Debug, Clone, Error)]
#[error("{message}")]
pub struct RuntimeActionFailure {
    code: &'static str,
    error_type: &'static str,
    message: String,
    retryable: bool,
    details: Map<String, Value>,
}

impl RuntimeActionFailure {
    #[must_use]
    pub fn new(
        code: &'static str,
        error_type: &'static str,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            code,
            error_type,
            message: message.into(),
            retryable,
            details: Map::new(),
        }
    }

    #[must_use]
    pub fn with_detail(mut self, key: impl Into<String>, value: Value) -> Self {
        self.details.insert(key.into(), value);
        self
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }

    #[must_use]
    pub const fn error_type(&self) -> &'static str {
        self.error_type
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    #[must_use]
    pub const fn details(&self) -> &Map<String, Value> {
        &self.details
    }
}

pub trait RuntimeActionHandler: Send + Sync {
    fn execute_action(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;

    fn run_finished(&self, _identity: &RunIdentity) {}
}

pub trait RuntimeRunObserver: Send + Sync {
    fn run_started(&self, identity: &RunIdentity, cancellation: RuntimeCancellationToken);

    fn log_emitted(&self, identity: &RunIdentity, entry: &RuntimeLogEntry);

    fn run_finished(&self, identity: &RunIdentity);

    fn run_recorded(&self) {}
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
    pub(super) package_bytes: Option<Arc<[u8]>>,
    pub(super) package_path: Option<PathBuf>,
    pub(super) workspace_path: Option<PathBuf>,
    pub(super) action_handler: &'a dyn RuntimeActionHandler,
    pub(super) cancellation: RuntimeCancellationToken,
    pub(super) state_store: Option<&'a dyn RuntimeStateStore>,
    pub(super) default_variables: &'a [RuntimeDefaultVariable],
    pub(super) observer: Option<Arc<dyn RuntimeRunObserver>>,
    pub(super) output_limits: RuntimeOutputLimits,
    pub(super) execution_policy: RuntimeExecutionPolicy,
    pub(super) script_settings: Option<&'a RuntimeScriptSettings>,
    pub(super) secrets: &'a [RuntimeSecretDeclaration],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeOutputLimits {
    pub max_log_entry_bytes: usize,
    pub max_runtime_variable_bytes: ResourceLimit,
    pub max_retained_variable_bytes: usize,
    pub max_run_log_bytes: usize,
    pub max_run_record_bytes: usize,
}

impl Default for RuntimeOutputLimits {
    fn default() -> Self {
        Self {
            max_log_entry_bytes: 16 * 1024,
            max_runtime_variable_bytes: ResourceLimit::limited(16 * 1024 * 1024),
            max_retained_variable_bytes: 256 * 1024,
            max_run_log_bytes: 2 * 1024 * 1024,
            max_run_record_bytes: 8 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeExecutionPolicy {
    pub max_steps_per_run: ResourceLimit,
    pub max_run_duration_ms: ResourceLimit,
    pub max_loop_iterations_per_run: ResourceLimit,
}

impl Default for RuntimeExecutionPolicy {
    fn default() -> Self {
        Self {
            max_steps_per_run: ResourceLimit::limited(1_000_000),
            max_run_duration_ms: ResourceLimit::limited(3_600_000),
            max_loop_iterations_per_run: ResourceLimit::limited(1_000_000),
        }
    }
}

impl<'a> RuntimeExecutionResources<'a> {
    #[must_use]
    pub fn new(action_handler: &'a dyn RuntimeActionHandler) -> Self {
        Self {
            package_bytes: None,
            package_path: None,
            workspace_path: None,
            action_handler,
            cancellation: RuntimeCancellationToken::new(),
            state_store: None,
            default_variables: &[],
            observer: None,
            output_limits: RuntimeOutputLimits::default(),
            execution_policy: RuntimeExecutionPolicy::default(),
            script_settings: None,
            secrets: &[],
        }
    }

    #[must_use]
    pub fn with_package_path(mut self, package_path: PathBuf) -> Self {
        self.package_path = Some(package_path);
        self
    }

    /// Sets the directory that bounded relative file paths resolve inside.
    ///
    /// The caller must own this location. It is never derived from the package
    /// path, which points at a staged copy in a temporary directory.
    #[must_use]
    pub fn with_workspace_path(mut self, workspace_path: PathBuf) -> Self {
        self.workspace_path = Some(workspace_path);
        self
    }

    #[must_use]
    pub fn with_package_bytes(mut self, package_bytes: Arc<[u8]>) -> Self {
        self.package_bytes = Some(package_bytes);
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

    #[must_use]
    pub fn with_default_variables(
        mut self,
        default_variables: &'a [RuntimeDefaultVariable],
    ) -> Self {
        self.default_variables = default_variables;
        self
    }

    #[must_use]
    pub fn with_script_settings(mut self, settings: &'a RuntimeScriptSettings) -> Self {
        self.script_settings = Some(settings);
        self
    }

    #[must_use]
    pub fn with_observer(mut self, observer: Arc<dyn RuntimeRunObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    #[must_use]
    pub fn with_output_limits(mut self, output_limits: RuntimeOutputLimits) -> Self {
        self.output_limits = output_limits;
        self
    }

    #[must_use]
    pub fn with_execution_policy(mut self, execution_policy: RuntimeExecutionPolicy) -> Self {
        self.execution_policy = execution_policy;
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
    pub execution_order: u32,
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
    #[error("action failed for node {node_id}: {failure}")]
    StructuredAction {
        node_id: String,
        failure: RuntimeActionFailure,
    },
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
    #[error("runtime resource limit exceeded: {resource} reached configured limit {limit}")]
    ResourceLimitExceeded {
        resource: &'static str,
        limit: ResourceLimit,
    },
    #[error("runtime was cancelled")]
    Cancelled,
}
