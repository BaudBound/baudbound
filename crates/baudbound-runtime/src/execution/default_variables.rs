use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use baudbound_script::is_user_identifier;

use crate::{
    RuntimeDefaultVariable, RuntimeDefaultVariableScope, RuntimeStateStore, RuntimeVariableScope,
};

use super::RuntimeError;

const SUPPORTED_VARIABLE_TYPES: &[&str] = &[
    "string",
    "number",
    "boolean",
    "object",
    "list",
    "datetime",
    "duration",
    "file_path",
    "integer",
    "float",
    "color",
    "keyboard_key",
];
const SUPPORTED_LIST_ITEM_TYPES: &[&str] = &[
    "string",
    "number",
    "boolean",
    "object",
    "datetime",
    "duration",
    "file_path",
];

pub(super) fn validate_default_variables(
    default_variables: &[RuntimeDefaultVariable],
    declared_variables: &BTreeMap<String, (String, Option<String>)>,
    secret_names: &[String],
) -> Result<(), RuntimeError> {
    let mut names = BTreeSet::new();
    for variable in default_variables {
        validate_default_variable(variable)?;
        if !names.insert(variable.name.as_str()) {
            return Err(RuntimeError::InvalidGraph(format!(
                "manifest contains duplicate default variable {:?}",
                variable.name
            )));
        }
        if secret_names.iter().any(|name| name == &variable.name) {
            return Err(RuntimeError::InvalidGraph(format!(
                "default variable {:?} conflicts with a secret declaration",
                variable.name
            )));
        }
        if let Some((node_scope, node_type)) = declared_variables.get(&variable.name) {
            let default_scope = match variable.scope {
                RuntimeDefaultVariableScope::Runtime => "runtime",
                RuntimeDefaultVariableScope::Persistent => "persistent",
            };
            if node_scope != default_scope
                || node_type
                    .as_ref()
                    .is_some_and(|node_type| node_type != &variable.value_type)
            {
                return Err(RuntimeError::InvalidGraph(format!(
                    "default variable {:?} does not match Variable Operation scope and type",
                    variable.name
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn load_or_initialize_persistent_default(
    store: &dyn RuntimeStateStore,
    script_id: &str,
    variable: &RuntimeDefaultVariable,
) -> Result<Value, RuntimeError> {
    if let Some(stored) = store
        .load_variable(RuntimeVariableScope::Persistent, script_id, &variable.name)
        .map_err(RuntimeError::State)?
    {
        return Ok(stored.value);
    }

    if store
        .compare_and_set_variable(
            RuntimeVariableScope::Persistent,
            script_id,
            &variable.name,
            None,
            &variable.value,
        )
        .map_err(RuntimeError::State)?
    {
        return Ok(variable.value.clone());
    }

    store
        .load_variable(RuntimeVariableScope::Persistent, script_id, &variable.name)
        .map_err(RuntimeError::State)?
        .map(|stored| stored.value)
        .ok_or_else(|| {
            RuntimeError::State(format!(
                "persistent default {:?} could not be initialized after a concurrent update",
                variable.name
            ))
        })
}

fn validate_default_variable(variable: &RuntimeDefaultVariable) -> Result<(), RuntimeError> {
    if !is_user_identifier(&variable.name)
        || variable.name.starts_with("system_")
        || variable.name.starts_with("manifest_")
    {
        return Err(RuntimeError::InvalidGraph(format!(
            "default variable name {:?} is invalid or reserved",
            variable.name
        )));
    }
    if !SUPPORTED_VARIABLE_TYPES.contains(&variable.value_type.as_str()) {
        return Err(RuntimeError::InvalidGraph(format!(
            "default variable {:?} uses unsupported type {:?}",
            variable.name, variable.value_type
        )));
    }
    if variable.value_type == "list" {
        match variable.item_type.as_deref() {
            Some(item_type) if SUPPORTED_LIST_ITEM_TYPES.contains(&item_type) => {}
            Some(item_type) => {
                return Err(RuntimeError::InvalidGraph(format!(
                    "default variable {:?} uses unsupported list item type {item_type:?}",
                    variable.name
                )));
            }
            None => {
                return Err(RuntimeError::InvalidGraph(format!(
                    "default variable {:?} must declare a list item type",
                    variable.name
                )));
            }
        }
    } else if variable.item_type.is_some() {
        return Err(RuntimeError::InvalidGraph(format!(
            "default variable {:?} declares an item type but is not a list",
            variable.name
        )));
    }
    if !value_matches_type(
        &variable.value_type,
        variable.item_type.as_deref(),
        &variable.value,
    ) {
        return Err(RuntimeError::InvalidGraph(format!(
            "default variable {:?} value does not match type {}",
            variable.name, variable.value_type
        )));
    }
    Ok(())
}

fn value_matches_type(value_type: &str, item_type: Option<&str>, value: &Value) -> bool {
    match value_type {
        // The types introduced by the type rework are validated against the
        // shared `ValueType` vocabulary once the default is placed into the
        // initial state (see `initial_state.rs`), rather than duplicating
        // that logic here. Accepting the type name at this stage only
        // confirms it is a recognized declaration.
        "integer" | "float" | "color" | "keyboard_key" => true,
        "string" => value.as_str().is_some_and(|text| !text.trim().is_empty()),
        "file_path" => value.as_str().is_some_and(|path| !path.trim().is_empty()),
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
                && object.get("value").is_some_and(Value::is_string)
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
