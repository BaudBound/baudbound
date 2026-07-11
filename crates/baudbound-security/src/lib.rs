//! Security and policy primitives shared by BaudBound runner apps.

mod capabilities;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub use capabilities::{
    CapabilityValidationError, ProgramCapabilityReport, calculate_program_capabilities,
    calculate_program_capabilities_with_secrets, validate_program_capabilities,
    validate_program_capabilities_with_secrets,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Dangerous,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PermissionGrant {
    pub name: String,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Capability {
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunnerPolicy {
    pub allow_dangerous_actions: bool,
    pub allow_network_servers: bool,
    pub allow_shell_commands: bool,
}

impl RunnerPolicy {
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            allow_dangerous_actions: true,
            allow_network_servers: true,
            allow_shell_commands: true,
        }
    }
}

#[must_use]
pub fn highest_risk(permissions: &[PermissionGrant]) -> RiskLevel {
    permissions
        .iter()
        .map(|permission| permission.risk)
        .max()
        .unwrap_or(RiskLevel::Low)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramPermissionReport {
    pub required_permissions: Vec<PermissionGrant>,
    pub calculated_risk: RiskLevel,
}

#[derive(Debug, Error)]
pub enum PermissionValidationError {
    #[error("program contains unsupported executable action type {0}")]
    UnsupportedActionType(String),
    #[error("permissions.json is missing declared permission {0}")]
    MissingPermission(String),
    #[error("permissions.json declares unused permission {0}")]
    UndeclaredPermission(String),
    #[error("permissions.json declares duplicate permission {0}")]
    DuplicatePermission(String),
    #[error("permissions.json risk_level is {declared:?}, expected {expected:?}")]
    RiskMismatch {
        declared: RiskLevel,
        expected: RiskLevel,
    },
    #[error("runner policy blocks permission {permission}: {reason}")]
    PolicyBlocked { permission: String, reason: String },
    #[error("program.json has invalid shape: {0}")]
    InvalidProgram(String),
}

#[derive(Debug, Error)]
pub enum SecurityValidationError {
    #[error(transparent)]
    Capability(#[from] CapabilityValidationError),
    #[error(transparent)]
    Permission(#[from] PermissionValidationError),
}

pub fn validate_program_permissions(
    program: &Value,
    declared_permissions: &[String],
    declared_risk: RiskLevel,
    policy: &RunnerPolicy,
) -> Result<ProgramPermissionReport, PermissionValidationError> {
    validate_program_permissions_with_secrets(
        program,
        declared_permissions,
        declared_risk,
        policy,
        false,
    )
}

pub fn validate_program_permissions_with_secrets(
    program: &Value,
    declared_permissions: &[String],
    declared_risk: RiskLevel,
    policy: &RunnerPolicy,
    has_secret_declarations: bool,
) -> Result<ProgramPermissionReport, PermissionValidationError> {
    let report = calculate_program_permissions_with_secrets(program, has_secret_declarations)?;
    let required = report
        .required_permissions
        .iter()
        .map(|permission| permission.name.as_str())
        .collect::<BTreeSet<_>>();
    let declared = declared_permissions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    if declared.len() != declared_permissions.len() {
        let duplicate = first_duplicate(declared_permissions)
            .expect("set length proves that a duplicate permission exists");
        return Err(PermissionValidationError::DuplicatePermission(duplicate));
    }

    for permission in &required {
        if !declared.contains(permission) {
            return Err(PermissionValidationError::MissingPermission(
                (*permission).to_owned(),
            ));
        }
    }

    for permission in &declared {
        if !required.contains(permission) {
            return Err(PermissionValidationError::UndeclaredPermission(
                (*permission).to_owned(),
            ));
        }
    }

    if declared_risk != report.calculated_risk {
        return Err(PermissionValidationError::RiskMismatch {
            declared: declared_risk,
            expected: report.calculated_risk,
        });
    }

    enforce_runner_policy(&report.required_permissions, policy)?;
    Ok(report)
}

pub fn calculate_program_permissions(
    program: &Value,
) -> Result<ProgramPermissionReport, PermissionValidationError> {
    calculate_program_permissions_with_secrets(program, false)
}

pub fn calculate_program_permissions_with_secrets(
    program: &Value,
    has_secret_declarations: bool,
) -> Result<ProgramPermissionReport, PermissionValidationError> {
    let mut permissions = Vec::<PermissionGrant>::new();
    let mut seen_permissions = BTreeSet::<String>::new();

    for action_type in
        executable_action_types(program).map_err(PermissionValidationError::InvalidProgram)?
    {
        if action_type == "runtime.set_variable" {
            continue;
        }
        let Some(permission) = permission_for_action_type(&action_type) else {
            if !is_known_permissionless_action_type(&action_type) {
                return Err(PermissionValidationError::UnsupportedActionType(
                    action_type,
                ));
            }
            continue;
        };

        if seen_permissions.insert(permission.name.clone()) {
            permissions.push(permission);
        }
    }

    for scope in
        variable_operation_scopes(program).map_err(PermissionValidationError::InvalidProgram)?
    {
        let permission = match scope.as_str() {
            "runtime" => PermissionGrant {
                name: "set_local_variable".to_owned(),
                risk: RiskLevel::Low,
            },
            "persistent" => PermissionGrant {
                name: "set_persistent_variable".to_owned(),
                risk: RiskLevel::Medium,
            },
            "global" => PermissionGrant {
                name: "set_global_variable".to_owned(),
                risk: RiskLevel::High,
            },
            invalid => {
                return Err(PermissionValidationError::InvalidProgram(format!(
                    "runtime.set_variable contains unsupported scope {invalid:?}"
                )));
            }
        };
        if seen_permissions.insert(permission.name.clone()) {
            permissions.push(permission);
        }
    }

    if has_secret_declarations && seen_permissions.insert("read_secret".to_owned()) {
        permissions.push(PermissionGrant {
            name: "read_secret".to_owned(),
            risk: RiskLevel::High,
        });
    }

    permissions.sort_by(|left, right| left.name.cmp(&right.name));
    let calculated_risk = highest_risk(&permissions);

    Ok(ProgramPermissionReport {
        required_permissions: permissions,
        calculated_risk,
    })
}

pub fn permission_for_action_type(action_type: &str) -> Option<PermissionGrant> {
    let (name, risk) = match action_type {
        "action.application.open" => ("open_application", RiskLevel::Medium),
        "action.beep" => ("beep", RiskLevel::Low),
        "action.calculate" => ("calculate", RiskLevel::Low),
        "action.clipboard" => ("write_clipboard", RiskLevel::Medium),
        "action.delay" => ("delay", RiskLevel::Low),
        "action.file.copy" => ("file_copy", RiskLevel::Medium),
        "action.file.delete" => ("delete_file", RiskLevel::Dangerous),
        "action.file.download" => ("download_file", RiskLevel::Medium),
        "action.file.move" => ("file_move", RiskLevel::Medium),
        "action.file.read" => ("file_read", RiskLevel::Medium),
        "action.file.write" => ("file_write_limited", RiskLevel::High),
        "action.http" => ("http_request", RiskLevel::Medium),
        "action.keyboard" | "action.keyboard.type_text" => ("keyboard_control", RiskLevel::High),
        "action.log" => ("log", RiskLevel::Low),
        "action.message_box" => ("show_message_box", RiskLevel::Medium),
        "action.mouse" | "action.mouse.move" => ("mouse_control", RiskLevel::High),
        "action.notification" => ("show_notification", RiskLevel::Medium),
        "action.pixel.get" => ("screen_pixel_read", RiskLevel::Medium),
        "action.process.kill" => ("process_kill", RiskLevel::High),
        "action.process.run" => ("run_process", RiskLevel::High),
        "action.process.status" => ("process_query", RiskLevel::Medium),
        "action.script.run" => ("sub_script_run", RiskLevel::High),
        "action.serial.write" => ("serial_write", RiskLevel::Medium),
        "action.shell" => ("run_shell_command", RiskLevel::Dangerous),
        "action.sound.play" => ("play_sound", RiskLevel::Medium),
        "action.text.format" => ("text_transform", RiskLevel::Low),
        "action.webhook_response" => ("webhook_response", RiskLevel::Low),
        "action.websocket.write" => ("websocket_write", RiskLevel::Medium),
        "action.window.active" => ("window_query", RiskLevel::Medium),
        "action.window.focus" => ("window_focus", RiskLevel::High),
        "runtime.set_variable" => ("set_local_variable", RiskLevel::Low),
        "trigger.serial_input" => ("serial_input", RiskLevel::High),
        "trigger.startup" => ("startup_trigger", RiskLevel::High),
        "trigger.webhook" => ("webhook_public_bind", RiskLevel::High),
        "trigger.websocket" => ("websocket_public_bind", RiskLevel::High),
        _ => return None,
    };

    Some(PermissionGrant {
        name: name.to_owned(),
        risk,
    })
}

fn is_known_permissionless_action_type(action_type: &str) -> bool {
    matches!(
        action_type,
        "control.for_each"
            | "control.if"
            | "control.loop"
            | "control.switch"
            | "control.while"
            | "trigger.file_watch"
            | "trigger.hotkey"
            | "trigger.manual"
            | "trigger.process_started"
            | "trigger.schedule"
    )
}

fn enforce_runner_policy(
    permissions: &[PermissionGrant],
    policy: &RunnerPolicy,
) -> Result<(), PermissionValidationError> {
    for permission in permissions {
        if permission.risk == RiskLevel::Dangerous && !policy.allow_dangerous_actions {
            return Err(PermissionValidationError::PolicyBlocked {
                permission: permission.name.clone(),
                reason: "dangerous actions are disabled".to_owned(),
            });
        }
        if permission.name == "run_shell_command" && !policy.allow_shell_commands {
            return Err(PermissionValidationError::PolicyBlocked {
                permission: permission.name.clone(),
                reason: "shell commands are disabled".to_owned(),
            });
        }
        if matches!(
            permission.name.as_str(),
            "webhook_public_bind" | "websocket_public_bind"
        ) && !policy.allow_network_servers
        {
            return Err(PermissionValidationError::PolicyBlocked {
                permission: permission.name.clone(),
                reason: "network server triggers are disabled".to_owned(),
            });
        }
    }

    Ok(())
}

pub(crate) fn executable_action_types(program: &Value) -> Result<Vec<String>, String> {
    let entry = program
        .get("entry")
        .and_then(Value::as_object)
        .ok_or_else(|| "missing entry".to_owned())?;
    let mut action_types = Vec::new();

    if let Some(action_type) = entry
        .get("trigger")
        .and_then(|trigger| trigger.get("action_type"))
        .and_then(Value::as_str)
    {
        action_types.push(action_type.to_owned());
    }

    if let Some(triggers) = entry.get("triggers").and_then(Value::as_array) {
        for trigger in triggers {
            if let Some(action_type) = trigger.get("action_type").and_then(Value::as_str) {
                action_types.push(action_type.to_owned());
            }
        }
    }

    let steps = entry
        .get("program")
        .and_then(|program| program.get("steps"))
        .and_then(Value::as_array)
        .ok_or_else(|| "missing entry.program.steps".to_owned())?;

    for step in steps {
        if let Some(action_type) = step.get("action_type").and_then(Value::as_str) {
            action_types.push(action_type.to_owned());
        }
    }

    action_types.sort();
    action_types.dedup();
    Ok(action_types)
}

pub(crate) fn variable_operation_scopes(program: &Value) -> Result<Vec<String>, String> {
    let steps = program
        .get("entry")
        .and_then(|entry| entry.get("program"))
        .and_then(|program| program.get("steps"))
        .and_then(Value::as_array)
        .ok_or_else(|| "missing entry.program.steps".to_owned())?;

    steps
        .iter()
        .filter(|step| {
            step.get("action_type").and_then(Value::as_str) == Some("runtime.set_variable")
        })
        .map(|step| {
            step.get("config")
                .and_then(|config| config.get("scope"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .ok_or_else(|| "runtime.set_variable is missing string config.scope".to_owned())
        })
        .collect()
}

pub(crate) fn first_duplicate(values: &[String]) -> Option<String> {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .find(|value| !seen.insert(value.as_str()))
        .cloned()
}

#[cfg(test)]
mod tests;
