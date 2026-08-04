use crate::RuntimeVariableScope;
use crate::runtime::{
    coerce_variable_value, config_string, empty_value_for_type, number_from_value, number_value,
    remove_object_field, required_config_string, resolve_config_value, resolve_template_value,
    set_object_field, validate_variable_name, value_kind,
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
        if name == "settings" {
            return Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: "Script Settings are read-only".to_owned(),
            });
        }
        if self.secret_names.iter().any(|secret| secret == &name) {
            return Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("secret {name:?} is read-only"),
            });
        }

        let operation =
            config_string(&node.config, "operation").unwrap_or_else(|| "set".to_owned());
        let value_type = variable_operation_declared_type(node, &operation)?;
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
        if operation == "delete" {
            let existed = if let Some(scope) = scope {
                let store = self.state_store.ok_or_else(|| {
                    RuntimeError::State(
                        "stored variable operation requires a runner state store".to_owned(),
                    )
                })?;
                let deleted = store
                    .delete_variable(scope, &self.context.identity.script_id, &name)
                    .map_err(RuntimeError::State)?;
                self.remove_variable(&name);
                deleted
            } else {
                let existed = self.context.variables.contains_key(&name);
                self.remove_variable(&name);
                existed
            };
            self.push_runtime_log(
                "info",
                if existed {
                    format!("Deleted {scope_label} variable {name:?}.")
                } else {
                    format!(
                        "{} variable {name:?} was already missing.",
                        capitalize(scope_label)
                    )
                },
                Some(node.id.clone()),
            );
            return Ok(());
        }
        let next = if let Some(scope) = scope {
            self.execute_stored_variable_operation(
                node,
                scope,
                &name,
                &operation,
                value_type.as_deref(),
            )?
        } else {
            let current = self.context.variables.get(&name).cloned();
            let value = self.calculate_variable_operation_value(
                node,
                &name,
                &operation,
                value_type.as_deref(),
                current,
            )?;
            if self.value_contains_sensitive(&value) {
                let transient = self.value_contains_transient_sensitive(&value);
                self.set_sensitive_variable(
                    name.clone(),
                    value.clone(),
                    RunVariableScope::Runtime,
                    transient,
                )?;
            } else {
                self.set_variable(name.clone(), value.clone(), RunVariableScope::Runtime)?;
            }
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
        value_type: Option<&str>,
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
            if self.value_contains_sensitive(&next) {
                return Err(RuntimeError::VariableOperation {
                    node_id: node.id.clone(),
                    message: "sensitive form dialog output is transient and cannot be stored in persistent or global variables".to_owned(),
                });
            }
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
        value_type: Option<&str>,
        current: Option<Value>,
    ) -> Result<Value, RuntimeError> {
        match operation {
            "set" => {
                let value_type = value_type.ok_or_else(|| RuntimeError::VariableOperation {
                    node_id: node.id.clone(),
                    message: "set requires valueType".to_owned(),
                })?;
                let item_type = config_string(&node.config, "itemType");
                let raw_value = if matches!(value_type, "list" | "object") {
                    self.resolve_json_compatible_input(node.config.get("value"))?
                } else {
                    self.resolve_variable_input(node.config.get("value"))
                };
                let value = coerce_variable_value(node, raw_value, value_type)?;
                validate_declared_value(node, value_type, item_type.as_deref(), &value)?;
                Ok(value)
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
            "toggle_boolean" => {
                let current = match current {
                    Some(Value::Bool(value)) => value,
                    Some(other) => {
                        return Err(RuntimeError::VariableOperation {
                            node_id: node.id.clone(),
                            message: format!(
                                "toggle_boolean requires existing variable {name} to be a boolean, found {}",
                                value_kind(&other)
                            ),
                        });
                    }
                    None => false,
                };
                Ok(Value::Bool(!current))
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
                let item = self.resolve_json_compatible_input(node.config.get("value"))?;
                validate_list_append(node, name, &list, &item)?;
                list.push(item);
                Ok(Value::Array(list))
            }
            "remove_list_items" => {
                let mut list = match current {
                    Some(Value::Array(values)) => values,
                    Some(other) => {
                        return Err(RuntimeError::VariableOperation {
                            node_id: node.id.clone(),
                            message: format!(
                                "remove_list_items requires existing variable {name} to be a list, found {}",
                                value_kind(&other)
                            ),
                        });
                    }
                    None => {
                        return Err(RuntimeError::VariableOperation {
                            node_id: node.id.clone(),
                            message: format!(
                                "remove_list_items requires existing list variable {name}"
                            ),
                        });
                    }
                };
                let item = self.resolve_json_compatible_input(node.config.get("value"))?;
                let remove_mode = required_config_string(node, "removeMode")?;
                if !matches!(remove_mode.as_str(), "first" | "all") {
                    return Err(RuntimeError::VariableOperation {
                        node_id: node.id.clone(),
                        message: format!("unsupported list removal mode {remove_mode:?}"),
                    });
                }
                let mut removed = false;
                list.retain(|entry| {
                    if entry != &item || (remove_mode == "first" && removed) {
                        return true;
                    }
                    removed = true;
                    false
                });
                Ok(Value::Array(list))
            }
            "set_object_field" => {
                let field_path = required_config_string(node, "fieldPath")?;
                let value = self.resolve_json_compatible_input(node.config.get("value"))?;
                let field_value_type = required_config_string(node, "fieldValueType")?;
                let field_item_type = config_string(&node.config, "fieldItemType");
                validate_declared_value(
                    node,
                    &field_value_type,
                    field_item_type.as_deref(),
                    &value,
                )?;
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
            "remove_object_field" => {
                let field_path = required_config_string(node, "fieldPath")?;
                let mut current = match current {
                    Some(Value::Object(object)) => Value::Object(object),
                    Some(other) => {
                        return Err(RuntimeError::VariableOperation {
                            node_id: node.id.clone(),
                            message: format!(
                                "remove_object_field requires existing variable {name} to be an object, found {}",
                                value_kind(&other)
                            ),
                        });
                    }
                    None => {
                        return Err(RuntimeError::VariableOperation {
                            node_id: node.id.clone(),
                            message: format!(
                                "remove_object_field requires existing object variable {name}"
                            ),
                        });
                    }
                };
                remove_object_field(node, &mut current, &field_path)?;
                Ok(current)
            }
            "merge_object" => {
                let incoming = self.resolve_json_compatible_input(node.config.get("value"))?;
                validate_declared_value(node, "object", None, &incoming)?;
                let mut object = match current {
                    Some(Value::Object(object)) => object,
                    Some(other) => {
                        return Err(RuntimeError::VariableOperation {
                            node_id: node.id.clone(),
                            message: format!(
                                "merge_object requires existing variable {name} to be an object, found {}",
                                value_kind(&other)
                            ),
                        });
                    }
                    None => Map::new(),
                };
                object.extend(
                    incoming
                        .as_object()
                        .expect("validated object value should remain an object")
                        .clone(),
                );
                Ok(Value::Object(object))
            }
            "clear" => clear_existing_variable(node, name, current.as_ref()),
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
            "toggle_boolean" => {
                format!("Toggled {scope} boolean variable {name:?}. New value: {next}.")
            }
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
            "remove_list_items" => {
                let item = self
                    .resolve_json_compatible_input(node.config.get("value"))
                    .unwrap_or(Value::Null);
                let mode =
                    config_string(&node.config, "removeMode").unwrap_or_else(|| "all".to_owned());
                format!(
                    "Removed {mode} matching occurrences of {} from {scope} list variable {name:?}. New value: {next}.",
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
            "remove_object_field" => {
                let path = config_string(&node.config, "fieldPath").unwrap_or_default();
                format!(
                    "Removed field {path:?} from {scope} object variable {name:?} when present. New value: {next}."
                )
            }
            "merge_object" => {
                let value = self
                    .resolve_json_compatible_input(node.config.get("value"))
                    .unwrap_or(Value::Null);
                format!(
                    "Shallow merged {} into {scope} object variable {name:?}. New value: {next}.",
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

fn variable_operation_declared_type(
    node: &RuntimeNode,
    operation: &str,
) -> Result<Option<String>, RuntimeError> {
    let value_type = match operation {
        "set" => Some(required_config_string(node, "valueType")?),
        "increment" => Some("number".to_owned()),
        "toggle_boolean" => Some("boolean".to_owned()),
        "append_list" | "remove_list_items" => Some("list".to_owned()),
        "set_object_field" | "remove_object_field" | "merge_object" => Some("object".to_owned()),
        "clear" | "delete" => None,
        _ => {
            return Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("unsupported variable operation {operation}"),
            });
        }
    };
    Ok(value_type)
}

fn clear_existing_variable(
    node: &RuntimeNode,
    name: &str,
    current: Option<&Value>,
) -> Result<Value, RuntimeError> {
    let current = current.ok_or_else(|| RuntimeError::VariableOperation {
        node_id: node.id.clone(),
        message: format!("clear requires existing variable {name:?}"),
    })?;
    let value_type = match current {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Array(_) => "list",
        Value::Object(object) => match object.get("type").and_then(Value::as_str) {
            Some("datetime") => "datetime",
            Some("duration") => "duration",
            _ => "object",
        },
        Value::Null => {
            return Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("clear cannot derive a supported type from variable {name:?}"),
            });
        }
    };
    Ok(empty_value_for_type(value_type))
}

fn validate_list_append(
    node: &RuntimeNode,
    name: &str,
    list: &[Value],
    item: &Value,
) -> Result<(), RuntimeError> {
    let item_kind = list_item_kind(item).ok_or_else(|| RuntimeError::VariableOperation {
        node_id: node.id.clone(),
        message: format!(
            "append_list requires a supported non-null, non-list item for variable {name:?}"
        ),
    })?;
    let mut existing_kind = None;
    for existing in list {
        let kind = list_item_kind(existing).ok_or_else(|| RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!(
                "existing list variable {name:?} contains an unsupported nested list or null item"
            ),
        })?;
        if existing_kind.is_some_and(|expected| expected != kind) {
            return Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("existing list variable {name:?} contains mixed item types"),
            });
        }
        existing_kind = Some(kind);
    }
    if let Some(expected) = existing_kind
        && expected != item_kind
    {
        return Err(RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!(
                "append_list requires {expected} items for variable {name:?}, found {item_kind}"
            ),
        });
    }
    Ok(())
}

fn list_item_kind(value: &Value) -> Option<&'static str> {
    match value {
        Value::String(_) => Some("string"),
        Value::Number(_) => Some("number"),
        Value::Bool(_) => Some("boolean"),
        Value::Object(object) => match object.get("type").and_then(Value::as_str) {
            Some("datetime") => Some("datetime"),
            Some("duration") => Some("duration"),
            _ => Some("object"),
        },
        Value::Null | Value::Array(_) => None,
    }
}

fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    characters
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
        .unwrap_or_default()
}

fn diagnostic_value(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

fn validate_declared_value(
    node: &RuntimeNode,
    value_type: &str,
    item_type: Option<&str>,
    value: &Value,
) -> Result<(), RuntimeError> {
    let valid = match value_type {
        "string" => value.is_string(),
        "file_path" => value.as_str().is_some_and(|path| !path.trim().is_empty()),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "list" => value.as_array().is_some_and(|items| {
            item_type.is_some_and(|item_type| {
                items
                    .iter()
                    .all(|item| validate_declared_value(node, item_type, None, item).is_ok())
            })
        }),
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
    };
    if valid {
        Ok(())
    } else {
        Err(RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!(
                "value of kind {} does not match declared type {value_type:?}",
                value_kind(value)
            ),
        })
    }
}
