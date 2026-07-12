use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::runtime::{
    RuntimeGraph, refresh_derived_variable_metadata, required_config_string, validate_variable_name,
};
use crate::{RuntimeSecretDeclaration, RuntimeStateStore, RuntimeVariableScope};

use super::RuntimeError;

pub(super) struct InitialRuntimeState {
    pub(super) secret_names: Vec<String>,
    pub(super) secret_values: Vec<Value>,
    pub(super) variables: BTreeMap<String, Value>,
}

pub(super) fn load_initial_state(
    graph: &RuntimeGraph,
    script_id: &str,
    state_store: Option<&dyn RuntimeStateStore>,
    secrets: &[RuntimeSecretDeclaration],
) -> Result<InitialRuntimeState, RuntimeError> {
    let mut variables = BTreeMap::new();
    let mut declarations = BTreeMap::<String, RuntimeVariableScope>::new();
    let mut declared_scopes = BTreeMap::<String, String>::new();

    for node in graph
        .nodes()
        .filter(|node| node.action_type == "runtime.set_variable")
    {
        let name = required_config_string(node, "name")?;
        validate_variable_name(node, &name)?;
        let scope_name = required_config_string(node, "scope")?;
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
        if let Some(existing) = declared_scopes.insert(name.clone(), scope_name.clone())
            && existing != scope_name
        {
            return Err(RuntimeError::InvalidGraph(format!(
                "variable {name:?} is declared with conflicting scopes {existing:?} and {scope_name:?}"
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
        .find(|name| declared_scopes.contains_key(name.as_str()))
    {
        return Err(RuntimeError::InvalidGraph(format!(
            "secret {collision:?} conflicts with a writable variable"
        )));
    }

    if (!declarations.is_empty() || !secrets.is_empty()) && state_store.is_none() {
        return Err(RuntimeError::State(
            "persistent, global, and secret variables require a runner state store".to_owned(),
        ));
    }
    let mut secret_values = Vec::new();
    if let Some(store) = state_store {
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
