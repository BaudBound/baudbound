//! Shared runner orchestration used by CLI, daemon, and desktop shells.

mod compatibility;
mod config;
mod package;
mod run_records;
mod runtime_state;
mod secrets;
mod serial;
mod status;
mod sub_script;
mod triggers;
mod version;

use std::{path::Path, sync::Arc};

use baudbound_actions::{HeadlessActionHandler, WebSocketMessageSink};
use baudbound_runtime::{
    RuntimeActionHandler, RuntimeCancellationToken, RuntimeDefaultVariable,
    RuntimeDefaultVariableScope, RuntimeExecutionResources, RuntimeSecretDeclaration,
    execute_manual_program_with_state, execute_trigger_program_with_state,
};
use baudbound_script::{PackageLoadError, PackageSummary, ScriptPackage, load_script_package};
use baudbound_security::{RunnerPolicy, SecurityValidationError};
use baudbound_storage::{
    ApproveScriptRequest, InstalledScript, ScriptApproval, ScriptStore, StorageError,
};
use thiserror::Error;

use compatibility::{
    CompatibilityError, default_host_target_runtime_names, runner_target_runtime_names,
    validate_package_for_runner,
};
use package::{import_request_from_package, validate_package_security};
use run_records::{
    append_cancelled_run_record, append_failed_run_record, stored_run_record_from_report,
};
use version::{VersionCompatibilityError, validate_minimum_runner_version};

pub use baudbound_runtime::RunReport;
pub use baudbound_triggers::{TriggerDispatcher, TriggerEvent, TriggerRegistration};
pub use compatibility::{DESKTOP_ONLY_ACTIONS, WINDOWS_DESKTOP_ONLY_ACTIONS};
pub use config::{
    DEFAULT_TRIGGER_RELOAD_SECONDS, DEFAULT_UPDATE_CHECK_INTERVAL_HOURS, DEFAULT_WEBHOOK_BIND,
    DEFAULT_WEBHOOK_MAX_BODY_BYTES, DEFAULT_WEBHOOK_PORT, DEFAULT_WEBSOCKET_BIND,
    DEFAULT_WEBSOCKET_MAX_MESSAGE_BYTES, DEFAULT_WEBSOCKET_PORT, DesktopSettings, DisplaySettings,
    RunnerConfig, RunnerConfigError, RunnerSettings, SerialDeviceSettings, SerialSettings,
    TimeFormat, TriggerSettings, UpdateSettings, WebSocketSettings, WebhookSettings,
};
pub use package::PackageInspection;
pub use secrets::InstalledSecretStatus;
pub use serial::{SerialDeviceConfig, serial_device_configs_from_settings};
pub use status::{
    ApprovalStatus, PackageHashStatus, RunnerStatus, ScriptStatus, TriggerRegistrationStatus,
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
    pub name: String,
    action_handler: Option<Arc<dyn RuntimeActionHandler>>,
    serial_devices: Vec<baudbound_actions::SerialDeviceConfig>,
    supported_target_runtimes: Vec<String>,
    websocket_sink: Option<Arc<dyn WebSocketMessageSink>>,
}

impl Default for RunnerCore {
    fn default() -> Self {
        Self {
            name: "BaudBound Runner".to_owned(),
            action_handler: None,
            serial_devices: Vec::new(),
            supported_target_runtimes: default_host_target_runtime_names(),
            websocket_sink: None,
        }
    }
}

impl RunnerCore {
    #[must_use]
    pub fn from_config(config: &RunnerConfig) -> Self {
        Self {
            name: config.runner_name(),
            action_handler: None,
            serial_devices: action_serial_devices_from_config(config),
            supported_target_runtimes: runner_target_runtime_names(&config.runner.target_runtimes),
            websocket_sink: None,
        }
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
    pub fn with_websocket_sink<T>(mut self, sink: Arc<T>) -> Self
    where
        T: WebSocketMessageSink + 'static,
    {
        self.websocket_sink = Some(sink);
        self
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
        let path = path.as_ref();
        let package = load_script_package(path)?;
        self.validate_loaded_package(&package, &RunnerPolicy::permissive())?;
        store
            .import_script(import_request_from_package(path, package))
            .map_err(CoreError::Storage)
    }

    pub fn update_package(
        &self,
        store: &impl ScriptStore,
        path: impl AsRef<Path>,
    ) -> Result<InstalledScript, CoreError> {
        let path = path.as_ref();
        let package = load_script_package(path)?;
        self.validate_loaded_package(&package, &RunnerPolicy::permissive())?;
        let declared_secret_names = package
            .manifest
            .secrets
            .iter()
            .map(|secret| secret.name.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let installed = store.update_script(import_request_from_package(path, package))?;
        for secret in store.list_secret_statuses(&installed.id)? {
            if !declared_secret_names.contains(&secret.name) {
                store.remove_secret(&installed.id, &secret.name)?;
            }
        }
        Ok(installed)
    }

    pub fn list_installed(
        &self,
        store: &impl ScriptStore,
    ) -> Result<Vec<InstalledScript>, CoreError> {
        store.list_scripts().map_err(CoreError::Storage)
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
            self.name.clone(),
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
    ) -> Result<ScriptApproval, CoreError> {
        let installed = store.verify_script_package_hash(reference)?;
        let package = load_script_package(&installed.package_path)?;
        self.validate_loaded_package(&package, &RunnerPolicy::permissive())?;
        store
            .approve_script(ApproveScriptRequest {
                approved_permissions: package.permissions.declared_permissions.clone(),
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
        call_stack.push(installed.id.clone());

        let package = load_script_package(&installed.package_path)?;
        if let Err(source) = self.validate_package_compatibility(&package) {
            append_failed_run_record(store, &package, trigger_node_id, source.to_string())?;
            return Err(source);
        }
        if !has_current_approval(store, &installed, &package)? {
            let source = CoreError::ApprovalRequired(installed.id.clone());
            append_failed_run_record(store, &package, trigger_node_id, source.to_string())?;
            return Err(source);
        }
        let policy = RunnerPolicy::permissive();
        if let Err(source) = validate_package_security(&package, &policy) {
            append_failed_run_record(store, &package, trigger_node_id, source.to_string())?;
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
            RuntimeExecutionResources::new(&core_action_handler)
                .with_package_path(installed.package_path.clone())
                .with_cancellation(cancellation.clone())
                .with_state(&runtime_state_store, &secret_declarations)
                .with_default_variables(&default_variables)
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
            }
            CoreError::Runtime(source)
        })?;
        store.append_run_record(stored_run_record_from_report(&report))?;
        Ok(report)
    }

    #[must_use]
    pub fn headless_action_handler(&self) -> HeadlessActionHandler {
        let mut action_handler =
            HeadlessActionHandler::from_serial_devices(self.serial_devices.clone());
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

        ScriptStatus {
            approval_status,
            declared_permissions,
            installed: script,
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
    #[error("sub-script cycle detected: {0}")]
    SubScriptCycle(String),
    #[error("secret configuration is invalid: {0}")]
    InvalidSecret(String),
    #[error(transparent)]
    Version(#[from] VersionCompatibilityError),
}

#[cfg(test)]
mod tests;
