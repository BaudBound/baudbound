use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::runtime::{
    DERIVED_VARIABLE_METADATA_SUFFIXES, MANIFEST_VARIABLE, RuntimeGraph, SETTINGS_VARIABLE,
    SYSTEM_VARIABLE, refresh_derived_variable_metadata, required_config_string,
    validate_variable_name,
};
use crate::{
    RuntimeDefaultVariable, RuntimeDefaultVariableScope, RuntimeScriptSettings,
    RuntimeSecretDeclaration, RuntimeStateStore, RuntimeVariableScope, ValueType, validate_value,
};

use super::default_variables::{load_or_initialize_persistent_default, validate_default_variables};
use super::{RunVariableScope, RuntimeError};

pub(super) struct InitialRuntimeState {
    pub(super) secret_names: Vec<String>,
    pub(super) secret_values: Vec<Value>,
    pub(super) variable_scopes: BTreeMap<String, RunVariableScope>,
    pub(super) variables: BTreeMap<String, Value>,
}

/// Everything a run is handed that no script can write: the `@` namespaces.
///
/// One argument rather than three, because they arrive together and are seeded
/// together, and because the caller reads better naming them than counting
/// positional maps.
pub(super) struct BuiltIns<'a> {
    pub(super) manifest: &'a BTreeMap<String, Value>,
    pub(super) run_id: &'a str,
    pub(super) started_at: Value,
    pub(super) system: &'a BTreeMap<String, Value>,
    pub(super) trigger_id: &'a str,
    pub(super) trigger_type: &'a str,
}

pub(super) fn load_initial_state(
    graph: &RuntimeGraph,
    script_id: &str,
    state_store: Option<&dyn RuntimeStateStore>,
    default_variables: &[RuntimeDefaultVariable],
    script_settings: Option<&RuntimeScriptSettings>,
    secrets: &[RuntimeSecretDeclaration],
    built_ins: &BuiltIns<'_>,
) -> Result<InitialRuntimeState, RuntimeError> {
    let mut variables = BTreeMap::new();
    let mut variable_scopes = BTreeMap::new();
    let mut declarations = BTreeMap::<String, RuntimeVariableScope>::new();
    let mut declared_variables = BTreeMap::<String, (String, Option<String>)>::new();

    for node in graph
        .nodes()
        .filter(|node| node.action_type == "runtime.set_variable")
    {
        let name = required_config_string(node, "name")?;
        validate_variable_name(node, &name)?;
        let scope_name = required_config_string(node, "scope")?;
        let operation = required_config_string(node, "operation")?;
        let value_type = match operation.as_str() {
            "set" => Some(required_config_string(node, "valueType")?),
            // Increment does not pin a numeric type: it operates on whichever
            // numeric type the variable was declared with, and preserves it.
            "increment" => None,
            "toggle_boolean" => Some("boolean".to_owned()),
            "append_list" | "remove_list_items" => Some("list".to_owned()),
            "set_object_field" | "remove_object_field" | "merge_object" => {
                Some("object".to_owned())
            }
            "clear" | "delete" => None,
            invalid => {
                return Err(RuntimeError::VariableOperation {
                    node_id: node.id.clone(),
                    message: format!("unsupported variable operation {invalid}"),
                });
            }
        };
        let scope = match scope_name.as_str() {
            "runtime" => None,
            "persistent" => Some(RuntimeVariableScope::Persistent),
            "global" => Some(RuntimeVariableScope::Global),
            invalid => {
                return Err(RuntimeError::VariableOperation {
                    node_id: node.id.clone(),
                    message: format!("unsupported variable scope {invalid}"),
                });
            }
        };
        if let Some((existing_scope, existing_type)) = declared_variables.get_mut(&name) {
            if *existing_scope != scope_name
                || existing_type
                    .as_ref()
                    .zip(value_type.as_ref())
                    .is_some_and(|(existing, incoming)| existing != incoming)
            {
                return Err(RuntimeError::InvalidGraph(format!(
                    "variable {name:?} is declared with conflicting scope or type"
                )));
            }
            if existing_type.is_none() {
                *existing_type = value_type;
            }
        } else {
            declared_variables.insert(name.clone(), (scope_name.clone(), value_type));
        }
        if let Some(scope) = scope {
            declarations.insert(name.clone(), scope);
        }
    }

    let secret_names = secrets
        .iter()
        .map(|secret| secret.name.clone())
        .collect::<Vec<_>>();
    if secret_names.iter().collect::<BTreeSet<_>>().len() != secret_names.len() {
        return Err(RuntimeError::InvalidGraph(
            "manifest contains duplicate secret declarations".to_owned(),
        ));
    }
    if let Some(collision) = secret_names
        .iter()
        .find(|name| declared_variables.contains_key(name.as_str()))
    {
        return Err(RuntimeError::InvalidGraph(format!(
            "secret {collision:?} conflicts with a writable variable"
        )));
    }

    validate_default_variables(default_variables, &declared_variables, &secret_names)?;

    // "settings" used to be reserved here for the Script Settings object. That
    // object is now "@settings", which no script can name, so the plain word is
    // an ordinary variable name again.

    let has_persistent_default = default_variables
        .iter()
        .any(|variable| variable.scope == RuntimeDefaultVariableScope::Persistent);
    if (!declarations.is_empty() || has_persistent_default || !secrets.is_empty())
        && state_store.is_none()
    {
        return Err(RuntimeError::State(
            "persistent, global, and secret variables require a runner state store".to_owned(),
        ));
    }
    // One object rather than a name per field, and behind an "@" no user
    // identifier may contain, so nothing a script declares can shadow it.
    let mut system = serde_json::Map::new();
    for (name, value) in built_ins.system {
        system.insert(name.clone(), value.clone());
    }
    system.insert(
        "run_id".to_owned(),
        Value::String(built_ins.run_id.to_owned()),
    );
    system.insert(
        "trigger_id".to_owned(),
        Value::String(built_ins.trigger_id.to_owned()),
    );
    system.insert(
        "trigger_type".to_owned(),
        Value::String(built_ins.trigger_type.to_owned()),
    );
    system.insert("run_started_at".to_owned(), built_ins.started_at.clone());
    insert_initial_variable(
        &mut variables,
        &mut variable_scopes,
        SYSTEM_VARIABLE.to_owned(),
        Value::Object(system),
        RunVariableScope::System,
    );
    // The manifest was never supplied to a run at all: the editor offered
    // manifest_name and the like, and every one of them reached production as
    // literal braces, exactly as the system values used to.
    if !built_ins.manifest.is_empty() {
        insert_initial_variable(
            &mut variables,
            &mut variable_scopes,
            MANIFEST_VARIABLE.to_owned(),
            Value::Object(built_ins.manifest.clone().into_iter().collect()),
            RunVariableScope::Manifest,
        );
    }
    let mut secret_values = Vec::new();
    if let Some(settings) = script_settings {
        if !settings.values.is_object() {
            return Err(RuntimeError::State(
                "Script Settings must be provided as an object".to_owned(),
            ));
        }
        insert_initial_variable(
            &mut variables,
            &mut variable_scopes,
            SETTINGS_VARIABLE.to_owned(),
            settings.values.clone(),
            RunVariableScope::Setting,
        );
    }
    for variable in default_variables
        .iter()
        .filter(|variable| variable.scope == RuntimeDefaultVariableScope::Runtime)
    {
        reject_wrong_type_default(variable)?;
        insert_initial_variable(
            &mut variables,
            &mut variable_scopes,
            variable.name.clone(),
            variable.value.clone(),
            RunVariableScope::Runtime,
        );
    }
    if let Some(store) = state_store {
        for variable in default_variables
            .iter()
            .filter(|variable| variable.scope == RuntimeDefaultVariableScope::Persistent)
        {
            reject_wrong_type_default(variable)?;
            let value = load_or_initialize_persistent_default(store, script_id, variable)?;
            insert_initial_variable(
                &mut variables,
                &mut variable_scopes,
                variable.name.clone(),
                value,
                RunVariableScope::Persistent,
            );
        }
        for (name, scope) in declarations {
            if let Some(stored) = store
                .load_variable(scope, script_id, &name)
                .map_err(RuntimeError::State)?
            {
                let run_scope = match scope {
                    RuntimeVariableScope::Persistent => RunVariableScope::Persistent,
                    RuntimeVariableScope::Global => RunVariableScope::Global,
                };
                insert_initial_variable(
                    &mut variables,
                    &mut variable_scopes,
                    name,
                    stored.value,
                    run_scope,
                );
            }
        }
        for secret in secrets {
            match store
                .read_secret(script_id, &secret.name)
                .map_err(RuntimeError::State)?
            {
                Some(value) => {
                    validate_secret_value(secret, &value)?;
                    insert_initial_variable(
                        &mut variables,
                        &mut variable_scopes,
                        secret.name.clone(),
                        value.clone(),
                        RunVariableScope::Secret,
                    );
                    secret_values.push(value);
                }
                None if secret.required => {
                    return Err(RuntimeError::State(format!(
                        "required secret {:?} is not configured",
                        secret.name
                    )));
                }
                None => {}
            }
        }
    }

    Ok(InitialRuntimeState {
        secret_names,
        secret_values,
        variable_scopes,
        variables,
    })
}

/// Rejects a declared default whose value violates its declared type,
/// against the shared `ValueType` vocabulary. `node_id` is empty because a
/// declared default belongs to the package rather than to a node. A type
/// name outside the ten-type vocabulary (the retired vocabulary such as
/// `number` or `file_path`) does not parse into a `ValueType` and is left to
/// `validate_default_variable`, which already accepts it.
fn reject_wrong_type_default(variable: &RuntimeDefaultVariable) -> Result<(), RuntimeError> {
    let type_error = |reason: String| RuntimeError::Type {
        node_id: String::new(),
        message: format!("default variable \"{}\" {reason}", variable.name),
    };

    // A declared value is read by the type declared beside it, so a whole
    // number under a float declaration is that number as a float. The editor
    // writes JSON with JavaScript numbers and cannot spell 300.0 as anything
    // but `300`, so holding a declaration to the run time rule would make a
    // whole float impossible to declare at all.
    let declared_float = variable.value_type == "float" && variable.value.is_number();
    if !declared_float
        && let Ok(declared) = variable.value_type.parse::<ValueType>()
        && let Err(reason) = validate_value(&variable.value, declared)
    {
        return Err(type_error(reason));
    }

    // A list only satisfies `ValueType::List` by being an array, so its
    // elements have to be checked separately. Without this a list declared as
    // integers would accept a string element and nothing would ever notice.
    if let Some(item_type) = variable.item_type.as_deref()
        && let Ok(declared_item) = item_type.parse::<ValueType>()
        && let Some(items) = variable.value.as_array()
    {
        for (index, item) in items.iter().enumerate() {
            if let Err(reason) = validate_value(item, declared_item) {
                return Err(type_error(format!("item {index} {reason}")));
            }
        }
    }

    Ok(())
}

fn insert_initial_variable(
    variables: &mut BTreeMap<String, Value>,
    scopes: &mut BTreeMap<String, RunVariableScope>,
    name: String,
    value: Value,
    scope: RunVariableScope,
) {
    variables.insert(name.clone(), value);
    scopes.insert(name.clone(), scope);
    refresh_derived_variable_metadata(variables, &name);
    for suffix in DERIVED_VARIABLE_METADATA_SUFFIXES {
        scopes.insert(format!("{name}{suffix}"), RunVariableScope::Metadata);
    }
}

fn validate_secret_value(
    declaration: &RuntimeSecretDeclaration,
    value: &Value,
) -> Result<(), RuntimeError> {
    let valid = declaration.value_type == "string" && value.is_string();
    if valid {
        Ok(())
    } else {
        Err(RuntimeError::State(format!(
            "secret {:?} does not match declared type {}",
            declaration.name, declaration.value_type
        )))
    }
}

/// The `@system` fields that are readings rather than facts.
///
/// A machine fact cannot change while a run is in progress, so it is read once.
/// The clock and the uptime can, and a run is not short: `delay`, `repeat`,
/// `while` and `for-each` all exist and a script that loops forever is
/// supported. Read once per run, these reported the same value until the run
/// ended, which made the clock useless in exactly the scripts that needed it.
pub(super) fn live_system_fields() -> [(&'static str, Value); 2] {
    let uptime = sysinfo::System::uptime();
    [
        (
            "datetime",
            serde_json::json!({
                "type": "datetime",
                "value": chrono::Local::now().to_rfc3339(),
            }),
        ),
        (
            "uptime",
            serde_json::json!({
                "type": "duration",
                "unit": "seconds",
                "value": uptime,
            }),
        ),
    ]
}
