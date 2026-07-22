use std::collections::{BTreeMap, BTreeSet};

use crate::{Manifest, Program};
use semver::Version;
use url::Url;

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

const MAX_DEFAULT_VALUE_BYTES: usize = 1_048_576;

pub(super) fn validate_manifest_metadata(manifest: &Manifest) -> Result<(), PackageLoadError> {
    let mut errors = Vec::new();

    validate_text("id", &manifest.id, 36, false, true, &mut errors);
    validate_text("name", &manifest.name, 128, false, true, &mut errors);
    validate_text(
        "description",
        &manifest.description,
        4096,
        true,
        false,
        &mut errors,
    );
    validate_text("author", &manifest.author, 128, false, false, &mut errors);
    validate_http_url("website", &manifest.website, &mut errors);
    validate_http_url("source", &manifest.source, &mut errors);
    validate_text(
        "created_with",
        &manifest.created_with,
        128,
        false,
        true,
        &mut errors,
    );
    validate_text(
        "created_at",
        &manifest.created_at,
        64,
        false,
        true,
        &mut errors,
    );
    validate_text(
        "updated_at",
        &manifest.updated_at,
        64,
        false,
        false,
        &mut errors,
    );
    validate_text(
        "minimum_runner_version",
        &manifest.minimum_runner_version,
        64,
        false,
        true,
        &mut errors,
    );
    validate_text("version", &manifest.version, 128, false, true, &mut errors);
    if Version::parse(&manifest.version).is_err() {
        errors.push("manifest version must be a valid semantic version".to_owned());
    }
    validate_update_url(&manifest.update_url, &mut errors);

    for tag in &manifest.tags {
        validate_text("tag", tag, 64, false, true, &mut errors);
    }
    for asset in &manifest.assets {
        validate_text("asset id", &asset.id, 128, false, true, &mut errors);
        validate_text(
            "asset media type",
            &asset.media_type,
            128,
            false,
            true,
            &mut errors,
        );
        validate_text("asset name", &asset.name, 256, false, true, &mut errors);
    }
    for variable in &manifest.variables {
        validate_text(
            "variable description",
            &variable.description,
            1024,
            true,
            false,
            &mut errors,
        );
        if serde_json::to_vec(&variable.value)
            .is_ok_and(|value| value.len() > MAX_DEFAULT_VALUE_BYTES)
        {
            errors.push(format!(
                "manifest variable {:?} default value exceeds {MAX_DEFAULT_VALUE_BYTES} bytes",
                variable.name
            ));
        }
    }
    for secret in &manifest.secrets {
        validate_text(
            "secret description",
            &secret.description,
            1024,
            true,
            false,
            &mut errors,
        );
    }

    finish_validation(errors)
}

fn validate_update_url(value: &str, errors: &mut Vec<String>) {
    validate_text("update_url", value, 2048, false, false, errors);
    if value.is_empty() {
        return;
    }
    match Url::parse(value) {
        Ok(url)
            if url.scheme() == "https"
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
                && url.path_segments().and_then(Iterator::last) == Some("update.json") => {}
        _ => errors.push(
            "manifest update_url must be an HTTPS URL without credentials or a fragment and must end in update.json"
                .to_owned(),
        ),
    }
}

fn validate_http_url(field: &str, value: &str, errors: &mut Vec<String>) {
    validate_text(field, value, 2048, false, false, errors);
    if value.is_empty() {
        return;
    }
    match Url::parse(value) {
        Ok(url) if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() => {}
        _ => errors.push(format!(
            "manifest {field} must be an absolute HTTP or HTTPS URL"
        )),
    }
}

fn validate_text(
    field: &str,
    value: &str,
    max_chars: usize,
    multiline: bool,
    required: bool,
    errors: &mut Vec<String>,
) {
    let character_count = value.chars().count();
    if required && value.trim().is_empty() {
        errors.push(format!("manifest {field} cannot be empty"));
    }
    if !value.is_empty() && value.trim() != value {
        errors.push(format!(
            "manifest {field} cannot start or end with whitespace"
        ));
    }
    if character_count > max_chars {
        errors.push(format!(
            "manifest {field} contains {character_count} characters, maximum is {max_chars}"
        ));
    }
    if value.chars().any(|character| {
        (character.is_control() && !(multiline && matches!(character, '\n' | '\t')))
            || is_bidirectional_control(character)
    }) {
        errors.push(format!(
            "manifest {field} contains unsupported control characters"
        ));
    }
}

fn is_bidirectional_control(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

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
    fn rejects_unsafe_manifest_metadata() {
        for (field, value, expected) in [
            (
                "website",
                "javascript:alert(1)",
                "absolute HTTP or HTTPS URL",
            ),
            ("name", "safe\u{202e}txt", "unsupported control characters"),
            (
                "author",
                "terminal\u{1b}[2J",
                "unsupported control characters",
            ),
        ] {
            let mut manifest = manifest_with_variables(serde_json::json!([]));
            match field {
                "website" => manifest.website = value.to_owned(),
                "name" => manifest.name = value.to_owned(),
                "author" => manifest.author = value.to_owned(),
                _ => unreachable!(),
            }
            let error = validate_manifest_metadata(&manifest)
                .expect_err("unsafe manifest metadata should fail");
            assert!(error.to_string().contains(expected), "{error}");
        }
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
