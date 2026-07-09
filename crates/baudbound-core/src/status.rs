use std::collections::BTreeSet;

use baudbound_script::ScriptPackage;
use baudbound_storage::{InstalledScript, ScriptApproval, ScriptStore};
use baudbound_triggers::TriggerRegistration;
use serde::Serialize;
use serde_json::Value;

use crate::CoreError;

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
    pub(crate) fn from_scripts(
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

pub(crate) fn has_current_approval(
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

    let approved = permission_set(&approval.approved_permissions);
    let declared = permission_set(&package.permissions.declared_permissions);

    Ok(approved == declared)
}

pub(crate) fn approval_status_from_package(
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

    let approved = permission_set(&approval.approved_permissions);
    let declared = permission_set(&package.permissions.declared_permissions);

    if approved == declared {
        ApprovalStatus::Current
    } else {
        ApprovalStatus::PermissionMismatch
    }
}

fn permission_set(permissions: &[String]) -> BTreeSet<&str> {
    permissions.iter().map(String::as_str).collect()
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
