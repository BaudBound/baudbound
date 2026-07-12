use crate::RuntimeVariableScope;
use crate::runtime::{
    coerce_variable_value, config_string, empty_value_for_type, number_from_value, number_value,
    refresh_derived_variable_metadata, required_config_string, resolve_template_value,
    set_object_field, validate_variable_name, value_kind,
};
use serde_json::{Map, Value};

use super::{RuntimeError, RuntimeExecutor, RuntimeNode};

impl RuntimeExecutor<'_> {
    pub(super) fn execute_variable_operation(
        &mut self,
        node: &RuntimeNode,
    ) -> Result<(), RuntimeError> {
        let name = required_config_string(node, "name")?;
        validate_variable_name(node, &name)?;
        if self.secret_names.iter().any(|secret| secret == &name) {
            return Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("secret {name:?} is read-only"),
            });
        }

        let operation =
            config_string(&node.config, "operation").unwrap_or_else(|| "set".to_owned());
        let value_type =
            config_string(&node.config, "valueType").unwrap_or_else(|| "string".to_owned());
        let scope = match required_config_string(node, "scope")?.as_str() {
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

        if let Some(scope) = scope {
            self.execute_stored_variable_operation(node, scope, &name, &operation, &value_type)?;
        } else {
            let current = self.context.variables.get(&name).cloned();
            let value = self.calculate_variable_operation_value(
                node,
                &name,
                &operation,
                &value_type,
                current,
            )?;
            self.set_variable(name.clone(), value);
        }

        self.push_runtime_log(
            "debug",
            format!("Variable operation {operation} completed."),
            Some(node.id.clone()),
        );
        Ok(())
    }

    fn execute_stored_variable_operation(
        &mut self,
        node: &RuntimeNode,
        scope: RuntimeVariableScope,
        name: &str,
        operation: &str,
        value_type: &str,
    ) -> Result<(), RuntimeError> {
        const MAX_COMPARE_AND_SET_ATTEMPTS: usize = 32;
        let store = self.state_store.ok_or_else(|| {
            RuntimeError::State(
                "stored variable operation requires a runner state store".to_owned(),
            )
        })?;

        for _ in 0..MAX_COMPARE_AND_SET_ATTEMPTS {
            self.ensure_not_cancelled()?;
            let stored = store
                .load_variable(scope, &self.context.identity.script_id, name)
                .map_err(RuntimeError::State)?;
            let expected_version = stored.as_ref().map(|variable| variable.version);
            let current = stored.map(|variable| variable.value);
            match &current {
                Some(value) => self.set_variable(name.to_owned(), value.clone()),
                None => {
                    self.context.variables.remove(name);
                    refresh_derived_variable_metadata(&mut self.context.variables, name);
                }
            }
            let next = self
                .calculate_variable_operation_value(node, name, operation, value_type, current)?;
            if store
                .compare_and_set_variable(
                    scope,
                    &self.context.identity.script_id,
                    name,
                    expected_version,
                    &next,
                )
                .map_err(RuntimeError::State)?
            {
                self.set_variable(name.to_owned(), next);
                return Ok(());
            }
        }

        Err(RuntimeError::State(format!(
            "variable {name:?} changed too frequently to update safely"
        )))
    }

    fn calculate_variable_operation_value(
        &self,
        node: &RuntimeNode,
        name: &str,
        operation: &str,
        value_type: &str,
        current: Option<Value>,
    ) -> Result<Value, RuntimeError> {
        match operation {
            "set" => {
                let raw_value = self.resolve_variable_input(node.config.get("value"));
                coerce_variable_value(node, raw_value, value_type)
            }
            "increment" => {
                let increment_value = self.resolve_variable_input(node.config.get("value"));
                let increment = number_from_value(Some(&increment_value)).ok_or_else(|| {
                    RuntimeError::VariableOperation {
                        node_id: node.id.clone(),
                        message: "increment value must resolve to a finite number".to_owned(),
                    }
                })?;
                let current = number_from_value(current.as_ref()).unwrap_or(0.0);
                number_value(node, current + increment)
            }
            "append_list" => {
                let mut list = match current {
                    Some(Value::Array(values)) => values,
                    Some(other) => {
                        return Err(RuntimeError::VariableOperation {
                            node_id: node.id.clone(),
                            message: format!(
                                "append_list requires existing variable {name} to be a list, found {}",
                                value_kind(&other)
                            ),
                        });
                    }
                    None => Vec::new(),
                };
                list.push(self.resolve_json_compatible_input(node.config.get("value"))?);
                Ok(Value::Array(list))
            }
            "set_object_field" => {
                let field_path = required_config_string(node, "fieldPath")?;
                let value = self.resolve_json_compatible_input(node.config.get("value"))?;
                let mut current = current.unwrap_or_else(|| Value::Object(Map::new()));
                set_object_field(node, &mut current, &field_path, value)?;
                Ok(current)
            }
            "clear" => Ok(empty_value_for_type(value_type)),
            _ => Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("unsupported variable operation {operation}"),
            }),
        }
    }

    fn resolve_variable_input(&self, value: Option<&Value>) -> Value {
        match value.cloned().unwrap_or(Value::Null) {
            Value::String(template) => resolve_template_value(&template, &self.context.variables),
            value => value,
        }
    }

    fn resolve_json_compatible_input(&self, value: Option<&Value>) -> Result<Value, RuntimeError> {
        let resolved = self.resolve_variable_input(value);
        match resolved {
            Value::String(text) => match serde_json::from_str(text.trim()) {
                Ok(value) => Ok(value),
                Err(_) => Ok(Value::String(text)),
            },
            value => Ok(value),
        }
    }
}
