use std::collections::BTreeMap;

use baudbound_runtime::RuntimeScriptSettings;
use baudbound_script::{
    ScriptPackage, ScriptSettingDeclaration, validate_script_setting_value_limits,
};
use baudbound_storage::{ScriptStore, StoredScriptSetting};
use baudbound_triggers::normalize_windows_hotkey;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::{CoreError, RunnerCore, load_verified_installed_package};

pub const MAX_SCRIPT_SETTING_INPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InstalledScriptSettingStatus {
    pub configured: bool,
    pub configured_value: Option<Value>,
    pub default_value: Option<Value>,
    pub description: String,
    pub effective_value: Option<Value>,
    pub item_type: Option<String>,
    pub name: String,
    pub required: bool,
    pub updated_at_unix: Option<u64>,
    pub value_type: String,
}

pub(crate) fn list_installed_script_settings(
    core: &RunnerCore,
    store: &impl ScriptStore,
    reference: &str,
) -> Result<Vec<InstalledScriptSettingStatus>, CoreError> {
    let (_, package) = load_installed_package(core, store, reference)?;
    let configured = store
        .list_script_settings(reference)?
        .into_iter()
        .map(|setting| (setting.name.clone(), setting))
        .collect::<BTreeMap<_, _>>();
    Ok(package
        .manifest
        .settings
        .iter()
        .map(|declaration| merge_status(declaration, configured.get(&declaration.name)))
        .collect())
}

pub(crate) fn set_installed_script_setting_from_text(
    core: &RunnerCore,
    store: &impl ScriptStore,
    reference: &str,
    name: &str,
    input: &str,
) -> Result<InstalledScriptSettingStatus, CoreError> {
    let (_, package) = load_installed_package(core, store, reference)?;
    let declaration = package
        .manifest
        .settings
        .iter()
        .find(|setting| setting.name == name)
        .ok_or_else(|| {
            CoreError::InvalidSetting(format!("{name:?} is not declared by this script"))
        })?;
    let value = parse_setting_value(declaration, input)?;
    let stored = store.set_script_setting(reference, name, &value)?;
    Ok(merge_status(declaration, Some(&stored)))
}

pub(crate) fn save_installed_script_settings_from_text(
    core: &RunnerCore,
    store: &impl ScriptStore,
    reference: &str,
    inputs: &BTreeMap<String, String>,
) -> Result<Vec<InstalledScriptSettingStatus>, CoreError> {
    let (_, package) = load_installed_package(core, store, reference)?;
    let declarations = package
        .manifest
        .settings
        .iter()
        .map(|setting| (setting.name.as_str(), setting))
        .collect::<BTreeMap<_, _>>();
    let mut values = BTreeMap::new();
    for (name, input) in inputs {
        let declaration = declarations.get(name.as_str()).ok_or_else(|| {
            CoreError::InvalidSetting(format!("{name:?} is not declared by this script"))
        })?;
        values.insert(name.clone(), parse_setting_value(declaration, input)?);
    }

    let stored = store
        .replace_script_settings(reference, &values)?
        .into_iter()
        .map(|setting| (setting.name.clone(), setting))
        .collect::<BTreeMap<_, _>>();
    Ok(package
        .manifest
        .settings
        .iter()
        .map(|declaration| merge_status(declaration, stored.get(&declaration.name)))
        .collect())
}

pub(crate) fn remove_installed_script_setting(
    core: &RunnerCore,
    store: &impl ScriptStore,
    reference: &str,
    name: &str,
) -> Result<bool, CoreError> {
    let (_, package) = load_installed_package(core, store, reference)?;
    if !package
        .manifest
        .settings
        .iter()
        .any(|setting| setting.name == name)
    {
        return Err(CoreError::InvalidSetting(format!(
            "{name:?} is not declared by this script"
        )));
    }
    store
        .remove_script_setting(reference, name)
        .map_err(CoreError::Storage)
}

pub(crate) fn resolve_runtime_script_settings(
    store: &impl ScriptStore,
    reference: &str,
    package: &ScriptPackage,
) -> Result<RuntimeScriptSettings, CoreError> {
    let configured = store
        .list_script_settings(reference)?
        .into_iter()
        .map(|setting| (setting.name, setting.value))
        .collect::<BTreeMap<_, _>>();
    let mut values = Map::new();
    for declaration in &package.manifest.settings {
        let value = configured
            .get(&declaration.name)
            .cloned()
            .or_else(|| declaration.default_value.clone());
        match value {
            Some(value) => {
                validate_setting_value(declaration, &value)?;
                values.insert(declaration.name.clone(), value);
            }
            None if declaration.required => {
                return Err(CoreError::InvalidSetting(format!(
                    "required Script Setting {:?} has no configured value or package default",
                    declaration.name
                )));
            }
            None => {
                values.insert(declaration.name.clone(), Value::Null);
            }
        }
    }
    Ok(RuntimeScriptSettings {
        values: Value::Object(values),
    })
}

fn load_installed_package(
    core: &RunnerCore,
    store: &impl ScriptStore,
    reference: &str,
) -> Result<(baudbound_storage::InstalledScript, ScriptPackage), CoreError> {
    let (installed, _staged_package, package) = load_verified_installed_package(store, reference)?;
    core.validate_package_compatibility(&package)?;
    Ok((installed, package))
}

fn merge_status(
    declaration: &ScriptSettingDeclaration,
    stored: Option<&StoredScriptSetting>,
) -> InstalledScriptSettingStatus {
    let configured_value = stored.map(|setting| setting.value.clone());
    InstalledScriptSettingStatus {
        configured: stored.is_some(),
        configured_value: configured_value.clone(),
        default_value: declaration.default_value.clone(),
        description: declaration.description.clone(),
        effective_value: configured_value.or_else(|| declaration.default_value.clone()),
        item_type: declaration.item_type.clone(),
        name: declaration.name.clone(),
        required: declaration.required,
        updated_at_unix: stored.map(|setting| setting.updated_at_unix),
        value_type: declaration.value_type.clone(),
    }
}

fn parse_setting_value(
    declaration: &ScriptSettingDeclaration,
    input: &str,
) -> Result<Value, CoreError> {
    if input.len() > MAX_SCRIPT_SETTING_INPUT_BYTES {
        return Err(CoreError::InvalidSetting(format!(
            "value exceeds the maximum size of {MAX_SCRIPT_SETTING_INPUT_BYTES} bytes"
        )));
    }
    let value = match declaration.value_type.as_str() {
        "string" | "file_path" | "hotkey" | "color" => Ok(Value::String(input.to_owned())),
        "number" => input
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .ok_or_else(|| CoreError::InvalidSetting("expected a finite number".to_owned())),
        "boolean" => input
            .parse::<bool>()
            .map(Value::Bool)
            .map_err(|_| CoreError::InvalidSetting("expected true or false".to_owned())),
        "list" => parse_json_container(input, false),
        "object" => parse_json_container(input, true),
        "datetime" | "duration" => parse_json_container(input, true),
        invalid => Err(CoreError::InvalidSetting(format!(
            "unsupported declared type {invalid:?}"
        ))),
    }?;
    validate_setting_value(declaration, &value)?;
    Ok(value)
}

fn parse_json_container(input: &str, object: bool) -> Result<Value, CoreError> {
    let value = serde_json::from_str::<Value>(input)
        .map_err(|error| CoreError::InvalidSetting(format!("expected valid JSON: {error}")))?;
    if (object && value.is_object()) || (!object && value.is_array()) {
        validate_script_setting_value_limits(&value).map_err(CoreError::InvalidSetting)?;
        Ok(value)
    } else {
        Err(CoreError::InvalidSetting(format!(
            "expected a JSON {}",
            if object { "object" } else { "list" }
        )))
    }
}

fn validate_setting_value(
    declaration: &ScriptSettingDeclaration,
    value: &Value,
) -> Result<(), CoreError> {
    if value_matches_type(
        &declaration.value_type,
        declaration.item_type.as_deref(),
        value,
    ) {
        Ok(())
    } else {
        Err(CoreError::InvalidSetting(format!(
            "Script Setting {:?} does not match declared type {}",
            declaration.name, declaration.value_type
        )))
    }
}

pub(crate) fn value_matches_type(value_type: &str, item_type: Option<&str>, value: &Value) -> bool {
    match value_type {
        "string" => value.is_string(),
        "file_path" => value.as_str().is_some_and(|path| !path.trim().is_empty()),
        "hotkey" => value
            .as_str()
            .is_some_and(|key| normalize_windows_hotkey(key).is_ok()),
        "color" => value.as_str().is_some_and(is_hex_color),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "list" => value.as_array().is_some_and(|items| {
            item_type.is_some_and(|item_type| {
                items
                    .iter()
                    .all(|item| value_matches_type(item_type, None, item))
            })
        }),
        "object" => value.is_object(),
        "datetime" => value.as_object().is_some_and(|object| {
            object.get("type").and_then(Value::as_str) == Some("datetime")
                && object
                    .get("value")
                    .and_then(Value::as_str)
                    .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok())
        }),
        "duration" => value.as_object().is_some_and(|object| {
            object.get("type").and_then(Value::as_str) == Some("duration")
                && object
                    .get("unit")
                    .and_then(Value::as_str)
                    .is_some_and(|unit| {
                        matches!(
                            unit,
                            "milliseconds" | "seconds" | "minutes" | "hours" | "days"
                        )
                    })
                && object
                    .get("value")
                    .and_then(Value::as_f64)
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_supported_setting_values_from_cli_and_desktop_text() {
        for (value_type, input, expected) in [
            ("string", "plain text", json!("plain text")),
            ("number", "12.5", json!(12.5)),
            ("boolean", "true", json!(true)),
            ("list", "[1,2,3]", json!([1, 2, 3])),
            ("object", "{\"enabled\":true}", json!({"enabled": true})),
            (
                "datetime",
                "{\"type\":\"datetime\",\"value\":\"2026-07-29T12:00:00Z\"}",
                json!({"type": "datetime", "value": "2026-07-29T12:00:00Z"}),
            ),
            (
                "duration",
                "{\"type\":\"duration\",\"unit\":\"minutes\",\"value\":5}",
                json!({"type": "duration", "unit": "minutes", "value": 5}),
            ),
            ("file_path", "/tmp/output.txt", json!("/tmp/output.txt")),
            ("hotkey", "Ctrl+Shift+F8", json!("Ctrl+Shift+F8")),
            ("color", "#1A2b3C", json!("#1A2b3C")),
        ] {
            assert_eq!(
                parse_setting_value(
                    &declaration(
                        value_type,
                        if value_type == "list" {
                            Some("number")
                        } else {
                            None
                        },
                    ),
                    input,
                )
                .expect("valid setting input should parse"),
                expected
            );
        }
    }

    #[test]
    fn rejects_invalid_or_oversized_setting_values() {
        for (value_type, input, expected) in [
            ("number", "NaN", "expected a finite number"),
            ("boolean", "yes", "expected true or false"),
            ("list", "{}", "expected a JSON list"),
            ("object", "[]", "expected a JSON object"),
            ("datetime", "now", "expected valid JSON"),
            (
                "datetime",
                "{\"type\":\"datetime\",\"value\":\"not-a-date\"}",
                "does not match declared type",
            ),
            (
                "duration",
                "{\"type\":\"duration\",\"unit\":\"weeks\",\"value\":1}",
                "does not match declared type",
            ),
            ("file_path", "   ", "does not match declared type"),
            (
                "hotkey",
                "Ctrl+DefinitelyNotAKey",
                "does not match declared type",
            ),
            ("color", "red", "does not match declared type"),
            ("color", "#12345G", "does not match declared type"),
            ("list", "[\"one\",2]", "does not match declared type"),
        ] {
            let error = parse_setting_value(&declaration(value_type, None), input)
                .expect_err("invalid setting input should fail");
            assert!(error.to_string().contains(expected), "{error}");
        }

        let oversized = "x".repeat(MAX_SCRIPT_SETTING_INPUT_BYTES + 1);
        let error = parse_setting_value(&declaration("string", None), &oversized)
            .expect_err("oversized setting input should fail");
        assert!(error.to_string().contains("maximum size"), "{error}");

        let oversized_list = serde_json::to_string(&vec![
            Value::Null;
            baudbound_script::MAX_SCRIPT_SETTING_CONTAINER_ITEMS
                + 1
        ])
        .expect("test list should serialize");
        let error = parse_setting_value(&declaration("list", Some("string")), &oversized_list)
            .expect_err("oversized setting list should fail");
        assert!(error.to_string().contains("list items"), "{error}");
    }

    fn declaration(value_type: &str, item_type: Option<&str>) -> ScriptSettingDeclaration {
        ScriptSettingDeclaration {
            name: "test".to_owned(),
            value_type: value_type.to_owned(),
            item_type: item_type.map(str::to_owned),
            description: String::new(),
            required: false,
            default_value: None,
        }
    }
}
