use std::path::Path;

use baudbound_script::{PackageSummary, RiskLevel, ScriptPackage};
use baudbound_security::{
    RunnerPolicy, RuntimeDeclarationRequirements, SecurityValidationError,
    validate_program_capabilities_with_declarations,
    validate_program_permissions_with_declarations,
};
use baudbound_storage::{ImportScriptRequest, NetworkTriggerDefinition, NetworkTriggerType};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct PackageInspection {
    pub entries: Vec<String>,
    pub summary: PackageSummary,
}

impl PackageInspection {
    pub(crate) fn from_package(package: ScriptPackage) -> Self {
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

pub(crate) fn import_request_from_package(
    path: &Path,
    package: ScriptPackage,
) -> ImportScriptRequest {
    let summary = package.summary();
    ImportScriptRequest {
        id: package.manifest.id,
        name: summary.script_name,
        package_source: path.to_path_buf(),
        package_format_version: summary.package_format_version,
        script_language_version: summary.script_language_version,
        target_runtime: summary.target_runtimes.join(", "),
        asset_count: summary.asset_count,
        risk_level: risk_level_name(&package.permissions.risk_level).to_owned(),
    }
}

pub(crate) fn network_trigger_definitions(program: &Value) -> Vec<NetworkTriggerDefinition> {
    let Some(entry) = program.get("entry").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut triggers = Vec::new();
    if let Some(trigger) = entry.get("trigger") {
        triggers.push(trigger);
    }
    if let Some(entries) = entry.get("triggers").and_then(Value::as_array) {
        triggers.extend(entries);
    }
    triggers
        .into_iter()
        .filter_map(|trigger| {
            let trigger_type = match trigger.get("action_type").and_then(Value::as_str)? {
                "trigger.webhook" => NetworkTriggerType::Webhook,
                "trigger.websocket" => NetworkTriggerType::Websocket,
                _ => return None,
            };
            Some(NetworkTriggerDefinition {
                node_id: trigger.get("id")?.as_str()?.to_owned(),
                trigger_type,
            })
        })
        .collect()
}

pub(crate) fn validate_package_security(
    package: &ScriptPackage,
    policy: &RunnerPolicy,
) -> Result<(), SecurityValidationError> {
    let requirements = RuntimeDeclarationRequirements {
        has_persistent_declared_variables: package
            .manifest
            .variables
            .iter()
            .any(|variable| variable.scope == "persistent"),
        has_runtime_declared_variables: package
            .manifest
            .variables
            .iter()
            .any(|variable| variable.scope == "runtime"),
        has_secret_declarations: !package.manifest.secrets.is_empty(),
    };
    validate_program_permissions_with_declarations(
        &package.program,
        &package.permissions.declared_permissions,
        security_risk_level(&package.permissions.risk_level),
        policy,
        requirements,
    )?;
    validate_program_capabilities_with_declarations(
        &package.program,
        &package.capabilities.required_capabilities,
        requirements,
    )?;
    Ok(())
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

/// Reads one trigger node's overlap option out of a program.
///
/// Mirrors the trigger traversal in `network_trigger_definitions`: the entry
/// trigger plus any secondary triggers. A node that cannot be found, or a
/// program written before the option existed, reads as `Queue`.
pub(crate) fn trigger_overlap(program: &Value, trigger_node_id: &str) -> crate::TriggerOverlap {
    let Some(entry) = program.get("entry") else {
        return crate::TriggerOverlap::Queue;
    };
    let mut triggers = Vec::new();
    if let Some(trigger) = entry.get("trigger") {
        triggers.push(trigger);
    }
    if let Some(entries) = entry.get("triggers").and_then(Value::as_array) {
        triggers.extend(entries);
    }

    triggers
        .into_iter()
        .find(|trigger| trigger.get("id").and_then(Value::as_str) == Some(trigger_node_id))
        .and_then(|trigger| trigger.get("config"))
        .map_or(
            crate::TriggerOverlap::Queue,
            crate::TriggerOverlap::from_config,
        )
}

#[cfg(test)]
mod overlap_tests {
    use super::trigger_overlap;
    use crate::TriggerOverlap;
    use serde_json::json;

    fn program(entry_overlap: &str, secondary_overlap: &str) -> serde_json::Value {
        json!({
            "entry": {
                "trigger": {
                    "id": "n-entry",
                    "action_type": "trigger.webhook",
                    "config": { "overlap": entry_overlap }
                },
                "triggers": [{
                    "id": "n-second",
                    "action_type": "trigger.schedule",
                    "config": { "overlap": secondary_overlap }
                }]
            }
        })
    }

    #[test]
    fn reads_the_mode_of_the_trigger_that_fired() {
        // Per trigger, not per script: one may toggle while another queues.
        let program = program("stop", "queue");
        assert_eq!(
            trigger_overlap(&program, "n-entry"),
            TriggerOverlap::Stop,
            "the entry trigger's own mode applies"
        );
        assert_eq!(
            trigger_overlap(&program, "n-second"),
            TriggerOverlap::Queue,
            "a secondary trigger keeps its own mode"
        );
    }

    #[test]
    fn every_mode_is_understood() {
        for (configured, expected) in [
            ("queue", TriggerOverlap::Queue),
            ("skip", TriggerOverlap::Skip),
            ("stop", TriggerOverlap::Stop),
            ("restart", TriggerOverlap::Restart),
        ] {
            assert_eq!(
                trigger_overlap(&program(configured, "queue"), "n-entry"),
                expected
            );
        }
    }

    #[test]
    fn anything_unrecognised_queues() {
        // A package written before the option existed, one carrying a value
        // from a newer editor, and a node that is not there at all all keep
        // today's behaviour rather than being refused.
        assert_eq!(
            trigger_overlap(
                &json!({"entry": {"trigger": {"id": "n-entry", "config": {}}}}),
                "n-entry"
            ),
            TriggerOverlap::Queue
        );
        assert_eq!(
            trigger_overlap(&program("nonsense", "queue"), "n-entry"),
            TriggerOverlap::Queue
        );
        assert_eq!(
            trigger_overlap(&program("stop", "stop"), "n-missing"),
            TriggerOverlap::Queue
        );
        assert_eq!(
            trigger_overlap(&json!({}), "n-entry"),
            TriggerOverlap::Queue
        );
    }
}
