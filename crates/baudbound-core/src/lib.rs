//! Shared runner orchestration used by CLI, daemon, and desktop shells.

mod compatibility;
mod config;
mod execution_queue;
mod package;
mod run_records;
mod runtime_state;
mod secrets;
mod serial;
mod status;
mod sub_script;
mod triggers;
mod version;

use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use baudbound_actions::{ActionLimits, HeadlessActionHandler, WebSocketMessageSink};
use baudbound_runtime::{
    RuntimeActionHandler, RuntimeCancellationToken, RuntimeDefaultVariable,
    RuntimeDefaultVariableScope, RuntimeExecutionResources, RuntimeRunObserver,
    RuntimeSecretDeclaration, execute_manual_program_with_state,
    execute_trigger_program_with_state,
};
use baudbound_script::{PackageLoadError, PackageSummary, ScriptPackage, load_script_package};
use baudbound_security::{RunnerPolicy, SecurityValidationError};
use baudbound_storage::{
    ApproveScriptRequest, GeneratedTriggerToken, InstalledScript, NetworkTriggerType,
    ScriptApproval, ScriptApprovalResult, ScriptStore, StorageError, TriggerAuthStatus,
    TriggerAuthentication,
};
use thiserror::Error;

use compatibility::{CompatibilityError, runner_target_runtime_names, validate_package_for_runner};
use execution_queue::{AcquireError, ScriptExecutionQueue};
use package::{
    import_request_from_package, network_trigger_definitions, validate_package_security,
};
use run_records::{
    append_cancelled_run_record, append_failed_run_record, stored_run_record_from_report,
};
use version::{VersionCompatibilityError, validate_minimum_runner_version};

pub use baudbound_runtime::RunReport;
pub use baudbound_triggers::{TriggerDispatcher, TriggerEvent, TriggerRegistration};
pub use compatibility::{DESKTOP_ONLY_ACTIONS, RunnerExecutionMode, WINDOWS_DESKTOP_ONLY_ACTIONS};
pub use config::{
    DEFAULT_MAX_FILE_DOWNLOAD_BYTES, DEFAULT_MAX_FILE_READ_BYTES, DEFAULT_MAX_HTTP_RESPONSE_BYTES,
    DEFAULT_SERIAL_BAUD_RATE, DEFAULT_SERIAL_DTR_ON_OPEN, DEFAULT_SERIAL_MAX_MESSAGE_BYTES,
    DEFAULT_SERIAL_MESSAGE_GAP_MS, DEFAULT_SERIAL_OPEN_STABILIZATION_MS, DEFAULT_SERIAL_READ_MODE,
    DEFAULT_TRIGGER_RELOAD_SECONDS, DEFAULT_UPDATE_CHECK_INTERVAL_HOURS, DEFAULT_WEBHOOK_BIND,
    DEFAULT_WEBHOOK_MAX_BODY_BYTES, DEFAULT_WEBHOOK_PORT, DEFAULT_WEBSOCKET_BIND,
    DEFAULT_WEBSOCKET_MAX_MESSAGE_BYTES, DEFAULT_WEBSOCKET_PORT, DesktopSettings, DisplaySettings,
    LimitSettings, MAX_RUNNER_CONFIG_BYTES, MAX_SERIAL_MESSAGE_BYTES, MAX_SERIAL_MESSAGE_GAP_MS,
    RunnerConfig, RunnerConfigError, RunnerSettings, SerialDeviceSettings, SerialSettings,
    TimeFormat, TriggerSettings, UpdateSettings, WebSocketSettings, WebhookSettings,
};
pub use package::PackageInspection;
pub use secrets::{InstalledSecretStatus, MAX_SECRET_INPUT_BYTES};
pub use serial::{SerialDeviceConfig, serial_device_configs_from_settings};
pub use status::{
    ApprovalStatus, PackageHashStatus, RunnerStatus, ScriptMetadata, ScriptStatus,
    TriggerRegistrationStatus,
};
pub use triggers::CoreTriggerDispatcher;

use runtime_state::CoreRuntimeStateStore;
use serial::action_serial_devices_from_config;
use status::{approval_status_from_package, has_current_approval};
use sub_script::CoreRuntimeActionHandler;
use triggers::trigger_registrations_from_package;

pub const SUPPORTED_CORE_ACTION_TYPES: &[&str] = &["action.script.run"];

pub const SUPPORTED_CORE_TRIGGER_ACTION_TYPES: &[&str] = &["trigger.manual"];

#[derive(Clone)]
pub struct RunnerCore {
    action_handler: Option<Arc<dyn RuntimeActionHandler>>,
    action_limits: ActionLimits,
    configured_target_runtimes: Vec<String>,
    execution_queue: Arc<ScriptExecutionQueue>,
    run_observer: Option<Arc<dyn RuntimeRunObserver>>,
    serial_connections: Arc<baudbound_actions::SerialConnectionRegistry>,
    supported_target_runtimes: Vec<String>,
    websocket_sink: Option<Arc<dyn WebSocketMessageSink>>,
}

impl Default for RunnerCore {
    fn default() -> Self {
        Self {
            action_handler: None,
            action_limits: ActionLimits::default(),
            configured_target_runtimes: Vec::new(),
            execution_queue: Arc::new(ScriptExecutionQueue::default()),
            run_observer: None,
            serial_connections: Arc::new(baudbound_actions::SerialConnectionRegistry::default()),
            supported_target_runtimes: runner_target_runtime_names(
                &[],
                RunnerExecutionMode::Headless,
            ),
            websocket_sink: None,
        }
    }
}

impl RunnerCore {
    #[must_use]
    pub fn from_config(config: &RunnerConfig) -> Self {
        let serial_connections = Arc::new(baudbound_actions::SerialConnectionRegistry::new(
            action_serial_devices_from_config(config),
        ));
        Self {
            action_handler: None,
            action_limits: ActionLimits {
                max_file_download_bytes: config.limits.max_file_download_bytes,
                max_file_read_bytes: config.limits.max_file_read_bytes,
                max_http_response_bytes: config.limits.max_http_response_bytes,
            },
            configured_target_runtimes: config.runner.target_runtimes.clone(),
            execution_queue: Arc::new(ScriptExecutionQueue::default()),
            run_observer: None,
            serial_connections,
            supported_target_runtimes: runner_target_runtime_names(
                &config.runner.target_runtimes,
                RunnerExecutionMode::Headless,
            ),
            websocket_sink: None,
        }
    }

    #[must_use]
    pub fn with_execution_mode(mut self, execution_mode: RunnerExecutionMode) -> Self {
        self.supported_target_runtimes =
            runner_target_runtime_names(&self.configured_target_runtimes, execution_mode);
        self
    }

    #[must_use]
    pub fn with_action_handler<T>(mut self, handler: Arc<T>) -> Self
    where
        T: RuntimeActionHandler + 'static,
    {
        self.action_handler = Some(handler);
        self
    }

    #[must_use]
    pub fn with_run_observer<T>(mut self, observer: Arc<T>) -> Self
    where
        T: RuntimeRunObserver + 'static,
    {
        self.run_observer = Some(observer);
        self
    }

    #[must_use]
    pub fn with_websocket_sink<T>(mut self, sink: Arc<T>) -> Self
    where
        T: WebSocketMessageSink + 'static,
    {
        self.websocket_sink = Some(sink);
        self
    }

    #[must_use]
    pub fn with_execution_queue_from(mut self, existing: &Self) -> Self {
        self.execution_queue = Arc::clone(&existing.execution_queue);
        self
    }

    #[must_use]
    pub fn serial_connections(&self) -> Arc<baudbound_actions::SerialConnectionRegistry> {
        Arc::clone(&self.serial_connections)
    }

    #[must_use]
    pub fn supported_target_runtimes(&self) -> &[String] {
        &self.supported_target_runtimes
    }

    pub fn inspect_package(&self, path: impl AsRef<Path>) -> Result<PackageInspection, CoreError> {
        let package = load_script_package(path)?;
        self.validate_loaded_package(&package, &RunnerPolicy::permissive())?;
        Ok(PackageInspection::from_package(package))
    }

    pub fn validate_package(&self, path: impl AsRef<Path>) -> Result<PackageSummary, CoreError> {
        let package = load_script_package(path)?;
        self.validate_loaded_package(&package, &RunnerPolicy::permissive())?;
        Ok(package.summary())
    }

    pub fn import_package(
        &self,
        store: &impl ScriptStore,
        path: impl AsRef<Path>,
    ) -> Result<InstalledScript, CoreError> {
        let staged = StagedPackage::copy_from(path.as_ref())?;
        let package = load_script_package(&staged.path)?;
        self.validate_loaded_package(&package, &RunnerPolicy::permissive())?;
        store
            .import_script(import_request_from_package(&staged.path, package))
            .map_err(CoreError::Storage)
    }

    pub fn update_package(
        &self,
        store: &impl ScriptStore,
        path: impl AsRef<Path>,
    ) -> Result<InstalledScript, CoreError> {
        let staged = StagedPackage::copy_from(path.as_ref())?;
        let package = load_script_package(&staged.path)?;
        self.validate_loaded_package(&package, &RunnerPolicy::permissive())?;
        let declared_secret_names = package
            .manifest
            .secrets
            .iter()
            .map(|secret| secret.name.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let result = store.update_script(import_request_from_package(&staged.path, package))?;
        for secret in store.list_secret_statuses(&result.id)? {
            if !declared_secret_names.contains(&secret.name) {
                store.remove_secret(&result.id, &secret.name)?;
            }
        }
        Ok(result)
    }

    pub fn list_installed(
        &self,
        store: &impl ScriptStore,
    ) -> Result<Vec<InstalledScript>, CoreError> {
        store.list_scripts().map_err(CoreError::Storage)
    }

    pub fn list_trigger_auth(
        &self,
        store: &impl ScriptStore,
        reference: &str,
    ) -> Result<Vec<TriggerAuthStatus>, CoreError> {
        store
            .list_trigger_auth_statuses(reference)
            .map_err(CoreError::Storage)
    }

    pub fn rotate_trigger_token(
        &self,
        store: &impl ScriptStore,
        reference: &str,
        node_id: &str,
        trigger_type: NetworkTriggerType,
    ) -> Result<GeneratedTriggerToken, CoreError> {
        store
            .rotate_trigger_auth_token(reference, node_id, trigger_type)
            .map_err(CoreError::Storage)
    }

    pub fn set_trigger_auth_enabled(
        &self,
        store: &impl ScriptStore,
        reference: &str,
        node_id: &str,
        trigger_type: NetworkTriggerType,
        enabled: bool,
    ) -> Result<TriggerAuthStatus, CoreError> {
        store
            .set_trigger_auth_enabled(reference, node_id, trigger_type, enabled)
            .map_err(CoreError::Storage)
    }

    pub fn authenticate_network_trigger(
        &self,
        store: &impl ScriptStore,
        script_id: &str,
        node_id: &str,
        trigger_type: NetworkTriggerType,
        provided_token: Option<&str>,
    ) -> Result<TriggerAuthentication, CoreError> {
        store
            .authenticate_trigger(script_id, node_id, trigger_type, provided_token)
            .map_err(CoreError::Storage)
    }

    pub fn remove_installed(
        &self,
        store: &impl ScriptStore,
        reference: &str,
    ) -> Result<InstalledScript, CoreError> {
        store.remove_script(reference).map_err(CoreError::Storage)
    }

    pub fn inspect_installed(
        &self,
        store: &impl ScriptStore,
        reference: &str,
    ) -> Result<InstalledScript, CoreError> {
        store.find_script(reference).map_err(CoreError::Storage)
    }

    pub fn set_installed_enabled(
        &self,
        store: &impl ScriptStore,
        reference: &str,
        enabled: bool,
    ) -> Result<InstalledScript, CoreError> {
        store
            .set_script_enabled(reference, enabled)
            .map_err(CoreError::Storage)
    }

    pub fn status(&self, store: &impl ScriptStore) -> Result<RunnerStatus, CoreError> {
        let scripts = store
            .list_scripts()?
            .into_iter()
            .map(|script| self.script_status(store, script))
            .collect::<Vec<_>>();

        Ok(RunnerStatus::from_scripts(
            self.supported_target_runtimes.clone(),
            scripts,
        ))
    }

    pub fn list_trigger_registrations(
        &self,
        store: &impl ScriptStore,
        reference: Option<&str>,
    ) -> Result<Vec<TriggerRegistration>, CoreError> {
        let include_inactive = reference.is_some();
        let scripts = match reference {
            Some(reference) => vec![store.verify_script_package_hash(reference)?],
            None => store
                .list_scripts()?
                .into_iter()
                .filter(|script| script.enabled)
                .map(|script| store.verify_script_package_hash(&script.id))
                .collect::<Result<Vec<_>, _>>()?,
        };

        let mut registrations = Vec::new();
        for script in scripts {
            let package = load_script_package(&script.package_path)?;
            self.validate_package_compatibility(&package)?;
            if !include_inactive && !has_current_approval(store, &script, &package)? {
                continue;
            }
            registrations.extend(trigger_registrations_from_package(&script, &package)?);
        }
        registrations.sort_by(|left, right| {
            left.script_name
                .cmp(&right.script_name)
                .then_with(|| left.node_id.cmp(&right.node_id))
        });
        Ok(registrations)
    }

    pub fn trigger_dispatcher<'core, S: ScriptStore>(
        &'core self,
        store: &'core S,
    ) -> CoreTriggerDispatcher<'core, S> {
        CoreTriggerDispatcher { core: self, store }
    }

    pub fn dispatch_trigger_event(
        &self,
        store: &impl ScriptStore,
        event: TriggerEvent,
    ) -> Result<RunReport, CoreError> {
        self.dispatch_trigger_event_with_cancellation(store, event, RuntimeCancellationToken::new())
    }

    pub fn dispatch_trigger_event_with_cancellation(
        &self,
        store: &impl ScriptStore,
        event: TriggerEvent,
        cancellation: RuntimeCancellationToken,
    ) -> Result<RunReport, CoreError> {
        self.run_installed_with_trigger_and_cancellation(
            store,
            &event.script_id,
            Some(&event.node_id),
            event.payload,
            cancellation,
        )
    }

    pub fn approve_installed(
        &self,
        store: &impl ScriptStore,
        reference: &str,
    ) -> Result<ScriptApprovalResult, CoreError> {
        let installed = store.verify_script_package_hash(reference)?;
        let package = load_script_package(&installed.package_path)?;
        self.validate_loaded_package(&package, &RunnerPolicy::permissive())?;
        store
            .approve_script(ApproveScriptRequest {
                approved_permissions: package.permissions.declared_permissions.clone(),
                network_triggers: network_trigger_definitions(&package.program),
                package_hash: installed.package_hash,
                script_id: installed.id,
            })
            .map_err(CoreError::Storage)
    }

    pub fn revoke_approval(
        &self,
        store: &impl ScriptStore,
        reference: &str,
    ) -> Result<Option<ScriptApproval>, CoreError> {
        store
            .revoke_script_approval(reference)
            .map_err(CoreError::Storage)
    }

    pub fn list_installed_secrets(
        &self,
        store: &impl ScriptStore,
        reference: &str,
    ) -> Result<Vec<InstalledSecretStatus>, CoreError> {
        secrets::list_installed_secrets(self, store, reference)
    }

    pub fn set_installed_secret_from_text(
        &self,
        store: &impl ScriptStore,
        reference: &str,
        name: &str,
        value: &str,
    ) -> Result<InstalledSecretStatus, CoreError> {
        secrets::set_installed_secret_from_text(self, store, reference, name, value)
    }

    pub fn remove_installed_secret(
        &self,
        store: &impl ScriptStore,
        reference: &str,
        name: &str,
    ) -> Result<bool, CoreError> {
        secrets::remove_installed_secret(self, store, reference, name)
    }

    pub fn run_installed(
        &self,
        store: &impl ScriptStore,
        reference: &str,
    ) -> Result<RunReport, CoreError> {
        self.run_installed_with_trigger(store, reference, None, serde_json::Value::Null)
    }

    pub fn run_installed_with_trigger(
        &self,
        store: &impl ScriptStore,
        reference: &str,
        trigger_node_id: Option<&str>,
        trigger_payload: serde_json::Value,
    ) -> Result<RunReport, CoreError> {
        self.run_installed_with_trigger_and_cancellation(
            store,
            reference,
            trigger_node_id,
            trigger_payload,
            RuntimeCancellationToken::new(),
        )
    }

    pub fn run_installed_with_trigger_and_cancellation(
        &self,
        store: &impl ScriptStore,
        reference: &str,
        trigger_node_id: Option<&str>,
        trigger_payload: serde_json::Value,
        cancellation: RuntimeCancellationToken,
    ) -> Result<RunReport, CoreError> {
        self.run_installed_with_trigger_in_stack(
            store,
            reference,
            trigger_node_id,
            trigger_payload,
            Vec::new(),
            cancellation,
        )
    }

    pub(crate) fn run_installed_with_trigger_in_stack(
        &self,
        store: &impl ScriptStore,
        reference: &str,
        trigger_node_id: Option<&str>,
        trigger_payload: serde_json::Value,
        mut call_stack: Vec<String>,
        cancellation: RuntimeCancellationToken,
    ) -> Result<RunReport, CoreError> {
        let installed = store.verify_script_package_hash(reference)?;
        if !installed.enabled {
            return Err(CoreError::ScriptDisabled(installed.id));
        }
        if call_stack
            .iter()
            .any(|script_id| script_id == &installed.id)
        {
            let mut cycle = call_stack;
            cycle.push(installed.id);
            return Err(CoreError::SubScriptCycle(cycle.join(" -> ")));
        }
        let _execution_permit = if call_stack.is_empty() {
            self.execution_queue
                .acquire(&installed.id, &cancellation)
                .map_err(|error| match error {
                    AcquireError::Cancelled => {
                        CoreError::Runtime(baudbound_runtime::RuntimeError::Cancelled)
                    }
                    AcquireError::Busy => unreachable!("blocking acquisition cannot be busy"),
                })?
        } else {
            let owner_script_id = call_stack
                .last()
                .expect("non-empty sub-script call stack should have an owner");
            self.execution_queue
                .acquire_nested(owner_script_id, &installed.id, &cancellation)
                .map_err(|error| match error {
                    AcquireError::Busy => CoreError::SubScriptDeadlock {
                        owner: owner_script_id.clone(),
                        target: installed.id.clone(),
                    },
                    AcquireError::Cancelled => {
                        CoreError::Runtime(baudbound_runtime::RuntimeError::Cancelled)
                    }
                })?
        };
        call_stack.push(installed.id.clone());

        let package = load_script_package(&installed.package_path)?;
        if let Err(source) = self.validate_package_compatibility(&package) {
            append_failed_run_record(store, &package, trigger_node_id, source.to_string())?;
            self.notify_run_recorded();
            return Err(source);
        }
        if !has_current_approval(store, &installed, &package)? {
            let source = CoreError::ApprovalRequired(installed.id.clone());
            append_failed_run_record(store, &package, trigger_node_id, source.to_string())?;
            self.notify_run_recorded();
            return Err(source);
        }
        let policy = RunnerPolicy::permissive();
        if let Err(source) = validate_package_security(&package, &policy) {
            append_failed_run_record(store, &package, trigger_node_id, source.to_string())?;
            self.notify_run_recorded();
            return Err(CoreError::Security(source));
        }
        let headless_action_handler;
        let action_handler: &dyn RuntimeActionHandler =
            if let Some(action_handler) = &self.action_handler {
                action_handler.as_ref()
            } else {
                headless_action_handler = self.headless_action_handler();
                &headless_action_handler
            };
        let core_action_handler = CoreRuntimeActionHandler::new(
            call_stack,
            self,
            action_handler,
            store,
            cancellation.clone(),
        );
        let runtime_state_store = CoreRuntimeStateStore::new(store);
        let secret_declarations = package
            .manifest
            .secrets
            .iter()
            .map(|secret| RuntimeSecretDeclaration {
                name: secret.name.clone(),
                required: secret.required,
                value_type: secret.value_type.clone(),
            })
            .collect::<Vec<_>>();
        let default_variables = package
            .manifest
            .variables
            .iter()
            .map(|variable| RuntimeDefaultVariable {
                name: variable.name.clone(),
                scope: if variable.scope == "persistent" {
                    RuntimeDefaultVariableScope::Persistent
                } else {
                    RuntimeDefaultVariableScope::Runtime
                },
                value_type: variable.value_type.clone(),
                value: variable.value.clone(),
            })
            .collect::<Vec<_>>();

        let runtime_resources = || {
            let resources = RuntimeExecutionResources::new(&core_action_handler)
                .with_package_path(installed.package_path.clone())
                .with_cancellation(cancellation.clone())
                .with_state(&runtime_state_store, &secret_declarations)
                .with_default_variables(&default_variables);
            if let Some(observer) = &self.run_observer {
                resources.with_observer(Arc::clone(observer))
            } else {
                resources
            }
        };
        let report = match trigger_node_id {
            Some(trigger_node_id) => execute_trigger_program_with_state(
                &package.program,
                &package.manifest.id,
                trigger_node_id,
                trigger_payload,
                runtime_resources(),
            ),
            None => execute_manual_program_with_state(
                &package.program,
                &package.manifest.id,
                runtime_resources(),
            ),
        }
        .map_err(|source| {
            let persistence_result = if matches!(source, baudbound_runtime::RuntimeError::Cancelled)
            {
                append_cancelled_run_record(store, &package, trigger_node_id)
            } else {
                append_failed_run_record(store, &package, trigger_node_id, source.to_string())
            };
            if let Err(error) = persistence_result {
                tracing::warn!("failed to persist failed run record: {error}");
            } else {
                self.notify_run_recorded();
            }
            CoreError::Runtime(source)
        })?;
        store.append_run_record(stored_run_record_from_report(&report))?;
        self.notify_run_recorded();
        Ok(report)
    }

    fn notify_run_recorded(&self) {
        if let Some(observer) = &self.run_observer {
            observer.run_recorded();
        }
    }

    #[must_use]
    pub fn headless_action_handler(&self) -> HeadlessActionHandler {
        let mut action_handler = HeadlessActionHandler::default()
            .with_serial_connections(Arc::clone(&self.serial_connections))
            .with_limits(self.action_limits);
        if let Some(sink) = &self.websocket_sink {
            action_handler = action_handler.with_websocket_sink(Arc::clone(sink));
        }
        action_handler
    }

    fn script_status(&self, store: &impl ScriptStore, script: InstalledScript) -> ScriptStatus {
        let package_hash_status = match store.verify_script_package_hash(&script.id) {
            Ok(_) => PackageHashStatus::Valid,
            Err(StorageError::HashMismatch {
                expected, actual, ..
            }) => PackageHashStatus::Mismatch { expected, actual },
            Err(error) => PackageHashStatus::Error {
                message: error.to_string(),
            },
        };
        let package_hash_valid = matches!(package_hash_status, PackageHashStatus::Valid);

        let mut declared_permissions = Vec::new();
        let mut triggers = Vec::new();
        let mut package_error = None;
        let mut package_loaded = false;

        let package = if package_hash_valid {
            match load_script_package(&script.package_path) {
                Ok(package) => {
                    package_loaded = true;
                    declared_permissions = package.permissions.declared_permissions.clone();
                    if let Err(error) = self.validate_package_compatibility(&package) {
                        package_error = Some(error.to_string());
                    } else {
                        match trigger_registrations_from_package(&script, &package) {
                            Ok(registrations) => {
                                triggers = registrations
                                    .into_iter()
                                    .map(TriggerRegistrationStatus::from)
                                    .collect();
                            }
                            Err(error) => {
                                package_error = Some(error.to_string());
                            }
                        }
                    }
                    Some(package)
                }
                Err(error) => {
                    package_error = Some(error.to_string());
                    None
                }
            }
        } else {
            None
        };

        let approval_status = match store.find_script_approval(&script.id) {
            Ok(Some(approval)) => {
                approval_status_from_package(&script, package.as_ref(), package_loaded, &approval)
            }
            Ok(None) => ApprovalStatus::Missing,
            Err(error) => ApprovalStatus::Error {
                message: error.to_string(),
            },
        };

        let metadata = package
            .as_ref()
            .map(|package| ScriptMetadata::from(&package.manifest));

        ScriptStatus {
            approval_status,
            declared_permissions,
            installed: script,
            metadata,
            package_error,
            package_hash_status,
            triggers,
        }
    }

    fn validate_package_for_runner(
        &self,
        package: &ScriptPackage,
    ) -> Result<(), CompatibilityError> {
        validate_package_for_runner(package, &self.supported_target_runtimes)
    }

    fn validate_package_compatibility(&self, package: &ScriptPackage) -> Result<(), CoreError> {
        self.validate_package_for_runner(package)?;
        validate_minimum_runner_version(
            &package.manifest.minimum_runner_version,
            env!("CARGO_PKG_VERSION"),
        )?;
        Ok(())
    }

    fn validate_loaded_package(
        &self,
        package: &ScriptPackage,
        policy: &RunnerPolicy,
    ) -> Result<(), CoreError> {
        self.validate_package_compatibility(package)?;
        validate_package_security(package, policy)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("script {0} is not approved for its current package")]
    ApprovalRequired(String),
    #[error(transparent)]
    Compatibility(#[from] CompatibilityError),
    #[error("program trigger registration failed: {0}")]
    InvalidTriggerRegistration(String),
    #[error(transparent)]
    Package(#[from] PackageLoadError),
    #[error(transparent)]
    Runtime(#[from] baudbound_runtime::RuntimeError),
    #[error(transparent)]
    Security(#[from] SecurityValidationError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("script {0} is disabled")]
    ScriptDisabled(String),
    #[error("failed to stage package {path}: {source}")]
    PackageStage {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("sub-script execution would deadlock while {owner} waits for {target}")]
    SubScriptDeadlock { owner: String, target: String },
    #[error("sub-script cycle detected: {0}")]
    SubScriptCycle(String),
    #[error("secret configuration is invalid: {0}")]
    InvalidSecret(String),
    #[error(transparent)]
    Version(#[from] VersionCompatibilityError),
}

struct StagedPackage {
    _directory: tempfile::TempDir,
    path: PathBuf,
}

impl StagedPackage {
    fn copy_from(source_path: &Path) -> Result<Self, CoreError> {
        let file_name = source_path
            .file_name()
            .ok_or_else(|| CoreError::PackageStage {
                path: source_path.to_path_buf(),
                source: io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "package path has no file name",
                ),
            })?;
        let directory = tempfile::Builder::new()
            .prefix("baudbound-package-")
            .tempdir()
            .map_err(|source| CoreError::PackageStage {
                path: source_path.to_path_buf(),
                source,
            })?;
        let staged_path = directory.path().join(file_name);
        let mut source = File::open(source_path).map_err(|source| CoreError::PackageStage {
            path: source_path.to_path_buf(),
            source,
        })?;
        let mut destination = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&staged_path)
            .map_err(|source| CoreError::PackageStage {
                path: staged_path.clone(),
                source,
            })?;
        io::copy(&mut source, &mut destination).map_err(|source| CoreError::PackageStage {
            path: staged_path.clone(),
            source,
        })?;
        destination
            .flush()
            .map_err(|source| CoreError::PackageStage {
                path: staged_path.clone(),
                source,
            })?;
        destination
            .sync_all()
            .map_err(|source| CoreError::PackageStage {
                path: staged_path.clone(),
                source,
            })?;

        Ok(Self {
            _directory: directory,
            path: staged_path,
        })
    }
}

#[cfg(test)]
mod tests;
