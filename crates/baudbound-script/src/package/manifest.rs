use std::collections::{BTreeMap, BTreeSet};

use crate::{Manifest, Program, is_user_identifier};
use semver::Version;
use url::Url;

use super::{PackageLoadError, finish_validation};

/// Variables, list elements and Script Settings all declare the same ten types.
/// A list element may be any of them except a list, so nesting stays one level.
const SUPPORTED_VARIABLE_TYPES: &[&str] = &[
    "string", "integer", "float", "boolean", "object", "list", "color", "hotkey", "datetime",
    "duration",
];
const SUPPORTED_LIST_ITEM_TYPES: &[&str] = &[
    "string", "integer", "float", "boolean", "object", "color", "hotkey", "datetime", "duration",
];
const SUPPORTED_SETTING_TYPES: &[&str] = SUPPORTED_VARIABLE_TYPES;
pub const MAX_SCRIPT_SETTING_CONTAINER_ITEMS: usize = 4096;
pub const MAX_SCRIPT_SETTING_VALUE_DEPTH: usize = 32;

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
    validate_repository_url(&manifest.repository_url, &mut errors);

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
    for setting in &manifest.settings {
        validate_text(
            "setting description",
            &setting.description,
            1024,
            true,
            false,
            &mut errors,
        );
        if setting.default_value.as_ref().is_some_and(|value| {
            serde_json::to_vec(value)
                .is_ok_and(|serialized| serialized.len() > MAX_DEFAULT_VALUE_BYTES)
        }) {
            errors.push(format!(
                "manifest setting {:?} default value exceeds {MAX_DEFAULT_VALUE_BYTES} bytes",
                setting.name
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

fn validate_repository_url(value: &str, errors: &mut Vec<String>) {
    validate_text("repository_url", value, 2048, false, false, errors);
    if value.is_empty() {
        return;
    }
    if crate::validate_public_https_repository_url(value).is_err() {
        errors.push(
            "manifest repository_url must be a public HTTPS URL without credentials, a query string, or a fragment and must end in repository.json"
                .to_owned(),
        );
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
        if secret.value_type != "string" {
            errors.push(format!(
                "manifest secret {:?} must use type \"string\", found {:?}",
                secret.name, secret.value_type
            ));
        }
    }
    finish_validation(errors)
}

pub(super) fn validate_manifest_settings(manifest: &Manifest) -> Result<(), PackageLoadError> {
    let mut errors = Vec::new();
    let mut names = BTreeSet::new();
    for setting in &manifest.settings {
        validate_name("setting", &setting.name, &mut errors);
        if !names.insert(setting.name.as_str()) {
            errors.push(format!(
                "duplicate manifest setting name {:?}",
                setting.name
            ));
        }
        if !SUPPORTED_SETTING_TYPES.contains(&setting.value_type.as_str()) {
            errors.push(format!(
                "manifest setting {:?} uses unsupported type {:?}",
                setting.name, setting.value_type
            ));
        } else if let Some(error) =
            validate_list_item_type(&setting.value_type, setting.item_type.as_deref())
        {
            errors.push(format!("manifest setting {:?} {error}", setting.name));
        } else if setting.default_value.as_ref().is_some_and(|value| {
            !value_matches_declared_type(&setting.value_type, setting.item_type.as_deref(), value)
        }) {
            errors.push(format!(
                "manifest setting {:?} default value does not match type {}",
                setting.name, setting.value_type
            ));
        } else if let Some(error) = setting
            .default_value
            .as_ref()
            .and_then(|value| validate_script_setting_value_limits(value).err())
        {
            errors.push(format!(
                "manifest setting {:?} default value {error}",
                setting.name
            ));
        }
    }
    finish_validation(errors)
}

pub fn validate_script_setting_value_limits(value: &serde_json::Value) -> Result<(), String> {
    validate_script_setting_value_at_depth(value, 0)
}

fn validate_script_setting_value_at_depth(
    value: &serde_json::Value,
    depth: usize,
) -> Result<(), String> {
    if depth > MAX_SCRIPT_SETTING_VALUE_DEPTH {
        return Err(format!(
            "exceeds the maximum nesting depth of {MAX_SCRIPT_SETTING_VALUE_DEPTH}"
        ));
    }
    match value {
        serde_json::Value::Array(items) => {
            if items.len() > MAX_SCRIPT_SETTING_CONTAINER_ITEMS {
                return Err(format!(
                    "contains more than {MAX_SCRIPT_SETTING_CONTAINER_ITEMS} list items"
                ));
            }
            for item in items {
                validate_script_setting_value_at_depth(item, depth + 1)?;
            }
        }
        serde_json::Value::Object(properties) => {
            if properties.len() > MAX_SCRIPT_SETTING_CONTAINER_ITEMS {
                return Err(format!(
                    "contains more than {MAX_SCRIPT_SETTING_CONTAINER_ITEMS} object properties"
                ));
            }
            for property in properties.values() {
                validate_script_setting_value_at_depth(property, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
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
        if !matches!(variable.scope.as_str(), "runtime" | "persistent" | "global") {
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
        } else if let Some(error) =
            validate_list_item_type(&variable.value_type, variable.item_type.as_deref())
        {
            errors.push(format!("manifest variable {:?} {error}", variable.name));
        } else if !value_matches_declared_type(
            &variable.value_type,
            variable.item_type.as_deref(),
            &variable.value,
        ) {
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
    let declared = manifest
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
        // This used to compare the node's scope and type against the
        // declaration's. A node carries neither now, so nothing can disagree;
        // what is left to catch is a write to a name the manifest never
        // declares, which a run refuses to start. Catching it at install time
        // means the refusal arrives before the script is on the machine.
        let Some(variable) = declared.get(name) else {
            errors.push(format!(
                "program.json writes variable {name:?}, which the manifest does not declare"
            ));
            continue;
        };
        let operation = config
            .get("operation")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("set");
        // An operation that implies a type has to agree with the declared one.
        // The editor will not offer Toggle boolean for a string, so a package
        // that asks for it was not built by one.
        let required_type = match operation {
            "toggle_boolean" => Some("boolean"),
            "append_list" | "remove_list_items" => Some("list"),
            "set_object_field" | "remove_object_field" | "merge_object" => Some("object"),
            _ => None,
        };
        if let Some(required) = required_type
            && required != variable.value_type
        {
            errors.push(format!(
                "program.json applies {operation} to variable {name:?}, which the manifest declares as {}",
                variable.value_type
            ));
        }
        if operation == "increment" && !matches!(variable.value_type.as_str(), "integer" | "float")
        {
            errors.push(format!(
                "program.json increments variable {name:?}, which the manifest declares as {}",
                variable.value_type
            ));
        }
    }

    finish_validation(errors)
}

fn validate_name(kind: &str, name: &str, errors: &mut Vec<String>) {
    if !is_user_identifier(name) {
        errors.push(format!(
            "manifest {kind} {name:?} may contain only ASCII letters, numbers, hyphens, and underscores"
        ));
    }
    if name.starts_with("system_") || name.starts_with("manifest_") {
        errors.push(format!(
            "manifest {kind} {name:?} uses a reserved variable prefix"
        ));
    }
    if kind != "setting" && name == "settings" {
        errors.push(format!(
            "manifest {kind} {name:?} uses the reserved Script Settings namespace"
        ));
    }
}

fn value_matches_declared_type(
    value_type: &str,
    item_type: Option<&str>,
    value: &serde_json::Value,
) -> bool {
    match value_type {
        // An empty string is a string. Requiring a non-empty one meant a
        // variable could not start empty, so scripts declared a sentinel like
        // "none" and explained it in the description — while `clear` writes ""
        // at run time, a value the declaration itself could not hold. A script
        // setting's default never had this rule.
        "string" => value.is_string(),
        "hotkey" => value.as_str().is_some_and(crate::is_hotkey),
        "color" => value.as_str().is_some_and(is_hex_color),
        // integer and float are disjoint, so a whole number is not a float and
        // a fractional one is not an integer.
        "integer" => crate::is_safe_integer(value),
        "float" => value.is_f64(),
        "boolean" => value.is_boolean(),
        "list" => value.as_array().is_some_and(|items| {
            item_type.is_some_and(|item_type| {
                items
                    .iter()
                    .all(|item| value_matches_declared_type(item_type, None, item))
            })
        }),
        "object" => value.is_object(),
        "datetime" => value.as_object().is_some_and(|object| {
            object.get("type").and_then(serde_json::Value::as_str) == Some("datetime")
                && object
                    .get("value")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok())
        }),
        "duration" => value.as_object().is_some_and(|object| {
            object.get("type").and_then(serde_json::Value::as_str) == Some("duration")
                && object
                    .get("unit")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|unit| {
                        matches!(
                            unit,
                            "milliseconds" | "seconds" | "minutes" | "hours" | "days"
                        )
                    })
                && object
                    .get("value")
                    .and_then(serde_json::Value::as_f64)
                    .is_some_and(|value| value.is_finite() && value >= 0.0)
        }),
        _ => false,
    }
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

fn validate_list_item_type(value_type: &str, item_type: Option<&str>) -> Option<String> {
    if value_type == "list" {
        match item_type {
            Some(item_type) if SUPPORTED_LIST_ITEM_TYPES.contains(&item_type) => None,
            Some(item_type) => Some(format!("uses unsupported list item type {item_type:?}")),
            None => Some("must declare item_type because its type is list".to_owned()),
        }
    } else if item_type.is_some() {
        Some("cannot declare item_type unless its type is list".to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_manifest_declared_variables() {
        let manifest = manifest_with_variables(serde_json::json!([{
            "name": "Counter-2_primary",
            "scope": "persistent",
            "type": "integer",
            "description": "Retained counter",
            "value": 10
        }]));

        validate_manifest_variables(&manifest).expect("valid declared variable should pass");
    }

    #[test]
    fn validates_typed_script_setting_defaults() {
        let manifest = manifest_with_settings(serde_json::json!([
            {
                "name": "API-endpoint_2",
                "type": "string",
                "description": "Service endpoint",
                "required": true,
                "default_value": "https://example.test"
            },
            {
                "name": "retries",
                "type": "integer",
                "default_value": 3
            },
            {
                "name": "enabled",
                "type": "boolean",
                "default_value": true
            },
            {
                "name": "headers",
                "type": "object",
                "default_value": {"Accept": "application/json"}
            },
            {
                "name": "labels",
                "type": "list",
                "item_type": "string",
                "default_value": ["one", "two"]
            },
            {
                "name": "started_at",
                "type": "datetime",
                "default_value": {
                    "type": "datetime",
                    "value": "2026-07-29T12:00:00Z"
                }
            },
            {
                "name": "timeout",
                "type": "duration",
                "default_value": {
                    "type": "duration",
                    "unit": "seconds",
                    "value": 30
                }
            },
            {
                "name": "output_path",
                "type": "string",
                "default_value": "/tmp/output.txt"
            },
            {
                "name": "shortcut",
                "type": "hotkey",
                "default_value": "Ctrl+Shift+F8"
            },
            {
                "name": "accent",
                "type": "color",
                "default_value": "#1A2b3C"
            }
        ]));

        validate_manifest_settings(&manifest).expect("valid Script Settings should pass");
    }

    #[test]
    fn rejects_invalid_script_setting_declarations() {
        for (settings, expected) in [
            (
                serde_json::json!([
                    {"name": "mode", "type": "string"},
                    {"name": "mode", "type": "string"}
                ]),
                "duplicate manifest setting name",
            ),
            (
                serde_json::json!([
                    {"name": "mode", "type": "http_response"}
                ]),
                "unsupported type",
            ),
            (
                serde_json::json!([
                    {"name": "retries", "type": "integer", "default_value": "three"}
                ]),
                "default value does not match type",
            ),
            (
                serde_json::json!([
                    {"name": "accent", "type": "color", "default_value": "#12345G"}
                ]),
                "default value does not match type",
            ),
            (
                serde_json::json!([
                    {
                        "name": "mixed",
                        "type": "list",
                        "item_type": "string",
                        "default_value": ["one", 2]
                    }
                ]),
                "default value does not match type",
            ),
            (
                serde_json::json!([
                    {
                        "name": "started_at",
                        "type": "datetime",
                        "default_value": {
                            "type": "datetime",
                            "value": "not-a-date"
                        }
                    }
                ]),
                "default value does not match type",
            ),
        ] {
            let error = validate_manifest_settings(&manifest_with_settings(settings))
                .expect_err("invalid Script Settings should fail");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn rejects_script_setting_defaults_that_exceed_structural_limits() {
        let oversized = serde_json::Value::Array(
            (0..=MAX_SCRIPT_SETTING_CONTAINER_ITEMS)
                .map(|_| serde_json::Value::String("value".to_owned()))
                .collect(),
        );
        let error = validate_manifest_settings(&manifest_with_settings(serde_json::json!([{
            "name": "items",
            "type": "list",
            "item_type": "string",
            "default_value": oversized
        }])))
        .expect_err("oversized Script Setting default should fail");
        assert!(error.to_string().contains("list items"), "{error}");
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
    fn rejects_invalid_manifest_declared_variables() {
        for (variables, expected) in [
            (
                serde_json::json!([{"name": "counter", "scope": "session", "type": "integer", "value": 10}]),
                "unsupported scope",
            ),
            (
                serde_json::json!([{"name": "counter", "scope": "runtime", "type": "integer", "value": "ten"}]),
                "does not match type",
            ),
            (
                serde_json::json!([{"name": "label", "scope": "runtime", "type": "string", "value": 5}]),
                "does not match type",
            ),
            (
                serde_json::json!([{"name": "settings", "scope": "runtime", "type": "string", "value": "reserved"}]),
                "reserved Script Settings namespace",
            ),
            // A list satisfies its own type by being an array, so each element
            // has to be checked against the item type in its own right.
            (
                serde_json::json!([{"name": "keys", "scope": "runtime", "type": "list", "item_type": "hotkey", "value": ["NotARealKey"]}]),
                "does not match type",
            ),
            (
                serde_json::json!([{"name": "counts", "scope": "runtime", "type": "list", "item_type": "integer", "value": ["not a number"]}]),
                "does not match type",
            ),
        ] {
            let error = validate_manifest_variables(&manifest_with_variables(variables))
                .expect_err("invalid declared variable should fail");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn a_string_variable_may_be_declared_empty() {
        // The empty string is what clear writes, so a declaration has to be
        // able to hold it. Without this a script had to invent a sentinel.
        let variables =
            serde_json::json!([{"name": "label", "scope": "runtime", "type": "string", "value": ""}]);
        validate_manifest_variables(&manifest_with_variables(variables))
            .expect("an empty string is a valid declared string value");
    }

    #[test]
    fn accepts_every_declarable_scope() {
        // Global was refused here while the schema already allowed it, which
        // meant no package declaring one could be installed at all.
        for scope in ["runtime", "persistent", "global"] {
            let variables =
                serde_json::json!([{"name": "counter", "scope": scope, "type": "integer", "value": 10}]);
            validate_manifest_variables(&manifest_with_variables(variables))
                .unwrap_or_else(|error| panic!("{scope} should be declarable: {error}"));
        }
    }

    #[test]
    fn rejects_a_write_to_a_variable_the_manifest_does_not_declare() {
        let manifest = manifest_with_variables(serde_json::json!([{
            "name": "counter",
            "scope": "persistent",
            "type": "integer",
            "value": 10
        }]));
        let program = serde_json::json!({
            "entry": {"program": {"steps": [{
                "action_type": "runtime.set_variable",
                "config": {"name": "tally", "operation": "set"}
            }]}}
        });

        let error = validate_manifest_variable_operations(&manifest, &program)
            .expect_err("an undeclared write should fail");
        let message = error.to_string();
        assert!(message.contains("tally"), "{message}");
        assert!(message.contains("does not declare"), "{message}");
    }

    #[test]
    fn rejects_an_operation_the_declared_type_cannot_take() {
        let manifest = manifest_with_variables(serde_json::json!([{
            "name": "label",
            "scope": "runtime",
            "type": "string",
            "value": "ready"
        }]));
        // The editor does not offer Toggle boolean for a string, so a package
        // asking for one was not built by it.
        let program = serde_json::json!({
            "entry": {"program": {"steps": [{
                "action_type": "runtime.set_variable",
                "config": {"name": "label", "operation": "toggle_boolean"}
            }]}}
        });

        let error = validate_manifest_variable_operations(&manifest, &program)
            .expect_err("a mismatched operation should fail");
        let message = error.to_string();
        assert!(message.contains("toggle_boolean"), "{message}");
        assert!(message.contains("string"), "{message}");
    }

    #[test]
    fn accepts_a_program_whose_writes_are_all_declared() {
        let manifest = manifest_with_variables(serde_json::json!([{
            "name": "counter",
            "scope": "persistent",
            "type": "integer",
            "value": 10
        }]));
        let program = serde_json::json!({
            "entry": {"program": {"steps": [{
                "action_type": "runtime.set_variable",
                "config": {"name": "counter", "operation": "increment", "value": 1}
            }]}}
        });

        validate_manifest_variable_operations(&manifest, &program)
            .expect("a declared write should pass");
    }

    fn manifest_with_variables(variables: serde_json::Value) -> Manifest {
        serde_json::from_value(serde_json::json!({
            "format_version": 1,
            "script_language_version": 1,
            "id": "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7",
            "name": "default-variables",
            "version": "1.0.0",
            "created_with": "BaudBound Test",
            "created_at": "2026-01-01T00:00:00.000Z",
            "minimum_runner_version": "0.1.0",
            "variables": variables
        }))
        .expect("test manifest should deserialize")
    }

    fn manifest_with_settings(settings: serde_json::Value) -> Manifest {
        serde_json::from_value(serde_json::json!({
            "format_version": 1,
            "script_language_version": 1,
            "id": "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7",
            "name": "script-settings",
            "version": "1.0.0",
            "created_with": "BaudBound Test",
            "created_at": "2026-01-01T00:00:00.000Z",
            "minimum_runner_version": "0.1.0",
            "settings": settings
        }))
        .expect("test manifest should deserialize")
    }
}
