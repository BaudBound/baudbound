use std::collections::BTreeMap;

use baudbound_script::{ScriptPackage, SecretDeclaration, load_script_package};
use baudbound_storage::{ScriptStore, SecretStatus};
use serde::Serialize;
use serde_json::Value;

use crate::{CoreError, RunnerCore};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InstalledSecretStatus {
    pub configured: bool,
    pub description: String,
    pub name: String,
    pub required: bool,
    pub updated_at_unix: Option<u64>,
    pub value_type: String,
}

pub(crate) fn list_installed_secrets(
    core: &RunnerCore,
    store: &impl ScriptStore,
    reference: &str,
) -> Result<Vec<InstalledSecretStatus>, CoreError> {
    let (_, package) = load_installed_package(core, store, reference)?;
    let configured = store
        .list_secret_statuses(reference)?
        .into_iter()
        .map(|status| (status.name.clone(), status))
        .collect::<BTreeMap<_, _>>();
    Ok(package
        .manifest
        .secrets
        .iter()
        .map(|declaration| merge_status(declaration, configured.get(&declaration.name)))
        .collect())
}

pub(crate) fn set_installed_secret_from_text(
    core: &RunnerCore,
    store: &impl ScriptStore,
    reference: &str,
    name: &str,
    input: &str,
) -> Result<InstalledSecretStatus, CoreError> {
    let (_, package) = load_installed_package(core, store, reference)?;
    let declaration = package
        .manifest
        .secrets
        .iter()
        .find(|secret| secret.name == name)
        .ok_or_else(|| {
            CoreError::InvalidSecret(format!("{name:?} is not declared by this script"))
        })?;
    let value = parse_secret_value(&declaration.value_type, input)?;
    let status = store.set_secret(reference, name, &value)?;
    Ok(merge_status(declaration, Some(&status)))
}

pub(crate) fn remove_installed_secret(
    core: &RunnerCore,
    store: &impl ScriptStore,
    reference: &str,
    name: &str,
) -> Result<bool, CoreError> {
    let (_, package) = load_installed_package(core, store, reference)?;
    if !package
        .manifest
        .secrets
        .iter()
        .any(|secret| secret.name == name)
    {
        return Err(CoreError::InvalidSecret(format!(
            "{name:?} is not declared by this script"
        )));
    }
    store
        .remove_secret(reference, name)
        .map_err(CoreError::Storage)
}

fn load_installed_package(
    core: &RunnerCore,
    store: &impl ScriptStore,
    reference: &str,
) -> Result<(baudbound_storage::InstalledScript, ScriptPackage), CoreError> {
    let installed = store.verify_script_package_hash(reference)?;
    let package = load_script_package(&installed.package_path)?;
    core.validate_package_compatibility(&package)?;
    Ok((installed, package))
}

fn merge_status(
    declaration: &SecretDeclaration,
    stored: Option<&SecretStatus>,
) -> InstalledSecretStatus {
    InstalledSecretStatus {
        configured: stored.is_some_and(|status| status.configured),
        description: declaration.description.clone(),
        name: declaration.name.clone(),
        required: declaration.required,
        updated_at_unix: stored.and_then(|status| status.updated_at_unix),
        value_type: declaration.value_type.clone(),
    }
}

fn parse_secret_value(value_type: &str, input: &str) -> Result<Value, CoreError> {
    match value_type {
        "string" | "file_path" => Ok(Value::String(input.to_owned())),
        "number" => input
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .ok_or_else(|| CoreError::InvalidSecret("expected a finite number".to_owned())),
        "boolean" => input
            .parse::<bool>()
            .map(Value::Bool)
            .map_err(|_| CoreError::InvalidSecret("expected true or false".to_owned())),
        "list" => parse_json_container(input, false),
        "object" | "http_response" | "datetime" | "duration" => parse_json_container(input, true),
        invalid => Err(CoreError::InvalidSecret(format!(
            "unsupported declared type {invalid:?}"
        ))),
    }
}

fn parse_json_container(input: &str, object: bool) -> Result<Value, CoreError> {
    let value = serde_json::from_str::<Value>(input)
        .map_err(|error| CoreError::InvalidSecret(format!("expected valid JSON: {error}")))?;
    if (object && value.is_object()) || (!object && value.is_array()) {
        Ok(value)
    } else {
        Err(CoreError::InvalidSecret(format!(
            "expected a JSON {}",
            if object { "object" } else { "array" }
        )))
    }
}
