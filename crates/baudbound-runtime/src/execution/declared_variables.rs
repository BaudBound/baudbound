use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use baudbound_script::is_user_identifier;

use crate::{
    RuntimeDeclaredScope, RuntimeDeclaredVariable, RuntimeStateStore, RuntimeVariableScope,
};

use super::RuntimeError;

/// The ten declarable types. A list element may be any of them except a list,
/// so nesting stays one level.
const SUPPORTED_VARIABLE_TYPES: &[&str] = &[
    "string", "integer", "float", "boolean", "object", "list", "color", "hotkey", "datetime",
    "duration",
];
const SUPPORTED_LIST_ITEM_TYPES: &[&str] = &[
    "string", "integer", "float", "boolean", "object", "color", "hotkey", "datetime", "duration",
];

pub(super) fn validate_declared_variables(
    declared_variables: &[RuntimeDeclaredVariable],
    graph_declarations: &BTreeMap<String, (String, Option<String>)>,
    secret_names: &[String],
) -> Result<(), RuntimeError> {
    let mut names = BTreeSet::new();
    for variable in declared_variables {
        validate_declared_variable(variable)?;
        if !names.insert(variable.name.as_str()) {
            return Err(RuntimeError::InvalidGraph(format!(
                "manifest contains duplicate declared variable {:?}",
                variable.name
            )));
        }
        if secret_names.iter().any(|name| name == &variable.name) {
            return Err(RuntimeError::InvalidGraph(format!(
                "declared variable {:?} conflicts with a secret declaration",
                variable.name
            )));
        }
        if let Some((node_scope, node_type)) = graph_declarations.get(&variable.name) {
            let default_scope = match variable.scope {
                RuntimeDeclaredScope::Global => "global",
                RuntimeDeclaredScope::Runtime => "runtime",
                RuntimeDeclaredScope::Persistent => "persistent",
            };
            if node_scope != default_scope
                || node_type
                    .as_ref()
                    .is_some_and(|node_type| node_type != &variable.value_type)
            {
                return Err(RuntimeError::InvalidGraph(format!(
                    "declared variable {:?} does not match Variable Operation scope and type",
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
    variable: &RuntimeDeclaredVariable,
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

fn validate_declared_variable(variable: &RuntimeDeclaredVariable) -> Result<(), RuntimeError> {
    if !is_user_identifier(&variable.name)
        || variable.name.starts_with("system_")
        || variable.name.starts_with("manifest_")
    {
        return Err(RuntimeError::InvalidGraph(format!(
            "declared variable name {:?} is invalid or reserved",
            variable.name
        )));
    }
    if !SUPPORTED_VARIABLE_TYPES.contains(&variable.value_type.as_str()) {
        return Err(RuntimeError::InvalidGraph(format!(
            "declared variable {:?} uses unsupported type {:?}",
            variable.name, variable.value_type
        )));
    }
    if variable.value_type == "list" {
        match variable.item_type.as_deref() {
            Some(item_type) if SUPPORTED_LIST_ITEM_TYPES.contains(&item_type) => {}
            Some(item_type) => {
                return Err(RuntimeError::InvalidGraph(format!(
                    "declared variable {:?} uses unsupported list item type {item_type:?}",
                    variable.name
                )));
            }
            None => {
                return Err(RuntimeError::InvalidGraph(format!(
                    "declared variable {:?} must declare a list item type",
                    variable.name
                )));
            }
        }
    } else if variable.item_type.is_some() {
        return Err(RuntimeError::InvalidGraph(format!(
            "declared variable {:?} declares an item type but is not a list",
            variable.name
        )));
    }
    if !value_matches_type(
        &variable.value_type,
        variable.item_type.as_deref(),
        &variable.value,
    ) {
        return Err(RuntimeError::InvalidGraph(format!(
            "declared variable {:?} value does not match type {}",
            variable.name, variable.value_type
        )));
    }
    Ok(())
}

fn value_matches_type(value_type: &str, item_type: Option<&str>, value: &Value) -> bool {
    match value_type {
        // Checked against the shared vocabulary rather than accepted on sight.
        // These once returned true here on the grounds that `initial_state.rs`
        // validates the default afterwards, but that check sees only the whole
        // value: for a list it confirms the value is an array and never looks
        // at the elements, so a list of integers would accept a string.
        // Only the declaration is confirmed here. The value itself is checked
        // against the shared vocabulary in `reject_wrong_type_default`, which
        // reports a type error rather than an invalid graph, so a mismatch
        // takes the same path as a failed cast.
        "integer" | "float" | "color" | "hotkey" => true,
        "string" => value.as_str().is_some_and(|text| !text.trim().is_empty()),
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
