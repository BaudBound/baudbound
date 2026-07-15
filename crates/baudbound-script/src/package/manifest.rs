use std::collections::{BTreeMap, BTreeSet};

use crate::{Manifest, Program};

use super::{PackageLoadError, finish_validation};

const SUPPORTED_VARIABLE_TYPES: &[&str] = &[
    "string",
    "number",
    "boolean",
    "object",
    "list",
    "http_response",
    "datetime",
    "duration",
    "file_path",
];

pub(super) fn validate_manifest_secrets(manifest: &Manifest) -> Result<(), PackageLoadError> {
    let mut errors = Vec::new();
    let mut names = BTreeSet::new();
    for secret in &manifest.secrets {
        validate_name("secret", &secret.name, &mut errors);
        if !names.insert(secret.name.as_str()) {
            errors.push(format!("duplicate manifest secret name {:?}", secret.name));
        }
        if !SUPPORTED_VARIABLE_TYPES.contains(&secret.value_type.as_str()) {
            errors.push(format!(
                "manifest secret {:?} uses unsupported type {:?}",
                secret.name, secret.value_type
            ));
        }
    }
    finish_validation(errors)
}

pub(super) fn validate_manifest_variables(manifest: &Manifest) -> Result<(), PackageLoadError> {
    let mut errors = Vec::new();
    let mut names = BTreeSet::new();
    let secret_names = manifest
        .secrets
        .iter()
        .map(|secret| secret.name.as_str())
        .collect::<BTreeSet<_>>();

    for variable in &manifest.variables {
        validate_name("variable", &variable.name, &mut errors);
        if !names.insert(variable.name.as_str()) {
            errors.push(format!(
                "duplicate manifest variable name {:?}",
                variable.name
            ));
        }
        if secret_names.contains(variable.name.as_str()) {
            errors.push(format!(
                "manifest variable {:?} conflicts with a secret declaration",
                variable.name
            ));
        }
        if !matches!(variable.scope.as_str(), "runtime" | "persistent") {
            errors.push(format!(
                "manifest variable {:?} uses unsupported scope {:?}",
                variable.name, variable.scope
            ));
        }
        if !SUPPORTED_VARIABLE_TYPES.contains(&variable.value_type.as_str()) {
            errors.push(format!(
                "manifest variable {:?} uses unsupported type {:?}",
                variable.name, variable.value_type
            ));
        } else if !default_value_matches_type(&variable.value_type, &variable.value) {
            errors.push(format!(
                "manifest variable {:?} default value does not match type {}",
                variable.name, variable.value_type
            ));
        }
    }
    finish_validation(errors)
}

pub(super) fn validate_manifest_variable_operations(
    manifest: &Manifest,
    program: &Program,
) -> Result<(), PackageLoadError> {
    if manifest.variables.is_empty() {
        return Ok(());
    }

    let defaults = manifest
        .variables
        .iter()
        .map(|variable| (variable.name.as_str(), variable))
        .collect::<BTreeMap<_, _>>();
    let steps = program
        .pointer("/entry/program/steps")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| PackageLoadError::Validation("program steps are missing".to_owned()))?;
    let mut errors = Vec::new();

    for step in steps {
        if step.get("action_type").and_then(serde_json::Value::as_str)
            != Some("runtime.set_variable")
        {
            continue;
        }
        let Some(config) = step.get("config").and_then(serde_json::Value::as_object) else {
            continue;
        };
        let Some(name) = config.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(variable) = defaults.get(name) else {
            continue;
        };
        let scope = config
            .get("scope")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let value_type = config
            .get("valueType")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("string");
        if scope != variable.scope || value_type != variable.value_type {
            errors.push(format!(
                "manifest variable {name:?} does not match Variable Operation scope and type"
            ));
        }
    }

    finish_validation(errors)
}

fn validate_name(kind: &str, name: &str, errors: &mut Vec<String>) {
    if !is_variable_identifier(name) {
        errors.push(format!(
            "manifest {kind} {name:?} must start with a letter or underscore and contain only letters, numbers, or underscores"
        ));
    }
    if name.starts_with("system_") || name.starts_with("manifest_") {
        errors.push(format!(
            "manifest {kind} {name:?} uses a reserved variable prefix"
        ));
    }
}

fn default_value_matches_type(value_type: &str, value: &serde_json::Value) -> bool {
    match value_type {
        "string" => value.as_str().is_some_and(|text| !text.trim().is_empty()),
        "file_path" => value.as_str().is_some_and(|path| !path.trim().is_empty()),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "list" => value.is_array(),
        "object" => value.is_object(),
        "http_response" => value.as_object().is_some_and(|object| {
            object.get("type").and_then(serde_json::Value::as_str) == Some("http_response")
                && object
                    .get("status")
                    .is_some_and(serde_json::Value::is_number)
                && object
                    .get("headers")
                    .is_some_and(serde_json::Value::is_object)
                && object.contains_key("body")
        }),
        "datetime" => value.as_object().is_some_and(|object| {
            object.get("type").and_then(serde_json::Value::as_str) == Some("datetime")
                && object
                    .get("value")
                    .is_some_and(serde_json::Value::is_string)
        }),
        "duration" => value.as_object().is_some_and(|object| {
            object.get("type").and_then(serde_json::Value::as_str) == Some("duration")
                && object.get("unit").is_some_and(serde_json::Value::is_string)
                && object
                    .get("value")
                    .is_some_and(serde_json::Value::is_number)
        }),
        _ => false,
    }
}

fn is_variable_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_manifest_default_variables() {
        let manifest = manifest_with_variables(serde_json::json!([{
            "name": "counter",
            "scope": "persistent",
            "type": "number",
            "description": "Retained counter",
            "value": 10
        }]));

        validate_manifest_variables(&manifest).expect("valid default variable should pass");
    }

    #[test]
    fn rejects_invalid_manifest_default_variables() {
        for (variables, expected) in [
            (
                serde_json::json!([{"name": "counter", "scope": "global", "type": "number", "value": 10}]),
                "unsupported scope",
            ),
            (
                serde_json::json!([{"name": "counter", "scope": "runtime", "type": "number", "value": "ten"}]),
                "does not match type",
            ),
            (
                serde_json::json!([{"name": "label", "scope": "runtime", "type": "string", "value": ""}]),
                "does not match type",
            ),
        ] {
            let error = validate_manifest_variables(&manifest_with_variables(variables))
                .expect_err("invalid default variable should fail");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn rejects_default_variable_operation_contract_mismatch() {
        let manifest = manifest_with_variables(serde_json::json!([{
            "name": "counter",
            "scope": "persistent",
            "type": "number",
            "value": 10
        }]));
        let program = serde_json::json!({
            "entry": {"program": {"steps": [{
                "action_type": "runtime.set_variable",
                "config": {"name": "counter", "scope": "runtime", "valueType": "number"}
            }]}}
        });

        let error = validate_manifest_variable_operations(&manifest, &program)
            .expect_err("mismatched operation should fail");
        assert!(
            error
                .to_string()
                .contains("does not match Variable Operation")
        );
    }

    fn manifest_with_variables(variables: serde_json::Value) -> Manifest {
        serde_json::from_value(serde_json::json!({
            "format_version": 1,
            "script_language_version": 1,
            "id": "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7",
            "name": "default-variables",
            "created_with": "BaudBound Test",
            "created_at": "2026-01-01T00:00:00.000Z",
            "minimum_runner_version": "0.1.0",
            "variables": variables
        }))
        .expect("test manifest should deserialize")
    }
}
