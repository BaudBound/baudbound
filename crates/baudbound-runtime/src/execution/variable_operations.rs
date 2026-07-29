use crate::RuntimeVariableScope;
use crate::runtime::{
    coerce_variable_value, config_string, empty_value_for_type, number_from_value, number_value,
    required_config_string, resolve_config_value, resolve_template_value, set_object_field,
    validate_variable_name, value_kind,
};
use serde_json::{Map, Value};

use super::{RunVariableScope, RuntimeError, RuntimeExecutor, RuntimeNode};

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

        let scope_label = match scope {
            Some(RuntimeVariableScope::Persistent) => "persistent",
            Some(RuntimeVariableScope::Global) => "global",
            None => "runtime",
        };
        let next = if let Some(scope) = scope {
            self.execute_stored_variable_operation(node, scope, &name, &operation, &value_type)?
        } else {
            let current = self.context.variables.get(&name).cloned();
            let value = self.calculate_variable_operation_value(
                node,
                &name,
                &operation,
                &value_type,
                current,
            )?;
            self.set_variable(name.clone(), value.clone(), RunVariableScope::Runtime)?;
            value
        };

        self.push_runtime_log(
            "info",
            self.variable_operation_message(node, &name, &operation, scope_label, &next),
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
    ) -> Result<Value, RuntimeError> {
        const MAX_COMPARE_AND_SET_ATTEMPTS: usize = 32;
        let store = self.state_store.ok_or_else(|| {
            RuntimeError::State(
                "stored variable operation requires a runner state store".to_owned(),
            )
        })?;
        let run_scope = match scope {
            RuntimeVariableScope::Persistent => RunVariableScope::Persistent,
            RuntimeVariableScope::Global => RunVariableScope::Global,
        };

        for _ in 0..MAX_COMPARE_AND_SET_ATTEMPTS {
            self.ensure_not_cancelled()?;
            let stored = store
                .load_variable(scope, &self.context.identity.script_id, name)
                .map_err(RuntimeError::State)?;
            let expected_version = stored.as_ref().map(|variable| variable.version);
            let current = stored.map(|variable| variable.value);
            match &current {
                Some(value) => {
                    self.set_variable(name.to_owned(), value.clone(), run_scope)?;
                }
                None => self.remove_variable(name),
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
                self.set_variable(name.to_owned(), next.clone(), run_scope)?;
                return Ok(next);
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
                let raw_value = if matches!(value_type, "list" | "object") {
                    self.resolve_json_compatible_input(node.config.get("value"))?
                } else {
                    self.resolve_variable_input(node.config.get("value"))
                };
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
                let current = match current {
                    Some(current) => number_from_value(Some(&current)).ok_or_else(|| {
                        RuntimeError::VariableOperation {
                            node_id: node.id.clone(),
                            message: format!(
                                "increment requires existing variable {name} to be a finite number"
                            ),
                        }
                    })?,
                    None => 0.0,
                };
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
                let mut current = match current {
                    Some(Value::Object(object)) => Value::Object(object),
                    Some(other) => {
                        return Err(RuntimeError::VariableOperation {
                            node_id: node.id.clone(),
                            message: format!(
                                "set_object_field requires existing variable {name} to be an object, found {}",
                                value_kind(&other)
                            ),
                        });
                    }
                    None => Value::Object(Map::new()),
                };
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
        resolve_config_value(value.unwrap_or(&Value::Null), &self.context.variables)
    }

    fn resolve_json_compatible_input(&self, value: Option<&Value>) -> Result<Value, RuntimeError> {
        let raw = value.cloned().unwrap_or(Value::Null);
        if let Value::String(text) = &raw
            && let Ok(json_value) = serde_json::from_str::<Value>(text.trim())
        {
            return Ok(resolve_config_value(&json_value, &self.context.variables));
        }

        let resolved = match raw {
            Value::String(template) => resolve_template_value(&template, &self.context.variables),
            value => resolve_config_value(&value, &self.context.variables),
        };
        match resolved {
            Value::String(text) => match serde_json::from_str(text.trim()) {
                Ok(value) => Ok(value),
                Err(_) => Ok(Value::String(text)),
            },
            value => Ok(value),
        }
    }

    fn variable_operation_message(
        &self,
        node: &RuntimeNode,
        name: &str,
        operation: &str,
        scope: &str,
        next: &Value,
    ) -> String {
        let next = diagnostic_value(next);
        match operation {
            "increment" => {
                let amount = self.resolve_variable_input(node.config.get("value"));
                format!(
                    "Incremented {scope} variable {name:?} by {}. New value: {next}.",
                    diagnostic_value(&amount)
                )
            }
            "append_list" => {
                let item = self
                    .resolve_json_compatible_input(node.config.get("value"))
                    .unwrap_or(Value::Null);
                format!(
                    "Appended {} to {scope} list variable {name:?}. New value: {next}.",
                    diagnostic_value(&item)
                )
            }
            "set_object_field" => {
                let path = config_string(&node.config, "fieldPath").unwrap_or_default();
                let value = self
                    .resolve_json_compatible_input(node.config.get("value"))
                    .unwrap_or(Value::Null);
                format!(
                    "Set field {path:?} on {scope} object variable {name:?} to {}. New value: {next}.",
                    diagnostic_value(&value)
                )
            }
            "clear" => {
                format!("Cleared {scope} variable {name:?}. New value: {next}.")
            }
            _ => format!("Set {scope} variable {name:?} to {next}."),
        }
    }
}

fn diagnostic_value(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}
