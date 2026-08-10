//! Shared runner orchestration used by CLI, daemon, and desktop shells.

mod active_runs;
mod compatibility;
mod config;
mod execution_queue;
mod package;
mod run_records;
mod runtime_state;
mod secrets;
mod serial;
mod settings;
mod status;
mod sub_script;
mod system_variables;
mod triggers;
mod version;

use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{self, Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use baudbound_actions::{
    ActionLimits, ActionSecurityPolicy, HeadlessActionHandler, WebSocketMessageSink,
};
use baudbound_runtime::{
    RunIdentity, RuntimeActionHandler, RuntimeCancellationToken, RuntimeDefaultVariable,
    RuntimeDefaultVariableScope, RuntimeExecutionResources, RuntimeLogEntry, RuntimeOutputLimits,
    RuntimeRunObserver, RuntimeSecretDeclaration, execute_manual_program_with_state,
    execute_trigger_program_with_state,
};
use baudbound_script::{
    PackageLoadError, PackageSummary, ScriptPackage, load_script_package,
    load_script_package_reader, max_package_archive_bytes,
};
use baudbound_security::{
    BlacklistDecision, BlacklistMatchSubject, BlacklistPolicy, BlacklistSeverity,
    PermissiveBlacklistPolicy, RunnerPolicy, SecurityValidationError,
};
use baudbound_storage::{
    ApproveScriptRequest, GeneratedTriggerToken, InstalledScript, NetworkTriggerType,
    ScriptApproval, ScriptApprovalResult, ScriptStore, StorageError, TriggerAuthStatus,
    TriggerAuthentication,
};
use sha2::{Digest as _, Sha256};
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

pub use active_runs::ActiveRunTracker;
pub use baudbound_runtime::RunReport;
pub use baudbound_triggers::{TriggerActivation, TriggerOverlap};
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
    QueueOverflowStrategy, RunnerConfig, RunnerConfigError, RunnerSettings, SecurityPolicySettings,
    SecuritySettings, SerialDeviceSettings, SerialSettings, TimeFormat, TriggerSettings,
    UpdateSettings, WebSocketSettings, WebhookSettings,
};
pub use execution_queue::ExecutionAdmissionPolicy;
pub use package::PackageInspection;
pub use secrets::{InstalledSecretStatus, MAX_SECRET_INPUT_BYTES};
pub use serial::{SerialDeviceConfig, serial_device_configs_from_settings};
pub use settings::{InstalledScriptSettingStatus, MAX_SCRIPT_SETTING_INPUT_BYTES};
pub use status::{
    ApprovalStatus, PackageHashStatus, RunnerStatus, ScriptMetadata, ScriptStatus,
    TriggerRegistrationStatus,
};
pub use system_variables::{manifest_variables, system_variables};
pub use triggers::CoreTriggerDispatcher;

use runtime_state::CoreRuntimeStateStore;
use serial::action_serial_devices_from_config;
use status::{approval_status_from_package, has_current_approval};
use sub_script::CoreRuntimeActionHandler;
use triggers::trigger_registrations_from_package;

pub const SUPPORTED_CORE_ACTION_TYPES: &[&str] = &["action.script.run"];

pub const SUPPORTED_CORE_TRIGGER_ACTION_TYPES: &[&str] = &["trigger.manual"];

struct CompositeRunObserver {
    observers: Vec<Arc<dyn RuntimeRunObserver>>,
}

impl RuntimeRunObserver for CompositeRunObserver {
    fn run_started(&self, identity: &RunIdentity, cancellation: RuntimeCancellationToken) {
        for observer in &self.observers {
            observer.run_started(identity, cancellation.clone());
        }
    }

    fn log_emitted(&self, identity: &RunIdentity, entry: &RuntimeLogEntry) {
        for observer in &self.observers {
            observer.log_emitted(identity, entry);
        }
    }

    fn run_finished(&self, identity: &RunIdentity) {
        for observer in &self.observers {
            observer.run_finished(identity);
        }
    }

    fn run_recorded(&self) {
        for observer in &self.observers {
            observer.run_recorded();
        }
    }
}

#[derive(Clone)]
pub struct RunnerCore {
    active_runs: Arc<ActiveRunTracker>,
    action_handler: Option<Arc<dyn RuntimeActionHandler>>,
    action_limits: ActionLimits,
    action_security_policy: ActionSecurityPolicy,
    blacklist_policy: Arc<dyn BlacklistPolicy>,
    configured_target_runtimes: Vec<String>,
    execution_admission_policy: ExecutionAdmissionPolicy,
    execution_policy: baudbound_runtime::RuntimeExecutionPolicy,
    execution_queue: Arc<ScriptExecutionQueue>,
    output_limits: RuntimeOutputLimits,
    policy: RunnerPolicy,
    run_observers: Vec<Arc<dyn RuntimeRunObserver>>,
    serial_connections: Arc<baudbound_actions::SerialConnectionRegistry>,
    supported_target_runtimes: Vec<String>,
    websocket_sink: Option<Arc<dyn WebSocketMessageSink>>,
}

impl Default for RunnerCore {
    fn default() -> Self {
        Self {
            active_runs: ActiveRunTracker::new(),
            action_handler: None,
            action_limits: ActionLimits::default(),
            action_security_policy: ActionSecurityPolicy::default(),
            blacklist_policy: Arc::new(PermissiveBlacklistPolicy),
            configured_target_runtimes: Vec::new(),
            execution_admission_policy: ExecutionAdmissionPolicy::default(),
            execution_policy: baudbound_runtime::RuntimeExecutionPolicy::default(),
            execution_queue: Arc::new(ScriptExecutionQueue::default()),
            output_limits: RuntimeOutputLimits::default(),
            policy: RunnerPolicy::permissive(),
            run_observers: Vec::new(),
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
            active_runs: ActiveRunTracker::new(),
            action_handler: None,
            action_limits: ActionLimits {
                max_file_download_bytes: config.limits.max_file_download_bytes,
                max_file_read_bytes: config.limits.max_file_read_bytes,
                max_http_response_bytes: config.limits.max_http_response_bytes,
                max_generated_text_bytes: config.limits.max_generated_text_bytes,
                max_process_output_bytes: config.limits.max_process_output_bytes,
                max_processes_per_script: config.limits.max_processes_per_script,
                max_process_launches_per_minute: config.limits.max_process_launches_per_minute,
                max_file_write_bytes_per_run: config.limits.max_file_write_bytes_per_run,
            },
            action_security_policy: ActionSecurityPolicy {
                allow_process_execution: config.security.policy.allow_dangerous_permissions,
                allow_private_http_requests: config.security.policy.allow_private_http_requests,
                allow_shell_commands: config.security.policy.allow_shell_commands,
            },
            blacklist_policy: Arc::new(PermissiveBlacklistPolicy),
            configured_target_runtimes: config.runner.target_runtimes.clone(),
            execution_admission_policy: ExecutionAdmissionPolicy {
                max_active_runs_global: config.limits.max_active_runs_global,
                max_active_runs_per_script: config.limits.max_active_runs_per_script,
                max_queued_activations_per_script: config.limits.max_queued_activations_per_script,
                queue_overflow_strategy: config.limits.queue_overflow_strategy,
            },
            execution_policy: baudbound_runtime::RuntimeExecutionPolicy {
                max_steps_per_run: config.limits.max_steps_per_run,
                max_run_duration_ms: config.limits.max_run_duration_ms,
                max_loop_iterations_per_run: config.limits.max_loop_iterations_per_run,
            },
            execution_queue: Arc::new(ScriptExecutionQueue::new(ExecutionAdmissionPolicy {
                max_active_runs_global: config.limits.max_active_runs_global,
                max_active_runs_per_script: config.limits.max_active_runs_per_script,
                max_queued_activations_per_script: config.limits.max_queued_activations_per_script,
                queue_overflow_strategy: config.limits.queue_overflow_strategy,
            })),
            output_limits: RuntimeOutputLimits {
                max_log_entry_bytes: config.limits.max_log_entry_bytes,
                max_runtime_variable_bytes: config.limits.max_runtime_variable_bytes,
                max_retained_variable_bytes: config.limits.max_retained_variable_bytes,
                max_run_log_bytes: config.limits.max_run_log_bytes,
                max_run_record_bytes: config.limits.max_run_record_bytes,
            },
            policy: RunnerPolicy {
                allow_dangerous_actions: config.security.policy.allow_dangerous_permissions,
                allow_shell_commands: config.security.policy.allow_shell_commands,
            },
            run_observers: Vec::new(),
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
    pub fn with_blacklist_policy<T>(mut self, policy: Arc<T>) -> Self
    where
        T: BlacklistPolicy + 'static,
    {
        self.blacklist_policy = policy;
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
        self.run_observers.push(observer);
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
    /// Shares the queue and the in-flight run tracker with an existing core.
    ///
    /// A reload builds a new core while runs from the old one are still going.
    /// Both have to be carried over or the new core would admit work the old
    /// one is already running, and would see nothing to stop.
    pub fn with_execution_queue_from(mut self, existing: &Self) -> Self {
        self.execution_queue = Arc::clone(&existing.execution_queue);
        self.active_runs = Arc::clone(&existing.active_runs);
        self.execution_queue
            .update_policy(self.execution_admission_policy);
        self
    }

    #[must_use]
    pub const fn execution_admission_policy(&self) -> ExecutionAdmissionPolicy {
        self.execution_admission_policy
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
        self.validate_loaded_package(&package)?;
        Ok(PackageInspection::from_package(package))
    }

    pub fn validate_package(&self, path: impl AsRef<Path>) -> Result<PackageSummary, CoreError> {
        let package = load_script_package(path)?;
        self.validate_loaded_package(&package)?;
        Ok(package.summary())
    }

    pub fn import_package(
        &self,
        store: &impl ScriptStore,
        path: impl AsRef<Path>,
    ) -> Result<InstalledScript, CoreError> {
        let staged = StagedPackage::copy_from(path.as_ref())?;
        let package = staged.load_package()?;
        self.validate_loaded_package(&package)?;
        self.ensure_package_distribution_allowed(&package, &staged.path, "import")?;
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
        let package = staged.load_package()?;
        self.validate_loaded_package(&package)?;
        self.ensure_package_distribution_allowed(&package, &staged.path, "update")?;
        let declared_secret_names = package
            .manifest
            .secrets
            .iter()
            .map(|secret| secret.name.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let declared_settings = package
            .manifest
            .settings
            .iter()
            .map(|setting| {
                (
                    setting.name.clone(),
                    (setting.value_type.clone(), setting.item_type.clone()),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let result = store.update_script(import_request_from_package(&staged.path, package))?;
        for secret in store.list_secret_statuses(&result.id)? {
            if !declared_secret_names.contains(&secret.name) {
                store.remove_secret(&result.id, &secret.name)?;
            }
        }
        for setting in store.list_script_settings(&result.id)? {
            let compatible =
                declared_settings
                    .get(&setting.name)
                    .is_some_and(|(value_type, item_type)| {
                        settings::value_matches_type(
                            value_type,
                            item_type.as_deref(),
                            &setting.value,
                        )
                    });
            if !compatible {
                store.remove_script_setting(&result.id, &setting.name)?;
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
        if enabled {
            let installed = store.verify_script_package_hash(reference)?;
            self.ensure_installed_execution_allowed(&installed, "enable")?;
        }
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
            Some(reference) => vec![store.find_script(reference)?],
            None => store
                .list_scripts()?
                .into_iter()
                .filter(|script| script.enabled)
                .collect(),
        };

        let mut registrations = Vec::new();
        for script in scripts {
            if !include_inactive
                && self
                    .blacklist_decision_for_installed(&script)
                    .blocks_execution()
            {
                continue;
            }
            let (script, _staged_package, package) =
                load_verified_installed_package(store, &script.id)?;
            self.validate_loaded_package(&package)?;
            if !include_inactive && !has_current_approval(store, &script, &package)? {
                continue;
            }
            registrations.extend(trigger_registrations_from_package(
                store, &script, &package,
            )?);
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
    ) -> Result<TriggerActivation, CoreError> {
        self.dispatch_trigger_event_with_cancellation(store, event, RuntimeCancellationToken::new())
    }

    pub fn dispatch_trigger_event_with_cancellation(
        &self,
        store: &impl ScriptStore,
        event: TriggerEvent,
        cancellation: RuntimeCancellationToken,
    ) -> Result<TriggerActivation, CoreError> {
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
        let (installed, _staged_package, package) =
            load_verified_installed_package(store, reference)?;
        self.validate_loaded_package(&package)?;
        self.ensure_installed_distribution_allowed(&installed, "approve")?;
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

    pub fn list_installed_script_settings(
        &self,
        store: &impl ScriptStore,
        reference: &str,
    ) -> Result<Vec<InstalledScriptSettingStatus>, CoreError> {
        settings::list_installed_script_settings(self, store, reference)
    }

    pub fn set_installed_script_setting_from_text(
        &self,
        store: &impl ScriptStore,
        reference: &str,
        name: &str,
        value: &str,
    ) -> Result<InstalledScriptSettingStatus, CoreError> {
        settings::set_installed_script_setting_from_text(self, store, reference, name, value)
    }

    pub fn save_installed_script_settings_from_text(
        &self,
        store: &impl ScriptStore,
        reference: &str,
        values: &BTreeMap<String, String>,
    ) -> Result<Vec<InstalledScriptSettingStatus>, CoreError> {
        settings::save_installed_script_settings_from_text(self, store, reference, values)
    }

    pub fn remove_installed_script_setting(
        &self,
        store: &impl ScriptStore,
        reference: &str,
        name: &str,
    ) -> Result<bool, CoreError> {
        settings::remove_installed_script_setting(self, store, reference, name)
    }

    pub fn run_installed(
        &self,
        store: &impl ScriptStore,
        reference: &str,
    ) -> Result<RunReport, CoreError> {
        // No trigger node, so no overlap option applies and the activation
        // always runs.
        Ok(self
            .run_installed_with_trigger(store, reference, None, serde_json::Value::Null)?
            .report()
            .expect("a run without a trigger node never carries an overlap decision"))
    }

    pub fn run_installed_with_trigger(
        &self,
        store: &impl ScriptStore,
        reference: &str,
        trigger_node_id: Option<&str>,
        trigger_payload: serde_json::Value,
    ) -> Result<TriggerActivation, CoreError> {
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
    ) -> Result<TriggerActivation, CoreError> {
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
    ) -> Result<TriggerActivation, CoreError> {
        let installed = store.find_script(reference)?;
        if !installed.enabled {
            return Err(CoreError::ScriptDisabled(installed.id));
        }
        self.ensure_installed_execution_allowed(&installed, "run")?;
        if call_stack
            .iter()
            .any(|script_id| script_id == &installed.id)
        {
            let mut cycle = call_stack;
            cycle.push(installed.id);
            return Err(CoreError::SubScriptCycle(cycle.join(" -> ")));
        }
        // What this activation should do about a run that is already going is
        // settled before asking the queue for a permit. A stop or a skip never
        // becomes a run, so it must not wait behind the run it was sent to
        // replace, and must not count against the per-script limit.
        //
        // The package is only read when the script is actually busy, which is
        // the rare case, so an ordinary activation pays nothing for this.
        let mut overlap = TriggerOverlap::Queue;
        if call_stack.is_empty()
            && let Some(trigger_node_id) = trigger_node_id
            && self.active_runs.is_active(&installed.id)
            && let Ok((_, _, package)) = load_verified_installed_package(store, &installed.id)
        {
            overlap = package::trigger_overlap(&package.program, trigger_node_id);
            match overlap {
                TriggerOverlap::Skip => return Ok(TriggerActivation::Skipped),
                TriggerOverlap::Stop => {
                    let cancelled = self.active_runs.cancel_script(&installed.id);
                    return Ok(TriggerActivation::Stopped { cancelled });
                }
                TriggerOverlap::Restart => {
                    self.active_runs.cancel_script(&installed.id);
                }
                TriggerOverlap::Queue => {}
            }
        }
        let _ = overlap;
        let _execution_permit = if call_stack.is_empty() {
            self.execution_queue
                .acquire(&installed.id, &cancellation, || {
                    self.blacklist_decision_for_installed(&installed)
                        .blocks_execution()
                })
                .map_err(|error| match error {
                    AcquireError::Cancelled => {
                        CoreError::Runtime(baudbound_runtime::RuntimeError::Cancelled)
                    }
                    AcquireError::Busy => unreachable!("blocking acquisition cannot be busy"),
                    AcquireError::Full => CoreError::ScriptQueueFull(installed.id.clone()),
                    AcquireError::Superseded => {
                        CoreError::ScriptQueueSuperseded(installed.id.clone())
                    }
                    AcquireError::Rejected => self
                        .ensure_installed_execution_allowed(&installed, "run")
                        .expect_err("the queue only rejects newly restricted scripts"),
                })?
        } else {
            let owner_script_id = call_stack
                .last()
                .expect("non-empty sub-script call stack should have an owner");
            self.execution_queue
                .acquire_nested(owner_script_id, &installed.id, &cancellation, || {
                    self.blacklist_decision_for_installed(&installed)
                        .blocks_execution()
                })
                .map_err(|error| match error {
                    AcquireError::Busy => CoreError::SubScriptDeadlock {
                        owner: owner_script_id.clone(),
                        target: installed.id.clone(),
                    },
                    AcquireError::Cancelled => {
                        CoreError::Runtime(baudbound_runtime::RuntimeError::Cancelled)
                    }
                    AcquireError::Full => CoreError::ScriptQueueFull(installed.id.clone()),
                    AcquireError::Superseded => {
                        CoreError::ScriptQueueSuperseded(installed.id.clone())
                    }
                    AcquireError::Rejected => self
                        .ensure_installed_execution_allowed(&installed, "run")
                        .expect_err("the queue only rejects newly restricted scripts"),
                })?
        };
        let installed = store.find_script(&installed.id)?;
        if !installed.enabled {
            return Err(CoreError::ScriptDisabled(installed.id));
        }
        self.ensure_installed_execution_allowed(&installed, "run")?;
        let (installed, staged_package, package) =
            load_verified_installed_package(store, &installed.id)?;
        call_stack.push(installed.id.clone());

        if let Err(source) = self.validate_package_compatibility(&package) {
            append_failed_run_record(
                store,
                &package,
                trigger_node_id,
                source.to_string(),
                self.output_limits,
            )?;
            self.notify_run_recorded();
            return Err(source);
        }
        if !has_current_approval(store, &installed, &package)? {
            let source = CoreError::ApprovalRequired(installed.id.clone());
            append_failed_run_record(
                store,
                &package,
                trigger_node_id,
                source.to_string(),
                self.output_limits,
            )?;
            self.notify_run_recorded();
            return Err(source);
        }
        if let Err(source) = validate_package_security(&package, &self.policy) {
            append_failed_run_record(
                store,
                &package,
                trigger_node_id,
                source.to_string(),
                self.output_limits,
            )?;
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
                item_type: variable.item_type.clone(),
                // The declaration settles what a bare number meant, so this is
                // where a float declared as `300` becomes a float, before a run
                // can read it as an integer.
                value: settings::coerce_declared_value(
                    &variable.value_type,
                    variable.item_type.as_deref(),
                    variable.value.clone(),
                ),
            })
            .collect::<Vec<_>>();
        let script_settings =
            settings::resolve_runtime_script_settings(store, &installed.id, &package)?;

        // Machine facts, read once because none of them can change during a
        // run. The readings that can — the clock, the uptime — are resolved by
        // the runtime when a reference asks for them.
        let run_system_variables = system_variables::system_variables();
        let run_manifest_variables = system_variables::manifest_variables(&package.manifest);
        let runtime_resources = || {
            let resources = RuntimeExecutionResources::new(&core_action_handler)
                .with_package_path(staged_package.path.clone())
                .with_workspace_path(store.script_workspace(&installed.id))
                .with_package_bytes(Arc::clone(&staged_package.bytes))
                .with_cancellation(cancellation.clone())
                .with_state(&runtime_state_store, &secret_declarations)
                .with_default_variables(&default_variables)
                .with_script_settings(&script_settings)
                .with_output_limits(self.output_limits)
                .with_execution_policy(self.execution_policy)
                .with_system_variables(&run_system_variables)
                .with_manifest_variables(&run_manifest_variables);
            // The in-flight tracker always observes. A trigger asking to stop
            // an already running script depends on it, so it cannot be left to
            // whichever observers a caller happened to register.
            let tracker: Arc<dyn RuntimeRunObserver> =
                Arc::clone(&self.active_runs) as Arc<dyn RuntimeRunObserver>;
            match self.run_observers.as_slice() {
                [] => resources.with_observer(tracker),
                observers => resources.with_observer(Arc::new(CompositeRunObserver {
                    observers: std::iter::once(tracker)
                        .chain(observers.iter().cloned())
                        .collect(),
                })),
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
                append_cancelled_run_record(store, &package, trigger_node_id, self.output_limits)
            } else {
                append_failed_run_record(
                    store,
                    &package,
                    trigger_node_id,
                    source.to_string(),
                    self.output_limits,
                )
            };
            if let Err(error) = persistence_result {
                tracing::warn!("failed to persist failed run record: {error}");
            } else {
                self.notify_run_recorded();
            }
            CoreError::Runtime(source)
        })?;
        store.append_run_record(stored_run_record_from_report(&report, self.output_limits))?;
        self.notify_run_recorded();
        Ok(TriggerActivation::Started {
            report: Box::new(report),
        })
    }

    fn notify_run_recorded(&self) {
        for observer in &self.run_observers {
            observer.run_recorded();
        }
    }

    #[must_use]
    pub fn headless_action_handler(&self) -> HeadlessActionHandler {
        let mut action_handler = HeadlessActionHandler::default()
            .with_serial_connections(Arc::clone(&self.serial_connections))
            .with_limits(self.action_limits)
            .with_security_policy(self.action_security_policy);
        if let Some(sink) = &self.websocket_sink {
            action_handler = action_handler.with_websocket_sink(Arc::clone(sink));
        }
        action_handler
    }

    fn script_status(&self, store: &impl ScriptStore, script: InstalledScript) -> ScriptStatus {
        let (staged_package, package_hash_status) = match StagedPackage::verified_copy_from(&script)
        {
            Ok(staged) => (Some(staged), PackageHashStatus::Valid),
            Err(CoreError::Storage(StorageError::HashMismatch {
                expected, actual, ..
            })) => (None, PackageHashStatus::Mismatch { expected, actual }),
            Err(error) => (
                None,
                PackageHashStatus::Error {
                    message: error.to_string(),
                },
            ),
        };
        let package_hash_valid = matches!(package_hash_status, PackageHashStatus::Valid);

        let mut declared_permissions = Vec::new();
        let mut triggers = Vec::new();
        let mut package_error = None;
        let mut package_loaded = false;

        let package = if package_hash_valid {
            match staged_package
                .as_ref()
                .expect("valid package hash must retain its staged package")
                .load_package()
            {
                Ok(package) => {
                    package_loaded = true;
                    declared_permissions = package.permissions.declared_permissions.clone();
                    if let Err(error) = self.validate_loaded_package(&package) {
                        package_error = Some(error.to_string());
                    } else {
                        match trigger_registrations_from_package(store, &script, &package) {
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
            blacklist: self.blacklist_decision_for_installed(&script),
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

    fn validate_loaded_package(&self, package: &ScriptPackage) -> Result<(), CoreError> {
        self.validate_package_compatibility(package)?;
        validate_package_security(package, &self.policy)?;
        Ok(())
    }

    fn blacklist_decision_for_installed(&self, installed: &InstalledScript) -> BlacklistDecision {
        self.blacklist_policy
            .decide(&BlacklistMatchSubject::installed(
                &installed.id,
                &installed.package_hash,
            ))
    }

    fn ensure_installed_distribution_allowed(
        &self,
        installed: &InstalledScript,
        operation: &'static str,
    ) -> Result<(), CoreError> {
        let decision = self.blacklist_decision_for_installed(installed);
        if decision.blocks_distribution() {
            return Err(CoreError::Blacklisted {
                operation,
                severity: decision
                    .severity
                    .expect("a blocking blacklist decision has a severity"),
                titles: blacklist_titles(&decision),
            });
        }
        Ok(())
    }

    fn ensure_installed_execution_allowed(
        &self,
        installed: &InstalledScript,
        operation: &'static str,
    ) -> Result<(), CoreError> {
        let decision = self.blacklist_decision_for_installed(installed);
        if decision.blocks_execution() {
            return Err(CoreError::Blacklisted {
                operation,
                severity: decision
                    .severity
                    .expect("a blocking blacklist decision has a severity"),
                titles: blacklist_titles(&decision),
            });
        }
        Ok(())
    }

    fn ensure_package_distribution_allowed(
        &self,
        package: &ScriptPackage,
        path: &Path,
        operation: &'static str,
    ) -> Result<(), CoreError> {
        let decision = self.blacklist_policy.decide(&BlacklistMatchSubject {
            package_hash: Some(sha256_bytes(&read_bounded_package_bytes(path)?)),
            script_id: Some(package.manifest.id.clone()),
            trusted_urls: Vec::new(),
        });
        if decision.blocks_distribution() {
            return Err(CoreError::Blacklisted {
                operation,
                severity: decision
                    .severity
                    .expect("a blocking blacklist decision has a severity"),
                titles: blacklist_titles(&decision),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("script {0} is not approved for its current package")]
    ApprovalRequired(String),
    #[error(
        "cannot {operation} because the package is {severity:?} on the Official blacklist: {titles}"
    )]
    Blacklisted {
        operation: &'static str,
        severity: BlacklistSeverity,
        titles: String,
    },
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
    #[error("script {0} already has too many queued runs")]
    ScriptQueueFull(String),
    #[error("a newer activation replaced the oldest queued run for script {0}")]
    ScriptQueueSuperseded(String),
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
    #[error("Script Setting configuration is invalid: {0}")]
    InvalidSetting(String),
    #[error(transparent)]
    Version(#[from] VersionCompatibilityError),
}

fn blacklist_titles(decision: &BlacklistDecision) -> String {
    decision
        .entries
        .iter()
        .map(|entry| entry.title.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn load_verified_installed_package(
    store: &impl ScriptStore,
    reference: &str,
) -> Result<(InstalledScript, StagedPackage, ScriptPackage), CoreError> {
    VerifiedPackageSnapshot::load(store, reference).map(VerifiedPackageSnapshot::into_parts)
}

pub(crate) struct StagedPackage {
    _directory: tempfile::TempDir,
    bytes: Arc<[u8]>,
    path: PathBuf,
}

impl StagedPackage {
    fn copy_from(source_path: &Path) -> Result<Self, CoreError> {
        let bytes = read_bounded_package_bytes(source_path)?;
        Self::from_bytes(source_path, Arc::from(bytes))
    }

    fn from_bytes(source_path: &Path, bytes: Arc<[u8]>) -> Result<Self, CoreError> {
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
        let mut destination = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&staged_path)
            .map_err(|source| CoreError::PackageStage {
                path: staged_path.clone(),
                source,
            })?;
        destination
            .write_all(&bytes)
            .map_err(|source| CoreError::PackageStage {
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
            bytes,
            path: staged_path,
        })
    }

    fn load_package(&self) -> Result<ScriptPackage, CoreError> {
        load_script_package_reader(Cursor::new(self.bytes.as_ref())).map_err(CoreError::Package)
    }

    fn verified_copy_from(installed: &InstalledScript) -> Result<Self, CoreError> {
        let bytes = Arc::<[u8]>::from(read_bounded_package_bytes(&installed.package_path)?);
        Self::verified_from_bytes(installed, bytes)
    }

    fn verified_from_bytes(
        installed: &InstalledScript,
        bytes: Arc<[u8]>,
    ) -> Result<Self, CoreError> {
        let actual = sha256_bytes(&bytes);
        if actual != installed.package_hash {
            return Err(CoreError::Storage(StorageError::HashMismatch {
                script_id: installed.id.clone(),
                expected: installed.package_hash.clone(),
                actual,
            }));
        }
        Self::from_bytes(&installed.package_path, bytes)
    }
}

struct VerifiedPackageSnapshot {
    installed: InstalledScript,
    package: ScriptPackage,
    staged: StagedPackage,
}

impl VerifiedPackageSnapshot {
    fn load(store: &impl ScriptStore, reference: &str) -> Result<Self, CoreError> {
        let installed = store.find_script(reference)?;
        let staged = StagedPackage::verified_copy_from(&installed)?;
        let package = staged.load_package()?;
        Ok(Self {
            installed,
            package,
            staged,
        })
    }

    fn into_parts(self) -> (InstalledScript, StagedPackage, ScriptPackage) {
        (self.installed, self.staged, self.package)
    }
}

fn read_bounded_package_bytes(path: &Path) -> Result<Vec<u8>, CoreError> {
    let maximum = max_package_archive_bytes();
    let mut file = File::open(path).map_err(|source| CoreError::PackageStage {
        path: path.to_path_buf(),
        source,
    })?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| CoreError::PackageStage {
            path: path.to_path_buf(),
            source,
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(CoreError::PackageStage {
            path: path.to_path_buf(),
            source: io::Error::new(
                io::ErrorKind::InvalidData,
                format!("package archive exceeds {maximum} bytes"),
            ),
        });
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
