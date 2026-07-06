//! Shared runner orchestration used by CLI, daemon, and desktop shells.

mod config;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use baudbound_actions::{HeadlessActionHandler, SerialDeviceConfig as ActionSerialDeviceConfig};
use baudbound_runtime::{
    execute_manual_program_with_actions, execute_trigger_program_with_actions,
};
use baudbound_script::{
    PackageLoadError, PackageSummary, RiskLevel, ScriptPackage, load_script_package,
};
use baudbound_security::{PermissionValidationError, RunnerPolicy, validate_program_permissions};
use baudbound_storage::{
    ApproveScriptRequest, ImportScriptRequest, InstalledScript, RunLogEntry, ScriptApproval,
    ScriptStore, StorageError, StoredRunRecord,
};
use serde_json::Value;
use thiserror::Error;

pub use baudbound_runtime::RunReport;
pub use baudbound_triggers::{TriggerDispatcher, TriggerEvent, TriggerRegistration};
pub use config::{
    DEFAULT_WEBHOOK_BIND, DEFAULT_WEBHOOK_MAX_BODY_BYTES, DEFAULT_WEBHOOK_PORT, RunnerConfig,
    RunnerConfigError, RunnerSettings, SerialDeviceSettings, SerialSettings, TriggerSettings,
    WebhookSettings,
};

#[derive(Debug, Clone)]
pub struct RunnerCore {
    pub name: String,
    serial_devices: Vec<ActionSerialDeviceConfig>,
}

impl Default for RunnerCore {
    fn default() -> Self {
        Self {
            name: "BaudBound Runner".to_owned(),
            serial_devices: Vec::new(),
        }
    }
}

impl RunnerCore {
    #[must_use]
    pub fn from_config(config: &RunnerConfig) -> Self {
        Self {
            name: config.runner_name(),
            serial_devices: action_serial_devices_from_config(config),
        }
    }

    pub fn inspect_package(&self, path: impl AsRef<Path>) -> Result<PackageInspection, CoreError> {
        let package = load_script_package(path)?;
        Ok(PackageInspection::from_package(package))
    }

    pub fn validate_package(&self, path: impl AsRef<Path>) -> Result<PackageSummary, CoreError> {
        let package = load_script_package(path)?;
        Ok(package.summary())
    }

    pub fn import_package(
        &self,
        store: &impl ScriptStore,
        path: impl AsRef<Path>,
    ) -> Result<InstalledScript, CoreError> {
        let path = path.as_ref();
        let package = load_script_package(path)?;
        validate_package_permissions(&package, &RunnerPolicy::permissive())?;
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
        validate_package_permissions(&package, &RunnerPolicy::permissive())?;
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
        validate_package_permissions(&package, &RunnerPolicy::permissive())?;
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
        let installed = store.verify_script_package_hash(reference)?;
        let package = load_script_package(&installed.package_path)?;
        let policy = if has_current_approval(store, &installed, &package)? {
            RunnerPolicy::permissive()
        } else {
            RunnerPolicy::default()
        };
        if let Err(source) = validate_package_permissions(&package, &policy) {
            append_failed_run_record(store, &package, trigger_node_id, source.to_string())?;
            return Err(CoreError::Security(source));
        }
        let action_handler =
            HeadlessActionHandler::from_serial_devices(self.serial_devices.clone());
        let report = match trigger_node_id {
            Some(trigger_node_id) => execute_trigger_program_with_actions(
                &package.program,
                &package.manifest.id,
                trigger_node_id,
                trigger_payload,
                &action_handler,
            ),
            None => execute_manual_program_with_actions(
                &package.program,
                &package.manifest.id,
                &action_handler,
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
}

fn action_serial_devices_from_config(config: &RunnerConfig) -> Vec<ActionSerialDeviceConfig> {
    serial_device_configs_from_settings(&config.serial.devices)
        .into_iter()
        .map(|device| ActionSerialDeviceConfig {
            auto_reconnect: device.auto_reconnect,
            baud_rate: device.baud_rate,
            data_bits: device.data_bits,
            device_id: device.device_id,
            flow_control: device.flow_control,
            parity: device.parity,
            port: device.port,
            product_id: device.product_id,
            read_mode: device.read_mode,
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
                baud_rate: settings.baud_rate,
                data_bits: settings.data_bits,
                device_id: device_id.to_owned(),
                flow_control: settings.flow_control.clone(),
                parity: settings.parity.clone(),
                port: port.to_owned(),
                product_id: settings.product_id.clone(),
                read_mode: settings.read_mode.clone(),
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
    pub baud_rate: u32,
    pub data_bits: u8,
    pub device_id: String,
    pub flow_control: String,
    pub parity: String,
    pub port: String,
    pub product_id: Option<String>,
    pub read_mode: String,
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
    };

    use baudbound_script::{Capabilities, Manifest, Permissions};
    use baudbound_storage::FilesystemScriptStore;
    use serde_json::json;
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    use super::*;

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
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (path, content) in [
            (
                "manifest.json",
                r#"{
                    "format_version": 1,
                    "script_language_version": 1,
                    "id": "network-trigger",
                    "name": "network-trigger",
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
                            },
                            {
                                "id": "n-webhook",
                                "action_type": "trigger.webhook",
                                "type": "webhook",
                                "config": {"method": "POST", "hookName": "hook"},
                                "runtime_outputs": []
                            }
                        ],
                        "program": {"type": "block", "steps": [], "edges": []}
                    }
                }"#,
            ),
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
}
