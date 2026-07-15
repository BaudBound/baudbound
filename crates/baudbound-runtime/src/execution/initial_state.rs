use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::runtime::{
    RuntimeGraph, refresh_derived_variable_metadata, required_config_string, validate_variable_name,
};
use crate::{
    RuntimeDefaultVariable, RuntimeDefaultVariableScope, RuntimeSecretDeclaration,
    RuntimeStateStore, RuntimeVariableScope,
};

use super::RuntimeError;
use super::default_variables::{load_or_initialize_persistent_default, validate_default_variables};

pub(super) struct InitialRuntimeState {
    pub(super) secret_names: Vec<String>,
    pub(super) secret_values: Vec<Value>,
    pub(super) variables: BTreeMap<String, Value>,
}

pub(super) fn load_initial_state(
    graph: &RuntimeGraph,
    script_id: &str,
    state_store: Option<&dyn RuntimeStateStore>,
    default_variables: &[RuntimeDefaultVariable],
    secrets: &[RuntimeSecretDeclaration],
) -> Result<InitialRuntimeState, RuntimeError> {
    let mut variables = BTreeMap::new();
    let mut declarations = BTreeMap::<String, RuntimeVariableScope>::new();
    let mut declared_variables = BTreeMap::<String, (String, String)>::new();

    for node in graph
        .nodes()
        .filter(|node| node.action_type == "runtime.set_variable")
    {
        let name = required_config_string(node, "name")?;
        validate_variable_name(node, &name)?;
        let scope_name = required_config_string(node, "scope")?;
        let value_type = required_config_string(node, "valueType")?;
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
        if let Some((existing_scope, existing_type)) =
            declared_variables.insert(name.clone(), (scope_name.clone(), value_type.clone()))
            && (existing_scope != scope_name || existing_type != value_type)
        {
            return Err(RuntimeError::InvalidGraph(format!(
                "variable {name:?} is declared with conflicting scope or type"
            )));
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
    for variable in default_variables
        .iter()
        .filter(|variable| variable.scope == RuntimeDefaultVariableScope::Runtime)
    {
        variables.insert(variable.name.clone(), variable.value.clone());
        refresh_derived_variable_metadata(&mut variables, &variable.name);
    }
    if let Some(store) = state_store {
        for variable in default_variables
            .iter()
            .filter(|variable| variable.scope == RuntimeDefaultVariableScope::Persistent)
        {
            let value = load_or_initialize_persistent_default(store, script_id, variable)?;
            variables.insert(variable.name.clone(), value);
            refresh_derived_variable_metadata(&mut variables, &variable.name);
        }
        for (name, scope) in declarations {
            if let Some(stored) = store
                .load_variable(scope, script_id, &name)
                .map_err(RuntimeError::State)?
            {
                variables.insert(name.clone(), stored.value);
                refresh_derived_variable_metadata(&mut variables, &name);
            }
        }
        for secret in secrets {
            match store
                .read_secret(script_id, &secret.name)
                .map_err(RuntimeError::State)?
            {
                Some(value) => {
                    validate_secret_value(secret, &value)?;
                    variables.insert(secret.name.clone(), value.clone());
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
        variables,
    })
}

fn validate_secret_value(
    declaration: &RuntimeSecretDeclaration,
    value: &Value,
) -> Result<(), RuntimeError> {
    let valid = match declaration.value_type.as_str() {
        "string" | "file_path" => value.is_string(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "list" => value.is_array(),
        "object" | "http_response" | "datetime" | "duration" => value.is_object(),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(RuntimeError::State(format!(
            "secret {:?} does not match declared type {}",
            declaration.name, declaration.value_type
        )))
    }
}
