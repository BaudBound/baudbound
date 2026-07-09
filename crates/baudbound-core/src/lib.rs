//! Shared runner orchestration used by CLI, daemon, and desktop shells.

mod compatibility;
mod config;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use baudbound_actions::{
    HeadlessActionHandler, SerialDeviceConfig as ActionSerialDeviceConfig, WebSocketMessageSink,
};
use baudbound_runtime::{
    RuntimeActionError, RuntimeActionHandler, RuntimeActionRequest, RuntimeActionResult,
    execute_manual_program_with_actions_and_package_path,
    execute_trigger_program_with_actions_and_package_path,
};
use baudbound_script::{
    PackageLoadError, PackageSummary, RiskLevel, ScriptPackage, load_script_package,
};
use baudbound_security::{PermissionValidationError, RunnerPolicy, validate_program_permissions};
use baudbound_storage::{
    ApproveScriptRequest, ImportScriptRequest, InstalledScript, RunLogEntry, ScriptApproval,
    ScriptStore, StorageError, StoredRunRecord,
};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use compatibility::{
    CompatibilityError, default_host_target_runtime_names, runner_target_runtime_names,
    validate_package_for_runner,
};

pub use baudbound_runtime::RunReport;
pub use baudbound_triggers::{TriggerDispatcher, TriggerEvent, TriggerRegistration};
pub use compatibility::{DESKTOP_ONLY_ACTIONS, WINDOWS_DESKTOP_ONLY_ACTIONS};
pub use config::{
    DEFAULT_TRIGGER_RELOAD_SECONDS, DEFAULT_WEBHOOK_BIND, DEFAULT_WEBHOOK_MAX_BODY_BYTES,
    DEFAULT_WEBHOOK_PORT, DEFAULT_WEBSOCKET_BIND, DEFAULT_WEBSOCKET_MAX_MESSAGE_BYTES,
    DEFAULT_WEBSOCKET_PORT, RunnerConfig, RunnerConfigError, RunnerSettings, SerialDeviceSettings,
    SerialSettings, TriggerSettings, WebSocketSettings, WebhookSettings,
};

#[derive(Clone)]
pub struct RunnerCore {
    pub name: String,
    action_handler: Option<Arc<dyn RuntimeActionHandler>>,
    serial_devices: Vec<ActionSerialDeviceConfig>,
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
        store
            .update_script(import_request_from_package(path, package))
            .map_err(CoreError::Storage)
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
            self.validate_package_for_runner(&package)?;
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
        self.run_installed_with_trigger(
            store,
            &event.script_id,
            Some(&event.node_id),
            event.payload,
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
        self.run_installed_with_trigger_in_stack(
            store,
            reference,
            trigger_node_id,
            trigger_payload,
            Vec::new(),
        )
    }

    fn run_installed_with_trigger_in_stack(
        &self,
        store: &impl ScriptStore,
        reference: &str,
        trigger_node_id: Option<&str>,
        trigger_payload: serde_json::Value,
        mut call_stack: Vec<String>,
    ) -> Result<RunReport, CoreError> {
        let installed = store.verify_script_package_hash(reference)?;
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
        if let Err(source) = self.validate_package_for_runner(&package) {
            append_failed_run_record(store, &package, trigger_node_id, source.to_string())?;
            return Err(CoreError::Compatibility(source));
        }
        let policy = if has_current_approval(store, &installed, &package)? {
            RunnerPolicy::permissive()
        } else {
            RunnerPolicy::default()
        };
        if let Err(source) = validate_package_permissions(&package, &policy) {
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
        let core_action_handler = CoreRuntimeActionHandler {
            call_stack,
            core: self,
            delegate: action_handler,
            store,
        };

        let report = match trigger_node_id {
            Some(trigger_node_id) => execute_trigger_program_with_actions_and_package_path(
                &package.program,
                &package.manifest.id,
                trigger_node_id,
                Some(installed.package_path.clone()),
                trigger_payload,
                &core_action_handler,
            ),
            None => execute_manual_program_with_actions_and_package_path(
                &package.program,
                &package.manifest.id,
                Some(installed.package_path.clone()),
                &core_action_handler,
            ),
        }
        .map_err(|source| {
            let message = source.to_string();
            if let Err(error) = append_failed_run_record(store, &package, trigger_node_id, message)
            {
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
                    if let Err(error) = self.validate_package_for_runner(&package) {
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

    fn validate_loaded_package(
        &self,
        package: &ScriptPackage,
        policy: &RunnerPolicy,
    ) -> Result<(), CoreError> {
        self.validate_package_for_runner(package)?;
        validate_package_permissions(package, policy)?;
        Ok(())
    }
}

struct CoreRuntimeActionHandler<'a, S: ScriptStore> {
    call_stack: Vec<String>,
    core: &'a RunnerCore,
    delegate: &'a dyn RuntimeActionHandler,
    store: &'a S,
}

impl<S: ScriptStore> RuntimeActionHandler for CoreRuntimeActionHandler<'_, S> {
    fn execute_action(
        &self,
        request: &RuntimeActionRequest,
        context: &baudbound_runtime::RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        if request.action_type == "action.script.run" {
            return self.execute_sub_script(request);
        }

        self.delegate.execute_action(request, context)
    }
}

impl<S: ScriptStore> CoreRuntimeActionHandler<'_, S> {
    fn execute_sub_script(
        &self,
        request: &RuntimeActionRequest,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        let script = required_action_config_string(request, "script")?;
        let report = self
            .core
            .run_installed_with_trigger_in_stack(
                self.store,
                &script,
                None,
                serde_json::Value::Null,
                self.call_stack.clone(),
            )
            .map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("sub-script {script:?} failed: {source}"),
            })?;

        Ok(RuntimeActionResult {
            output_data: serde_json::Map::from_iter([
                ("status".to_owned(), Value::String("completed".to_owned())),
                ("exit_code".to_owned(), Value::Number(0.into())),
                ("run_id".to_owned(), Value::String(report.identity.run_id)),
                (
                    "script_id".to_owned(),
                    Value::String(report.identity.script_id),
                ),
                (
                    "trigger_node_id".to_owned(),
                    Value::String(report.identity.trigger_node_id),
                ),
            ]),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RunnerStatus {
    pub disabled_script_count: usize,
    pub enabled_script_count: usize,
    pub problem_count: usize,
    pub runner_name: String,
    pub scripts: Vec<ScriptStatus>,
    pub supported_target_runtimes: Vec<String>,
    pub total_script_count: usize,
    pub trigger_count: usize,
}

impl RunnerStatus {
    fn from_scripts(
        runner_name: String,
        supported_target_runtimes: Vec<String>,
        scripts: Vec<ScriptStatus>,
    ) -> Self {
        let enabled_script_count = scripts
            .iter()
            .filter(|script| script.installed.enabled)
            .count();
        let trigger_count = scripts
            .iter()
            .filter(|script| script.installed.enabled)
            .map(|script| script.triggers.len())
            .sum();
        let problem_count = scripts.iter().filter(|script| script.has_problem()).count();
        Self {
            disabled_script_count: scripts.len().saturating_sub(enabled_script_count),
            enabled_script_count,
            problem_count,
            runner_name,
            total_script_count: scripts.len(),
            scripts,
            supported_target_runtimes,
            trigger_count,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScriptStatus {
    pub approval_status: ApprovalStatus,
    pub declared_permissions: Vec<String>,
    pub installed: InstalledScript,
    pub package_error: Option<String>,
    pub package_hash_status: PackageHashStatus,
    pub triggers: Vec<TriggerRegistrationStatus>,
}

impl ScriptStatus {
    #[must_use]
    pub fn has_problem(&self) -> bool {
        self.package_error.is_some()
            || !matches!(self.package_hash_status, PackageHashStatus::Valid)
            || matches!(
                self.approval_status,
                ApprovalStatus::Error { .. }
                    | ApprovalStatus::PackageUnavailable
                    | ApprovalStatus::PermissionMismatch
                    | ApprovalStatus::StalePackageHash { .. }
            )
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PackageHashStatus {
    Error { message: String },
    Mismatch { actual: String, expected: String },
    Valid,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ApprovalStatus {
    Current,
    Error {
        message: String,
    },
    Missing,
    PackageUnavailable,
    PermissionMismatch,
    StalePackageHash {
        approved_package_hash: String,
        installed_package_hash: String,
    },
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct TriggerRegistrationStatus {
    pub action_type: String,
    pub device_id: Option<String>,
    pub node_id: String,
    pub runner_type: String,
    pub target: Option<String>,
}

impl From<TriggerRegistration> for TriggerRegistrationStatus {
    fn from(registration: TriggerRegistration) -> Self {
        let device_id = serial_device_id_from_trigger_config(&registration);
        let target = trigger_target_label(&registration, device_id.as_deref());
        Self {
            action_type: registration.action_type,
            device_id,
            node_id: registration.node_id,
            runner_type: registration.runner_type,
            target,
        }
    }
}

fn serial_device_id_from_trigger_config(registration: &TriggerRegistration) -> Option<String> {
    if registration.action_type != "trigger.serial_input" {
        return None;
    }

    registration
        .config
        .get("deviceId")
        .or_else(|| registration.config.get("device_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn trigger_target_label(
    registration: &TriggerRegistration,
    serial_device_id: Option<&str>,
) -> Option<String> {
    match registration.action_type.as_str() {
        "trigger.serial_input" => serial_device_id.map(ToOwned::to_owned),
        "trigger.webhook" => {
            let method = registration
                .config
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("POST");
            let hook_name = registration
                .config
                .get("hookName")
                .and_then(Value::as_str)
                .unwrap_or("");
            Some(format!("{method} /events/{hook_name}"))
        }
        "trigger.websocket" => registration
            .config
            .get("path")
            .or_else(|| registration.config.get("route"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        "trigger.hotkey" => registration
            .config
            .get("hotkey")
            .or_else(|| registration.config.get("key"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        "trigger.file_watch" => registration
            .config
            .get("path")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        "trigger.process_started" => registration
            .config
            .get("processName")
            .or_else(|| registration.config.get("process"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        _ => None,
    }
}

fn action_serial_devices_from_config(config: &RunnerConfig) -> Vec<ActionSerialDeviceConfig> {
    serial_device_configs_from_settings(&config.serial.devices)
        .into_iter()
        .map(|device| ActionSerialDeviceConfig {
            auto_reconnect: device.auto_reconnect,
            auto_rebind_port: device.auto_rebind_port,
            baud_rate: device.baud_rate,
            data_bits: device.data_bits,
            device_id: device.device_id,
            flow_control: device.flow_control,
            manufacturer: device.manufacturer,
            parity: device.parity,
            port: device.port,
            product_id: device.product_id,
            product: device.product,
            read_mode: device.read_mode,
            serial_number: device.serial_number,
            stop_bits: device.stop_bits,
            validate_usb_identity: device.validate_usb_identity,
            vendor_id: device.vendor_id,
        })
        .collect()
}

#[must_use]
pub fn serial_device_configs_from_settings(
    devices: &BTreeMap<String, SerialDeviceSettings>,
) -> Vec<SerialDeviceConfig> {
    devices
        .iter()
        .filter_map(|(device_id, settings)| {
            let device_id = device_id.trim();
            let port = settings.port.trim();
            if device_id.is_empty() || port.is_empty() {
                return None;
            }

            Some(SerialDeviceConfig {
                auto_reconnect: settings.auto_reconnect,
                auto_rebind_port: settings.auto_rebind_port,
                baud_rate: settings.baud_rate,
                data_bits: settings.data_bits,
                device_id: device_id.to_owned(),
                flow_control: settings.flow_control.clone(),
                manufacturer: settings.manufacturer.clone(),
                parity: settings.parity.clone(),
                port: port.to_owned(),
                product_id: settings.product_id.clone(),
                product: settings.product.clone(),
                read_mode: settings.read_mode.clone(),
                serial_number: settings.serial_number.clone(),
                stop_bits: settings.stop_bits.clone(),
                validate_usb_identity: settings.validate_usb_identity,
                vendor_id: settings.vendor_id.clone(),
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialDeviceConfig {
    pub auto_reconnect: bool,
    pub auto_rebind_port: bool,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub device_id: String,
    pub flow_control: String,
    pub manufacturer: Option<String>,
    pub parity: String,
    pub port: String,
    pub product_id: Option<String>,
    pub product: Option<String>,
    pub read_mode: String,
    pub serial_number: Option<String>,
    pub stop_bits: String,
    pub validate_usb_identity: bool,
    pub vendor_id: Option<String>,
}

pub struct CoreTriggerDispatcher<'core, S: ScriptStore> {
    core: &'core RunnerCore,
    store: &'core S,
}

impl<S: ScriptStore> TriggerDispatcher for CoreTriggerDispatcher<'_, S> {
    fn dispatch(&self, event: TriggerEvent) -> Result<RunReport, baudbound_triggers::TriggerError> {
        let script_id = event.script_id.clone();
        let node_id = event.node_id.clone();
        self.core
            .dispatch_trigger_event(self.store, event)
            .map_err(|source| {
                baudbound_triggers::TriggerError::Failed(
                    format!("{script_id}:{node_id}"),
                    source.to_string(),
                )
            })
    }
}

fn has_current_approval(
    store: &impl ScriptStore,
    installed: &InstalledScript,
    package: &ScriptPackage,
) -> Result<bool, CoreError> {
    let Some(approval) = store.find_script_approval(&installed.id)? else {
        return Ok(false);
    };
    if approval.package_hash != installed.package_hash {
        return Ok(false);
    }

    let approved = approval
        .approved_permissions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let declared = package
        .permissions
        .declared_permissions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    Ok(approved == declared)
}

fn approval_status_from_package(
    installed: &InstalledScript,
    package: Option<&ScriptPackage>,
    package_loaded: bool,
    approval: &ScriptApproval,
) -> ApprovalStatus {
    if approval.package_hash != installed.package_hash {
        return ApprovalStatus::StalePackageHash {
            approved_package_hash: approval.package_hash.clone(),
            installed_package_hash: installed.package_hash.clone(),
        };
    }

    let Some(package) = package else {
        return if package_loaded {
            ApprovalStatus::PermissionMismatch
        } else {
            ApprovalStatus::PackageUnavailable
        };
    };

    let approved = approval
        .approved_permissions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let declared = package
        .permissions
        .declared_permissions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    if approved == declared {
        ApprovalStatus::Current
    } else {
        ApprovalStatus::PermissionMismatch
    }
}

#[derive(Debug, Clone)]
pub struct PackageInspection {
    pub entries: Vec<String>,
    pub summary: PackageSummary,
}

impl PackageInspection {
    fn from_package(package: ScriptPackage) -> Self {
        Self {
            entries: package
                .entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect(),
            summary: package.summary(),
        }
    }
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Compatibility(#[from] CompatibilityError),
    #[error("program trigger registration failed: {0}")]
    InvalidTriggerRegistration(String),
    #[error(transparent)]
    Package(#[from] PackageLoadError),
    #[error(transparent)]
    Runtime(#[from] baudbound_runtime::RuntimeError),
    #[error(transparent)]
    Security(#[from] PermissionValidationError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("sub-script cycle detected: {0}")]
    SubScriptCycle(String),
}

fn trigger_registrations_from_package(
    installed: &InstalledScript,
    package: &ScriptPackage,
) -> Result<Vec<TriggerRegistration>, CoreError> {
    let entry = package
        .program
        .get("entry")
        .and_then(Value::as_object)
        .ok_or_else(|| CoreError::InvalidTriggerRegistration("missing entry".to_owned()))?;

    let mut trigger_values = Vec::new();
    if let Some(trigger) = entry.get("trigger") {
        trigger_values.push(trigger);
    }
    if let Some(triggers) = entry.get("triggers").and_then(Value::as_array) {
        trigger_values.extend(triggers);
    }

    let mut seen_node_ids = BTreeSet::new();
    let mut registrations = Vec::new();
    for trigger in trigger_values {
        let action_type = trigger
            .get("action_type")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CoreError::InvalidTriggerRegistration("trigger is missing action_type".to_owned())
            })?;
        if !action_type.starts_with("trigger.") {
            return Err(CoreError::InvalidTriggerRegistration(format!(
                "{action_type} is not a trigger action_type"
            )));
        }

        let node_id = trigger.get("id").and_then(Value::as_str).ok_or_else(|| {
            CoreError::InvalidTriggerRegistration("trigger is missing id".to_owned())
        })?;
        if !seen_node_ids.insert(node_id.to_owned()) {
            continue;
        }

        let runner_type = trigger
            .get("type")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| action_type.trim_start_matches("trigger.").to_owned());
        let config = trigger
            .get("config")
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));

        registrations.push(TriggerRegistration {
            action_type: action_type.to_owned(),
            config,
            node_id: node_id.to_owned(),
            runner_type,
            script_id: installed.id.clone(),
            script_name: installed.name.clone(),
        });
    }

    Ok(registrations)
}

fn required_action_config_string(
    request: &RuntimeActionRequest,
    key: &str,
) -> Result<String, RuntimeActionError> {
    request
        .config
        .get(key)
        .map(action_config_value_to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("missing required config field {key}"),
        })
}

fn action_config_value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn risk_level_name(risk_level: &RiskLevel) -> &'static str {
    match risk_level {
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
        RiskLevel::Dangerous => "dangerous",
    }
}

fn security_risk_level(risk_level: &RiskLevel) -> baudbound_security::RiskLevel {
    match risk_level {
        RiskLevel::Low => baudbound_security::RiskLevel::Low,
        RiskLevel::Medium => baudbound_security::RiskLevel::Medium,
        RiskLevel::High => baudbound_security::RiskLevel::High,
        RiskLevel::Dangerous => baudbound_security::RiskLevel::Dangerous,
    }
}

fn validate_package_permissions(
    package: &ScriptPackage,
    policy: &RunnerPolicy,
) -> Result<(), PermissionValidationError> {
    validate_program_permissions(
        &package.program,
        &package.permissions.declared_permissions,
        security_risk_level(&package.permissions.risk_level),
        policy,
    )
    .map(|_| ())
}

fn stored_run_record_from_report(report: &RunReport) -> StoredRunRecord {
    StoredRunRecord {
        completed_at_unix: current_unix_timestamp(),
        logs: report
            .logs
            .iter()
            .map(|log| RunLogEntry {
                level: log.level.clone(),
                message: log.message.clone(),
                node_id: log.node_id.clone(),
            })
            .collect(),
        run_id: report.identity.run_id.clone(),
        script_id: report.identity.script_id.clone(),
        status: "completed".to_owned(),
        trigger_node_id: report.identity.trigger_node_id.clone(),
        variables: report.variables.clone(),
    }
}

fn append_failed_run_record(
    store: &impl ScriptStore,
    package: &ScriptPackage,
    selected_trigger_node_id: Option<&str>,
    message: String,
) -> Result<(), StorageError> {
    store.append_run_record(failed_run_record(
        package,
        selected_trigger_node_id,
        message,
    ))
}

fn failed_run_record(
    package: &ScriptPackage,
    selected_trigger_node_id: Option<&str>,
    message: String,
) -> StoredRunRecord {
    let trigger_node_id = selected_trigger_node_id
        .map(ToOwned::to_owned)
        .or_else(|| trigger_node_id(&package.program))
        .unwrap_or_else(|| "unknown".to_owned());
    StoredRunRecord {
        completed_at_unix: current_unix_timestamp(),
        logs: vec![RunLogEntry {
            level: "error".to_owned(),
            message,
            node_id: None,
        }],
        run_id: create_run_id(&package.manifest.id, &trigger_node_id),
        script_id: package.manifest.id.clone(),
        status: "failed".to_owned(),
        trigger_node_id,
        variables: Default::default(),
    }
}

fn trigger_node_id(program: &serde_json::Value) -> Option<String> {
    program
        .get("entry")?
        .get("trigger")?
        .get("id")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn create_run_id(script_id: &str, trigger_node_id: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{script_id}:{trigger_node_id}:{timestamp}")
}

fn import_request_from_package(path: &Path, package: ScriptPackage) -> ImportScriptRequest {
    let summary = package.summary();
    ImportScriptRequest {
        id: package.manifest.id,
        name: summary.script_name,
        package_source: path.to_path_buf(),
        package_format_version: summary.package_format_version,
        script_language_version: summary.script_language_version,
        target_runtime: summary.target_runtime,
        asset_count: summary.asset_count,
        risk_level: risk_level_name(&package.permissions.risk_level).to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{Cursor, Write},
        sync::Mutex,
    };

    use baudbound_runtime::{
        RuntimeActionError, RuntimeActionHandler, RuntimeActionRequest, RuntimeActionResult,
        RuntimeContext,
    };
    use baudbound_script::{Capabilities, Manifest, Permissions};
    use baudbound_storage::FilesystemScriptStore;
    use serde_json::{Map, Value, json};
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    use super::*;

    #[derive(Default)]
    struct RecordingActionHandler {
        actions: Mutex<Vec<String>>,
    }

    impl RuntimeActionHandler for RecordingActionHandler {
        fn execute_action(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<RuntimeActionResult, RuntimeActionError> {
            self.actions
                .lock()
                .expect("recording action lock should not be poisoned")
                .push(request.action_type.clone());
            Ok(RuntimeActionResult {
                output_data: Map::from_iter([("handled".to_owned(), Value::Bool(true))]),
            })
        }
    }

    #[test]
    fn creates_failed_run_record_with_package_identity() {
        let package = ScriptPackage {
            capabilities: Capabilities {
                required_capabilities: Vec::new(),
                target_runtime: "Generic Desktop".to_owned(),
            },
            editor: None,
            entries: Vec::new(),
            manifest: Manifest {
                format_version: 1,
                script_language_version: 1,
                id: "script-1".to_owned(),
                name: "Script One".to_owned(),
                description: String::new(),
                author: String::new(),
                website: String::new(),
                repository: String::new(),
                created_with: "test".to_owned(),
                created_at: "2026-01-01T00:00:00.000Z".to_owned(),
                updated_at: String::new(),
                tags: Vec::new(),
                minimum_runner_version: "0.1.0".to_owned(),
                assets: Vec::new(),
            },
            permissions: Permissions {
                declared_permissions: Vec::new(),
                risk_level: RiskLevel::Low,
            },
            program: json!({
                "entry": {
                    "trigger": {
                        "id": "n-trigger",
                        "action_type": "trigger.manual",
                        "type": "manual",
                        "config": {},
                        "runtime_outputs": []
                    },
                    "triggers": [],
                    "program": {
                        "steps": [],
                        "edges": []
                    }
                }
            }),
        };

        let record = failed_run_record(&package, None, "permission denied".to_owned());

        assert_eq!(record.script_id, "script-1");
        assert_eq!(record.trigger_node_id, "n-trigger");
        assert_eq!(record.status, "failed");
        assert!(record.run_id.starts_with("script-1:n-trigger:"));
        assert_eq!(record.logs[0].message, "permission denied");
    }

    #[test]
    fn current_script_approval_allows_policy_blocked_permissions() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("network-trigger.bbs");
        fs::write(&package_path, create_policy_test_package())
            .expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let core = RunnerCore::default();
        core.import_package(&store, &package_path)
            .expect("package should import");

        let unapproved = core
            .run_installed(&store, "network-trigger")
            .expect_err("unapproved network trigger should be blocked");
        assert!(matches!(unapproved, CoreError::Security(_)));

        let approval = core
            .approve_installed(&store, "network-trigger")
            .expect("package should approve");
        assert_eq!(approval.approved_permissions, ["webhook_public_bind"]);

        let report = core
            .run_installed(&store, "network-trigger")
            .expect("approved package should run");
        assert_eq!(report.identity.script_id, "network-trigger");
    }

    #[test]
    fn installed_package_lifecycle_uses_real_bbs_packages() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("network-trigger.bbs");
        let updated_package_path = temporary_directory
            .path()
            .join("network-trigger-updated.bbs");
        fs::write(
            &package_path,
            create_policy_test_package_with_webhook("network-trigger", "hook"),
        )
        .expect("test package should be written");
        fs::write(
            &updated_package_path,
            create_policy_test_package_with_webhook("network-trigger-updated", "updated-hook"),
        )
        .expect("updated test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let core = RunnerCore::default();

        let imported = core
            .import_package(&store, &package_path)
            .expect("package should import");
        assert_eq!(imported.id, "network-trigger");
        assert_eq!(imported.package_file_name, "network-trigger.bbs");
        assert!(imported.package_path.exists());
        assert!(store.verify_script_package_hash("network-trigger").is_ok());

        let blocked = core
            .run_installed(&store, "network-trigger")
            .expect_err("unapproved high-risk package should be blocked");
        assert!(matches!(blocked, CoreError::Security(_)));
        assert_eq!(
            store
                .list_run_records(Some("network-trigger"), None)
                .expect("failed run record should list")
                .first()
                .expect("failed run record should exist")
                .status,
            "failed"
        );

        core.approve_installed(&store, "network-trigger")
            .expect("package should approve");
        let report = core
            .dispatch_trigger_event(
                &store,
                TriggerEvent {
                    node_id: "n-webhook".to_owned(),
                    payload: json!({"body": "hello from lifecycle test"}),
                    script_id: "network-trigger".to_owned(),
                },
            )
            .expect("approved trigger should run");
        assert_eq!(report.identity.trigger_node_id, "n-webhook");
        assert_eq!(
            report.variables.get("n-webhook.body"),
            Some(&json!("hello from lifecycle test"))
        );

        let run_records = store
            .list_run_records(Some("network-trigger"), None)
            .expect("run records should list");
        assert_eq!(
            run_records
                .iter()
                .map(|record| record.status.as_str())
                .collect::<Vec<_>>(),
            ["completed", "failed"]
        );

        let updated = core
            .update_package(&store, &updated_package_path)
            .expect("installed package should update");
        assert_eq!(updated.id, "network-trigger");
        assert_eq!(updated.name, "network-trigger-updated");
        assert_eq!(updated.package_file_name, "network-trigger-updated.bbs");
        assert!(!imported.package_path.exists());
        assert!(updated.package_path.exists());
        assert!(
            store
                .find_script_approval("network-trigger")
                .expect("approval lookup should succeed")
                .is_none()
        );

        let registrations = core
            .list_trigger_registrations(&store, Some("network-trigger"))
            .expect("updated trigger registrations should list");
        let webhook = registrations
            .iter()
            .find(|registration| registration.node_id == "n-webhook")
            .expect("webhook registration should exist");
        assert_eq!(webhook.config["hookName"], "updated-hook");

        core.remove_installed(&store, "network-trigger")
            .expect("installed package should remove");
        assert!(
            core.list_installed(&store)
                .expect("installed scripts should list")
                .is_empty()
        );
        assert!(!updated.package_path.exists());
    }

    #[test]
    fn custom_action_handler_is_used_for_script_execution() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("action-handler-test.bbs");
        fs::write(&package_path, create_action_handler_test_package())
            .expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let handler = Arc::new(RecordingActionHandler::default());
        let core = RunnerCore::default().with_action_handler(handler.clone());
        core.import_package(&store, &package_path)
            .expect("package should import");
        let report = core
            .run_installed(&store, "action-handler-test")
            .expect("script should run with injected action handler");

        assert_eq!(
            report.variables.get("n-format.handled"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            handler
                .actions
                .lock()
                .expect("recording action lock should not be poisoned")
                .as_slice(),
            &["action.text.format".to_owned()]
        );
    }

    #[test]
    fn import_rejects_desktop_actions_for_headless_target_runtime() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("headless-notification.bbs");
        fs::write(
            &package_path,
            create_target_runtime_test_package(
                "headless-notification",
                "Generic Headless",
                "action.notification",
            ),
        )
        .expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let error = RunnerCore::default()
            .import_package(&store, &package_path)
            .expect_err("desktop-only action should not import into headless target");

        assert!(
            error
                .to_string()
                .contains("requires a desktop target runtime"),
            "{error}"
        );
    }

    #[test]
    fn import_rejects_windows_only_actions_for_non_windows_desktop_target_runtime() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("linux-pixel.bbs");
        fs::write(
            &package_path,
            create_target_runtime_test_package("linux-pixel", "Linux Desktop", "action.pixel.get"),
        )
        .expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let error = RunnerCore::default()
            .import_package(&store, &package_path)
            .expect_err("Windows-only action should not import into Linux Desktop target");

        assert!(
            error.to_string().contains("requires Windows Desktop"),
            "{error}"
        );
    }

    #[test]
    fn import_rejects_platform_specific_unsupported_action_config() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("macos-mouse-back.bbs");
        fs::write(
            &package_path,
            create_target_runtime_test_package_with_action_config(
                "macos-mouse-back",
                "macOS Desktop",
                "action.mouse",
                r#"{"button":"back","clickType":"single"}"#,
            ),
        )
        .expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let error = RunnerCore::default()
            .import_package(&store, &package_path)
            .expect_err("unsupported platform-specific mouse config should not import");

        assert!(
            error
                .to_string()
                .contains("does not have a native macOS backend"),
            "{error}"
        );
    }

    #[test]
    fn configured_runner_target_runtimes_reject_other_package_targets() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("desktop-text.bbs");
        fs::write(
            &package_path,
            create_target_runtime_test_package(
                "desktop-text",
                "Generic Desktop",
                "action.text.format",
            ),
        )
        .expect("test package should be written");
        let config = RunnerConfig {
            runner: RunnerSettings {
                name: Some("Headless Test Runner".to_owned()),
                target_runtimes: vec!["Generic Headless".to_owned()],
                trigger_reload_seconds: DEFAULT_TRIGGER_RELOAD_SECONDS,
            },
            ..RunnerConfig::default()
        };

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let error = RunnerCore::from_config(&config)
            .import_package(&store, &package_path)
            .expect_err("headless-only runner should reject desktop package target");

        assert!(
            error
                .to_string()
                .contains("this runner supports only Generic Headless"),
            "{error}"
        );
    }

    #[test]
    fn sub_script_action_runs_installed_manual_script() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let child_package_path = temporary_directory.path().join("child-script.bbs");
        let parent_package_path = temporary_directory.path().join("parent-script.bbs");
        fs::write(&child_package_path, create_action_handler_test_package())
            .expect("child test package should be written");
        fs::write(
            &parent_package_path,
            create_sub_script_parent_package("parent-script", "action-handler-test"),
        )
        .expect("parent test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let core = RunnerCore::default();
        core.import_package(&store, &child_package_path)
            .expect("child package should import");
        core.import_package(&store, &parent_package_path)
            .expect("parent package should import");
        core.approve_installed(&store, "parent-script")
            .expect("sub-script parent should approve");

        let report = core
            .run_installed(&store, "parent-script")
            .expect("parent should run sub-script");

        assert_eq!(
            report.variables.get("n-sub.status"),
            Some(&json!("completed"))
        );
        assert_eq!(report.variables.get("n-sub.exit_code"), Some(&json!(0)));
        assert_eq!(
            report.variables.get("n-sub.script_id"),
            Some(&json!("action-handler-test"))
        );

        let child_runs = store
            .list_run_records(Some("action-handler-test"), None)
            .expect("child run records should list");
        assert_eq!(child_runs.len(), 1);
        assert_eq!(child_runs[0].status, "completed");
    }

    #[test]
    fn sub_script_action_rejects_recursive_cycle() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("recursive-script.bbs");
        fs::write(
            &package_path,
            create_sub_script_parent_package("recursive-script", "recursive-script"),
        )
        .expect("recursive test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let core = RunnerCore::default();
        core.import_package(&store, &package_path)
            .expect("recursive package should import");
        core.approve_installed(&store, "recursive-script")
            .expect("recursive package should approve");

        let error = core
            .run_installed(&store, "recursive-script")
            .expect_err("recursive sub-script should fail");

        assert!(error.to_string().contains("sub-script cycle detected"));
        let runs = store
            .list_run_records(Some("recursive-script"), None)
            .expect("failed run record should list");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "failed");
    }

    #[test]
    fn lists_trigger_registrations_for_installed_scripts() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("network-trigger.bbs");
        fs::write(&package_path, create_policy_test_package())
            .expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let core = RunnerCore::default();
        core.import_package(&store, &package_path)
            .expect("package should import");

        let registrations = core
            .list_trigger_registrations(&store, Some("network-trigger"))
            .expect("trigger registrations should list");

        assert_eq!(registrations.len(), 2);
        assert!(
            registrations
                .iter()
                .any(|registration| registration.node_id == "n-manual"
                    && registration.action_type == "trigger.manual")
        );
        let webhook = registrations
            .iter()
            .find(|registration| registration.node_id == "n-webhook")
            .expect("webhook trigger should be registered");
        assert_eq!(webhook.runner_type, "webhook");
        assert_eq!(webhook.config["hookName"], "hook");
    }

    #[test]
    fn disabled_scripts_are_omitted_from_global_trigger_registrations() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("network-trigger.bbs");
        fs::write(&package_path, create_policy_test_package())
            .expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let core = RunnerCore::default();
        core.import_package(&store, &package_path)
            .expect("package should import");

        assert!(
            !core
                .list_trigger_registrations(&store, None)
                .expect("enabled trigger registrations should list")
                .is_empty()
        );

        let disabled = core
            .set_installed_enabled(&store, "network-trigger", false)
            .expect("script should disable");
        assert!(!disabled.enabled);

        assert!(
            core.list_trigger_registrations(&store, None)
                .expect("global trigger registrations should list")
                .is_empty()
        );
        assert!(
            !core
                .list_trigger_registrations(&store, Some("network-trigger"))
                .expect("direct trigger registrations should list")
                .is_empty()
        );
    }

    #[test]
    fn status_reports_script_health_and_approval_state() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("network-trigger.bbs");
        fs::write(&package_path, create_policy_test_package())
            .expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let core = RunnerCore::default();
        core.import_package(&store, &package_path)
            .expect("package should import");

        let status = core.status(&store).expect("status should build");
        assert_eq!(status.runner_name, "BaudBound Runner");
        assert!(
            status
                .supported_target_runtimes
                .contains(&"Generic Desktop".to_owned())
        );
        assert_eq!(status.total_script_count, 1);
        assert_eq!(status.enabled_script_count, 1);
        assert_eq!(status.disabled_script_count, 0);
        assert_eq!(status.problem_count, 0);
        assert_eq!(status.trigger_count, 2);
        assert!(matches!(
            status.scripts[0].package_hash_status,
            PackageHashStatus::Valid
        ));
        assert!(matches!(
            status.scripts[0].approval_status,
            ApprovalStatus::Missing
        ));
        assert_eq!(
            status.scripts[0].declared_permissions,
            ["webhook_public_bind"]
        );

        core.approve_installed(&store, "network-trigger")
            .expect("package should approve");
        core.set_installed_enabled(&store, "network-trigger", false)
            .expect("script should disable");

        let status = core.status(&store).expect("status should build");
        assert_eq!(status.enabled_script_count, 0);
        assert_eq!(status.disabled_script_count, 1);
        assert_eq!(status.trigger_count, 0);
        assert!(matches!(
            status.scripts[0].approval_status,
            ApprovalStatus::Current
        ));
    }

    #[test]
    fn dispatches_trigger_event_through_core_dispatcher() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("network-trigger.bbs");
        fs::write(&package_path, create_policy_test_package())
            .expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let core = RunnerCore::default();
        core.import_package(&store, &package_path)
            .expect("package should import");
        core.approve_installed(&store, "network-trigger")
            .expect("package should approve");

        let report = core
            .trigger_dispatcher(&store)
            .dispatch(TriggerEvent {
                node_id: "n-webhook".to_owned(),
                payload: json!({"body": "hello"}),
                script_id: "network-trigger".to_owned(),
            })
            .expect("trigger event should dispatch");

        assert_eq!(report.identity.script_id, "network-trigger");
        assert_eq!(report.identity.trigger_node_id, "n-webhook");
        assert_eq!(
            report.variables.get("n-webhook.body"),
            Some(&json!("hello"))
        );
    }

    fn create_policy_test_package() -> Vec<u8> {
        create_policy_test_package_with_webhook("network-trigger", "hook")
    }

    fn create_policy_test_package_with_webhook(script_name: &str, hook_name: &str) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        let manifest = format!(
            r#"{{
                    "format_version": 1,
                    "script_language_version": 1,
                    "id": "network-trigger",
                    "name": "{script_name}",
                    "created_with": "BaudBound Test",
                    "created_at": "2026-01-01T00:00:00.000Z",
                    "minimum_runner_version": "0.1.0"
                }}"#
        );
        let program = format!(
            r#"{{
                    "entry": {{
                        "trigger": {{
                            "id": "n-manual",
                            "action_type": "trigger.manual",
                            "type": "manual",
                            "config": {{}},
                            "runtime_outputs": []
                        }},
                        "triggers": [
                            {{
                                "id": "n-manual",
                                "action_type": "trigger.manual",
                                "type": "manual",
                                "config": {{}},
                                "runtime_outputs": []
                            }},
                            {{
                                "id": "n-webhook",
                                "action_type": "trigger.webhook",
                                "type": "webhook",
                                "config": {{"method": "POST", "hookName": "{hook_name}"}},
                                "runtime_outputs": []
                            }}
                        ],
                        "program": {{"type": "block", "steps": [], "edges": []}}
                    }}
                }}"#
        );

        for (path, content) in [
            ("manifest.json", manifest.as_str()),
            ("program.json", program.as_str()),
            (
                "permissions.json",
                r#"{"declared_permissions": ["webhook_public_bind"], "risk_level": "high"}"#,
            ),
            (
                "capabilities.json",
                r#"{"required_capabilities": [], "target_runtime": "Generic Desktop"}"#,
            ),
        ] {
            writer
                .start_file(path, options)
                .expect("test zip file should start");
            writer
                .write_all(content.as_bytes())
                .expect("test zip content should write");
        }

        writer
            .finish()
            .expect("test zip should finish")
            .into_inner()
    }

    fn create_sub_script_parent_package(script_id: &str, target_script: &str) -> Vec<u8> {
        let manifest = format!(
            r#"{{
                "format_version": 1,
                "script_language_version": 1,
                "id": "{script_id}",
                "name": "{script_id}",
                "created_with": "BaudBound Test",
                "created_at": "2026-01-01T00:00:00.000Z",
                "minimum_runner_version": "0.1.0"
            }}"#
        );
        let program = format!(
            r#"{{
                "entry": {{
                    "trigger": {{
                        "id": "n-manual",
                        "action_type": "trigger.manual",
                        "type": "manual",
                        "config": {{}},
                        "runtime_outputs": []
                    }},
                    "triggers": [
                        {{
                            "id": "n-manual",
                            "action_type": "trigger.manual",
                            "type": "manual",
                            "config": {{}},
                            "runtime_outputs": []
                        }}
                    ],
                    "program": {{
                        "type": "block",
                        "steps": [
                            {{
                                "id": "n-sub",
                                "action_type": "action.script.run",
                                "type": "action",
                                "action": "run_sub_script",
                                "config": {{
                                    "script": "{target_script}"
                                }},
                                "runtime_outputs": []
                            }}
                        ],
                        "edges": [
                            {{
                                "source": "n-manual",
                                "source_handle": "out",
                                "target": "n-sub",
                                "target_handle": "input"
                            }}
                        ]
                    }}
                }}
            }}"#
        );

        create_test_package([
            ("manifest.json", manifest.as_str()),
            ("program.json", program.as_str()),
            (
                "permissions.json",
                r#"{"declared_permissions": ["sub_script_run"], "risk_level": "high"}"#,
            ),
            (
                "capabilities.json",
                r#"{"required_capabilities": [], "target_runtime": "Generic Desktop"}"#,
            ),
        ])
    }

    fn create_action_handler_test_package() -> Vec<u8> {
        create_test_package([
            (
                "manifest.json",
                r#"{
                    "format_version": 1,
                    "script_language_version": 1,
                    "id": "action-handler-test",
                    "name": "action-handler-test",
                    "created_with": "BaudBound Test",
                    "created_at": "2026-01-01T00:00:00.000Z",
                    "minimum_runner_version": "0.1.0"
                }"#,
            ),
            (
                "program.json",
                r#"{
                    "entry": {
                        "trigger": {
                            "id": "n-manual",
                            "action_type": "trigger.manual",
                            "type": "manual",
                            "config": {},
                            "runtime_outputs": []
                        },
                        "triggers": [
                            {
                                "id": "n-manual",
                                "action_type": "trigger.manual",
                                "type": "manual",
                                "config": {},
                                "runtime_outputs": []
                            }
                        ],
                        "program": {
                            "type": "block",
                            "steps": [
                                {
                                    "id": "n-format",
                                    "action_type": "action.text.format",
                                    "type": "action",
                                    "action": "format_text",
                                    "config": {
                                        "operation": "uppercase",
                                        "input": "hello"
                                    },
                                    "runtime_outputs": []
                                }
                            ],
                            "edges": [
                                {
                                    "source": "n-manual",
                                    "source_handle": "out",
                                    "target": "n-format",
                                    "target_handle": "input"
                                }
                            ]
                        }
                    }
                }"#,
            ),
            (
                "permissions.json",
                r#"{"declared_permissions": ["text_transform"], "risk_level": "low"}"#,
            ),
            (
                "capabilities.json",
                r#"{"required_capabilities": [], "target_runtime": "Generic Desktop"}"#,
            ),
        ])
    }

    fn create_target_runtime_test_package(
        script_id: &str,
        target_runtime: &str,
        action_type: &str,
    ) -> Vec<u8> {
        create_target_runtime_test_package_with_action_config(
            script_id,
            target_runtime,
            action_type,
            "{}",
        )
    }

    fn create_target_runtime_test_package_with_action_config(
        script_id: &str,
        target_runtime: &str,
        action_type: &str,
        action_config: &str,
    ) -> Vec<u8> {
        let manifest = format!(
            r#"{{
                "format_version": 1,
                "script_language_version": 1,
                "id": "{script_id}",
                "name": "{script_id}",
                "created_with": "BaudBound Test",
                "created_at": "2026-01-01T00:00:00.000Z",
                "minimum_runner_version": "0.1.0"
            }}"#
        );
        let program = format!(
            r#"{{
                "entry": {{
                    "trigger": {{
                        "id": "n-manual",
                        "action_type": "trigger.manual",
                        "type": "manual",
                        "config": {{}},
                        "runtime_outputs": []
                    }},
                    "triggers": [
                        {{
                            "id": "n-manual",
                            "action_type": "trigger.manual",
                            "type": "manual",
                            "config": {{}},
                            "runtime_outputs": []
                        }}
                    ],
                    "program": {{
                        "type": "block",
                        "steps": [
                            {{
                                "id": "n-native",
                                "action_type": "{action_type}",
                                "type": "action",
                                "config": {action_config},
                                "runtime_outputs": []
                            }}
                        ],
                        "edges": []
                    }}
                }}
            }}"#
        );
        let capabilities =
            format!(r#"{{"required_capabilities": [], "target_runtime": "{target_runtime}"}}"#);

        create_test_package([
            ("manifest.json", manifest.as_str()),
            ("program.json", program.as_str()),
            (
                "permissions.json",
                r#"{"declared_permissions": [], "risk_level": "low"}"#,
            ),
            ("capabilities.json", capabilities.as_str()),
        ])
    }

    fn create_test_package<const N: usize>(files: [(&str, &str); N]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (path, content) in files {
            writer
                .start_file(path, options)
                .expect("test zip file should start");
            writer
                .write_all(content.as_bytes())
                .expect("test zip content should write");
        }

        writer
            .finish()
            .expect("test zip should finish")
            .into_inner()
    }
}
