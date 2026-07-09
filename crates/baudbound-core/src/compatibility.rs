use std::{collections::BTreeSet, fmt};

use baudbound_script::ScriptPackage;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetRuntime {
    GenericHeadless,
    LinuxHeadless,
    WindowsHeadless,
    GenericDesktop,
    WindowsDesktop,
    LinuxDesktop,
}

impl TargetRuntime {
    pub fn is_desktop(self) -> bool {
        matches!(
            self,
            Self::GenericDesktop | Self::WindowsDesktop | Self::LinuxDesktop
        )
    }
}

impl fmt::Display for TargetRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::GenericHeadless => "Generic Headless",
            Self::LinuxHeadless => "Linux Headless",
            Self::WindowsHeadless => "Windows Headless",
            Self::GenericDesktop => "Generic Desktop",
            Self::WindowsDesktop => "Windows Desktop",
            Self::LinuxDesktop => "Linux Desktop",
        })
    }
}

#[derive(Debug, Error)]
pub enum CompatibilityError {
    #[error("unsupported target runtime {0:?}")]
    UnknownTargetRuntime(String),
    #[error("runner target runtime config contains unsupported target runtime {0:?}")]
    UnknownRunnerConfigTargetRuntime(String),
    #[error("script targets {target_runtime}, but this runner supports only {supported}")]
    RunnerTargetMismatch {
        supported: String,
        target_runtime: TargetRuntime,
    },
    #[error("target runtime compatibility failed: {0}")]
    NodeTargetMismatch(String),
}

pub fn validate_package_target_runtime(package: &ScriptPackage) -> Result<(), CompatibilityError> {
    let target_runtime = parse_target_runtime(&package.capabilities.target_runtime)?;
    let errors = compatibility_errors(&package.program, target_runtime);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(CompatibilityError::NodeTargetMismatch(errors.join("; ")))
    }
}

pub fn validate_package_for_runner(
    package: &ScriptPackage,
    supported_target_runtimes: &[String],
) -> Result<(), CompatibilityError> {
    validate_package_target_runtime(package)?;
    let target_runtime = parse_target_runtime(&package.capabilities.target_runtime)?;
    let supported = parse_supported_target_runtimes(supported_target_runtimes)?;
    if supported.contains(&target_runtime) {
        return Ok(());
    }

    Err(CompatibilityError::RunnerTargetMismatch {
        supported: format_target_runtime_list(&supported),
        target_runtime,
    })
}

#[must_use]
pub fn default_host_target_runtime_names() -> Vec<String> {
    default_host_target_runtimes()
        .into_iter()
        .map(|target_runtime| target_runtime.to_string())
        .collect()
}

#[must_use]
pub fn runner_target_runtime_names(configured: &[String]) -> Vec<String> {
    if configured.is_empty() {
        return default_host_target_runtime_names();
    }

    configured
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn compatibility_errors(program: &Value, target_runtime: TargetRuntime) -> Vec<String> {
    program_nodes(program)
        .into_iter()
        .filter_map(|node| node_compatibility_error(node, target_runtime))
        .collect()
}

fn node_compatibility_error(
    node: ProgramNode<'_>,
    target_runtime: TargetRuntime,
) -> Option<String> {
    let support = action_support(node.action_type)?;
    if support.supports(target_runtime) {
        return None;
    }

    Some(match support {
        ActionSupport::Desktop => format!(
            "{} ({}) requires a desktop target runtime, but the script targets {}",
            node.id, node.action_type, target_runtime
        ),
        ActionSupport::WindowsDesktop => format!(
            "{} ({}) requires Windows Desktop, but the script targets {}",
            node.id, node.action_type, target_runtime
        ),
    })
}

#[derive(Debug, Clone, Copy)]
enum ActionSupport {
    Desktop,
    WindowsDesktop,
}

impl ActionSupport {
    fn supports(self, target_runtime: TargetRuntime) -> bool {
        match self {
            Self::Desktop => target_runtime.is_desktop(),
            Self::WindowsDesktop => target_runtime == TargetRuntime::WindowsDesktop,
        }
    }
}

fn action_support(action_type: &str) -> Option<ActionSupport> {
    if WINDOWS_DESKTOP_ONLY_ACTIONS.contains(&action_type) {
        return Some(ActionSupport::WindowsDesktop);
    }

    if DESKTOP_ONLY_ACTIONS.contains(&action_type) {
        return Some(ActionSupport::Desktop);
    }

    None
}

pub const WINDOWS_DESKTOP_ONLY_ACTIONS: &[&str] = &[
    "action.pixel.get",
    "action.window.active",
    "action.window.focus",
];

pub const DESKTOP_ONLY_ACTIONS: &[&str] = &[
    "action.application.open",
    "action.clipboard",
    "action.keyboard",
    "action.keyboard.type_text",
    "action.message_box",
    "action.mouse",
    "action.mouse.move",
    "action.notification",
    "action.sound.play",
    "trigger.hotkey",
];

#[derive(Debug, Clone, Copy)]
struct ProgramNode<'a> {
    action_type: &'a str,
    id: &'a str,
}

fn program_nodes(program: &Value) -> Vec<ProgramNode<'_>> {
    let Some(entry) = program.get("entry").and_then(Value::as_object) else {
        return Vec::new();
    };

    let mut nodes = Vec::new();
    let mut seen_ids = BTreeSet::new();
    if let Some(trigger) = entry.get("trigger") {
        push_program_node(&mut nodes, &mut seen_ids, trigger);
    }
    if let Some(triggers) = entry.get("triggers").and_then(Value::as_array) {
        for trigger in triggers {
            push_program_node(&mut nodes, &mut seen_ids, trigger);
        }
    }
    if let Some(steps) = entry
        .get("program")
        .and_then(Value::as_object)
        .and_then(|program| program.get("steps"))
        .and_then(Value::as_array)
    {
        for step in steps {
            push_program_node(&mut nodes, &mut seen_ids, step);
        }
    }
    nodes
}

fn push_program_node<'a>(
    nodes: &mut Vec<ProgramNode<'a>>,
    seen_ids: &mut BTreeSet<&'a str>,
    value: &'a Value,
) {
    let Some(record) = value.as_object() else {
        return;
    };
    let Some(action_type) = record.get("action_type").and_then(Value::as_str) else {
        return;
    };
    let id = record
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .unwrap_or(action_type);

    if seen_ids.insert(id) {
        nodes.push(ProgramNode { action_type, id });
    }
}

fn parse_supported_target_runtimes(
    values: &[String],
) -> Result<BTreeSet<TargetRuntime>, CompatibilityError> {
    let values = if values.is_empty() {
        return Ok(default_host_target_runtimes().into_iter().collect());
    } else {
        values
    };
    values
        .iter()
        .map(|value| {
            parse_target_runtime(value)
                .map_err(|_| CompatibilityError::UnknownRunnerConfigTargetRuntime(value.clone()))
        })
        .collect()
}

fn default_host_target_runtimes() -> Vec<TargetRuntime> {
    #[cfg(windows)]
    {
        vec![
            TargetRuntime::GenericHeadless,
            TargetRuntime::WindowsHeadless,
            TargetRuntime::GenericDesktop,
            TargetRuntime::WindowsDesktop,
        ]
    }

    #[cfg(unix)]
    {
        vec![
            TargetRuntime::GenericHeadless,
            TargetRuntime::LinuxHeadless,
            TargetRuntime::GenericDesktop,
            TargetRuntime::LinuxDesktop,
        ]
    }

    #[cfg(not(any(windows, unix)))]
    {
        vec![TargetRuntime::GenericHeadless]
    }
}

fn format_target_runtime_list(target_runtimes: &BTreeSet<TargetRuntime>) -> String {
    target_runtimes
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_target_runtime(value: &str) -> Result<TargetRuntime, CompatibilityError> {
    match value.trim() {
        "Generic Headless" => Ok(TargetRuntime::GenericHeadless),
        "Linux Headless" => Ok(TargetRuntime::LinuxHeadless),
        "Windows Headless" => Ok(TargetRuntime::WindowsHeadless),
        "Generic Desktop" => Ok(TargetRuntime::GenericDesktop),
        "Windows Desktop" => Ok(TargetRuntime::WindowsDesktop),
        "Linux Desktop" => Ok(TargetRuntime::LinuxDesktop),
        other => Err(CompatibilityError::UnknownTargetRuntime(other.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use baudbound_script::{Capabilities, Manifest, Permissions, RiskLevel, ScriptPackage};
    use serde_json::json;

    use super::*;

    #[test]
    fn accepts_cross_platform_desktop_actions_on_desktop_targets() {
        let package = package_with_target_and_step("Linux Desktop", "action.notification");

        validate_package_target_runtime(&package)
            .expect("desktop action should support desktop target");
    }

    #[test]
    fn rejects_desktop_actions_on_headless_targets() {
        let package = package_with_target_and_step("Generic Headless", "action.notification");

        let error =
            validate_package_target_runtime(&package).expect_err("headless target should reject");

        assert!(
            error
                .to_string()
                .contains("requires a desktop target runtime")
        );
    }

    #[test]
    fn rejects_windows_only_actions_on_non_windows_desktop_targets() {
        let package = package_with_target_and_step("Linux Desktop", "action.pixel.get");

        let error =
            validate_package_target_runtime(&package).expect_err("linux desktop should reject");

        assert!(error.to_string().contains("requires Windows Desktop"));
    }

    #[test]
    fn accepts_windows_only_actions_on_windows_desktop() {
        let package = package_with_target_and_step("Windows Desktop", "action.window.focus");

        validate_package_target_runtime(&package)
            .expect("Windows Desktop should support native Win32 window actions");
    }

    #[test]
    fn rejects_packages_not_supported_by_this_runner_config() {
        let package = package_with_target_and_step("Windows Desktop", "action.text.format");

        let error = validate_package_for_runner(&package, &["Linux Headless".to_owned()])
            .expect_err("runner config should reject unsupported target runtime");

        assert!(
            error
                .to_string()
                .contains("this runner supports only Linux Headless"),
            "{error}"
        );
    }

    #[test]
    fn rejects_invalid_runner_target_runtime_config_values() {
        let package = package_with_target_and_step("Generic Headless", "action.text.format");

        let error = validate_package_for_runner(&package, &["Toaster".to_owned()])
            .expect_err("invalid runner target runtime config should fail");

        assert!(error.to_string().contains("unsupported target runtime"));
    }

    fn package_with_target_and_step(target_runtime: &str, action_type: &str) -> ScriptPackage {
        ScriptPackage {
            capabilities: Capabilities {
                required_capabilities: Vec::new(),
                target_runtime: target_runtime.to_owned(),
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
                        "type": "block",
                        "steps": [
                            {
                                "id": "n-action",
                                "action_type": action_type,
                                "type": "action",
                                "config": {},
                                "runtime_outputs": []
                            }
                        ],
                        "edges": []
                    }
                }
            }),
        }
    }
}
