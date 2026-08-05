use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::runtime::{
    DERIVED_VARIABLE_METADATA_SUFFIXES, RuntimeGraph, refresh_derived_variable_metadata,
    required_config_string, validate_variable_name,
};
use crate::{
    RuntimeDefaultVariable, RuntimeDefaultVariableScope, RuntimeScriptSettings,
    RuntimeSecretDeclaration, RuntimeStateStore, RuntimeVariableScope,
};

use super::default_variables::{load_or_initialize_persistent_default, validate_default_variables};
use super::{RunVariableScope, RuntimeError};

pub(super) struct InitialRuntimeState {
    pub(super) secret_names: Vec<String>,
    pub(super) secret_values: Vec<Value>,
    pub(super) variable_scopes: BTreeMap<String, RunVariableScope>,
    pub(super) variables: BTreeMap<String, Value>,
}

pub(super) fn load_initial_state(
    graph: &RuntimeGraph,
    script_id: &str,
    state_store: Option<&dyn RuntimeStateStore>,
    default_variables: &[RuntimeDefaultVariable],
    script_settings: Option<&RuntimeScriptSettings>,
    secrets: &[RuntimeSecretDeclaration],
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
    if declared_variables.contains_key("settings")
        || default_variables
            .iter()
            .any(|variable| variable.name == "settings")
        || secret_names.iter().any(|name| name == "settings")
    {
        return Err(RuntimeError::InvalidGraph(
            "\"settings\" is reserved for the read-only Script Settings object".to_owned(),
        ));
    }

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
            "settings".to_owned(),
            settings.values.clone(),
            RunVariableScope::Setting,
        );
    }
    for variable in default_variables
        .iter()
        .filter(|variable| variable.scope == RuntimeDefaultVariableScope::Runtime)
    {
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
