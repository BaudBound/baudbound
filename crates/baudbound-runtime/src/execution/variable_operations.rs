use super::cast_validation::validate_value_casts;
use crate::runtime::{
    coerce_variable_value, config_string, empty_value_for_declared_type, empty_value_for_type,
    integer_from_value, number_from_value, number_value, remove_object_field,
    required_config_string, resolve_config_value, resolve_template_value, set_object_field,
    validate_variable_name, value_kind,
};
use crate::value_type::MAX_SAFE_INTEGER;
use crate::{RuntimeVariableScope, ValueType, validate_value};
use serde_json::{Map, Value};

use super::{RunVariableScope, RuntimeError, RuntimeExecutor, RuntimeNode};

impl RuntimeExecutor<'_> {
    pub(super) fn execute_variable_operation(
        &mut self,
        node: &RuntimeNode,
    ) -> Result<(), RuntimeError> {
        let name = required_config_string(node, "name")?;
        // A built-in lives behind "@", which is not a legal identifier
        // character, so validate_variable_name refuses every one of them and
        // no separate guard per built-in is needed.
        validate_variable_name(node, &name)?;
        if self.secret_names.iter().any(|secret| secret == &name) {
            return Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("secret {name:?} is read-only"),
            });
        }

        let operation =
            config_string(&node.config, "operation").unwrap_or_else(|| "set".to_owned());
        let declared_type = self.declared_types.get(&name).map(String::as_str);
        let value_type = variable_operation_declared_type(node, &operation, declared_type)?;
        // From the declaration, not the node. A variable has one scope, and a
        // node repeating it was a second place for it to disagree.
        let scope = match self.declared_scopes.get(&name).map(String::as_str) {
            Some("persistent") => Some(RuntimeVariableScope::Persistent),
            Some("global") => Some(RuntimeVariableScope::Global),
            _ => None,
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
                    self.resolve_json_compatible_input(&node.id, node.config.get("value"))?
                } else {
                    self.resolve_variable_input(node.config.get("value"))
                };
                // Coercion is lenient (a numeric string is accepted for an
                // integer field, an integer widens into a float field), so a
                // coercion failure does not necessarily mean the value is the
                // wrong type. Re-check the pre-coercion value against the
                // shared vocabulary before giving up, so a genuine type
                // mismatch is reported as a type error naming the variable
                // rather than the coercion function's generic message.
                let value = match coerce_variable_value(node, raw_value.clone(), value_type) {
                    Ok(value) => value,
                    Err(error) => {
                        reject_wrong_type(node, name, value_type, &raw_value)?;
                        return Err(error);
                    }
                };
                reject_wrong_type(node, name, value_type, &value)?;
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
                if let Some(current) = current.as_ref()
                    && number_from_value(Some(current)).is_none()
                {
                    return Err(RuntimeError::VariableOperation {
                        node_id: node.id.clone(),
                        message: format!(
                            "increment requires existing variable {name} to be a finite number"
                        ),
                    });
                }

                // An integer stays an integer. Routing every increment through
                // f64 would turn a counter into a float on its first step, and
                // a float renders with a decimal, so `3` would become `3.0`.
                if let (Some(current_whole), Some(increment_whole)) = (
                    integer_operand(current.as_ref()),
                    integer_operand(Some(&increment_value)),
                ) {
                    // Bound the sum by the safe integer range rather than by
                    // i64, so increment cannot produce a value that the
                    // integer type itself would reject.
                    return current_whole
                        .checked_add(increment_whole)
                        .filter(|sum| (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(sum))
                        .map(|sum| Value::Number(sum.into()))
                        .ok_or_else(|| RuntimeError::VariableOperation {
                            node_id: node.id.clone(),
                            message: format!(
                                "increment moved variable {name} outside the safe integer range"
                            ),
                        });
                }

                let current = current
                    .as_ref()
                    .and_then(|current| number_from_value(Some(current)))
                    .unwrap_or(0.0);
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
                let item =
                    self.resolve_json_compatible_input(&node.id, node.config.get("value"))?;
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
                let item =
                    self.resolve_json_compatible_input(&node.id, node.config.get("value"))?;
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
                let value =
                    self.resolve_json_compatible_input(&node.id, node.config.get("value"))?;
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
                let incoming =
                    self.resolve_json_compatible_input(&node.id, node.config.get("value"))?;
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
            "clear" => clear_existing_variable(
                node,
                name,
                current.as_ref(),
                self.declared_types.get(name).map(String::as_str),
            ),
            _ => Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("unsupported variable operation {operation}"),
            }),
        }
    }

    fn resolve_variable_input(&self, value: Option<&Value>) -> Value {
        resolve_config_value(value.unwrap_or(&Value::Null), &self.context.variables)
    }

    fn resolve_json_compatible_input(
        &self,
        node_id: &str,
        value: Option<&Value>,
    ) -> Result<Value, RuntimeError> {
        let raw = value.cloned().unwrap_or(Value::Null);
        if let Value::String(text) = &raw
            && let Ok(json_value) = serde_json::from_str::<Value>(text.trim())
        {
            // Parsing turns an escaped brace into a real template, which the
            // pre-pass over the raw string could not see, so the parsed value
            // has to be proven before it is resolved.
            validate_value_casts(node_id, &json_value, &self.context.variables)?;
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
                    .resolve_json_compatible_input(&node.id, node.config.get("value"))
                    .unwrap_or(Value::Null);
                format!(
                    "Appended {} to {scope} list variable {name:?}. New value: {next}.",
                    diagnostic_value(&item)
                )
            }
            "remove_list_items" => {
                let item = self
                    .resolve_json_compatible_input(&node.id, node.config.get("value"))
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
                    .resolve_json_compatible_input(&node.id, node.config.get("value"))
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
                    .resolve_json_compatible_input(&node.id, node.config.get("value"))
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

/// The type an operation writes at.
///
/// `set` takes it from the declaration, which is the only place a variable's
/// type lives now. Every other operation implies one, and `increment` pins
/// none: it applies to whichever numeric type the variable was declared with
/// and preserves it.
fn variable_operation_declared_type(
    node: &RuntimeNode,
    operation: &str,
    declared_type: Option<&str>,
) -> Result<Option<String>, RuntimeError> {
    let value_type = match operation {
        "set" => Some(
            declared_type
                .ok_or_else(|| RuntimeError::VariableOperation {
                    node_id: node.id.clone(),
                    message: "the manifest does not declare a type for this variable".to_owned(),
                })?
                .to_owned(),
        ),
        // Increment does not pin a numeric type: it applies to whichever numeric
        // type the variable was declared with, and preserves it.
        "increment" => None,
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

/// Names which string-shaped type a variable was declared with, if it matters.
///
/// `string`, `color` and `hotkey` are all JSON strings, so a stored value
/// cannot say which of them it is. Only these three need asking about, because
/// every other type is identifiable from the value itself. This used to read
/// the node; the declaration is the only place a type lives now.
fn declared_string_type(declared_type: Option<&str>) -> Option<&'static str> {
    match declared_type? {
        "color" => Some("color"),
        "hotkey" => Some("hotkey"),
        _ => None,
    }
}

fn clear_existing_variable(
    node: &RuntimeNode,
    name: &str,
    current: Option<&Value>,
    declared_type: Option<&str>,
) -> Result<Value, RuntimeError> {
    let current = current.ok_or_else(|| RuntimeError::VariableOperation {
        node_id: node.id.clone(),
        message: format!("clear requires existing variable {name:?}"),
    })?;
    let value_type = match current {
        // The stored value's shape identifies every type except which of the
        // string-shaped ones it is, so only that case consults the declared
        // type. Trusting the declaration for the others would clear an integer
        // to an empty string whenever a node carried a stale default.
        Value::String(_) => match declared_string_type(declared_type) {
            Some(declared) => {
                return empty_value_for_declared_type(declared).ok_or_else(|| {
                    RuntimeError::VariableOperation {
                        node_id: node.id.clone(),
                        message: format!(
                            "clear has no empty value for the {declared} variable {name:?}, use delete instead"
                        ),
                    }
                });
            }
            None => "string",
        },
        Value::Number(number) if number.as_i64().is_some() || number.as_u64().is_some() => {
            "integer"
        }
        Value::Number(_) => "float",
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

/// Reads a value as a whole-number operand for `increment`.
///
/// Returns `None` when the value is not an integer, which sends the increment
/// down the float path. Only serde_json's integer variants qualify: a float
/// stays a float even when its value is whole, and a numeric string such as
/// `"42"` is not a number at all. An absent variable counts as integer zero so
/// that a fresh counter starts as an integer.
fn integer_operand(value: Option<&Value>) -> Option<i64> {
    match value {
        // An absent variable counts as integer zero, so a fresh counter starts
        // as an integer.
        None => Some(0),
        // The editor stores a config value as text, so an amount typed as "1"
        // arrives as a string. It uses the same rule `set` uses, otherwise
        // typing a literal into the editor would silently widen a counter to a
        // float while a variable reference kept it an integer.
        Some(value) => integer_from_value(value),
    }
}

fn list_item_kind(value: &Value) -> Option<&'static str> {
    match value {
        Value::String(_) => Some("string"),
        Value::Number(number) if number.as_i64().is_some() || number.as_u64().is_some() => {
            Some("integer")
        }
        Value::Number(_) => Some("float"),
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

/// Rejects a `set` value that violates the type declared for the variable,
/// stopping the run rather than routing to the node's failed output: a value
/// that never satisfied its declared type was never a valid program to begin
/// with. Declared type names outside the shared `ValueType` vocabulary (the
/// retired names such as `number` or `file_path`) are left to
/// `validate_declared_value`, which still handles them.
fn reject_wrong_type(
    node: &RuntimeNode,
    name: &str,
    declared_type: &str,
    value: &Value,
) -> Result<(), RuntimeError> {
    if let Ok(declared) = declared_type.parse::<ValueType>()
        && let Err(reason) = validate_value(value, declared)
    {
        return Err(RuntimeError::Type {
            node_id: node.id.clone(),
            message: format!("variable \"{name}\" {reason}"),
        });
    }
    Ok(())
}

fn validate_declared_value(
    node: &RuntimeNode,
    value_type: &str,
    item_type: Option<&str>,
    value: &Value,
) -> Result<(), RuntimeError> {
    let valid = match value_type {
        "string" => value.is_string(),
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
        // The types introduced by the type rework are checked by the shared
        // validator so that a declared variable and a validated boundary can
        // never disagree about what the type means.
        "integer" | "float" | "color" | "hotkey" => value_type
            .parse::<crate::ValueType>()
            .is_ok_and(|declared| crate::validate_value(value, declared).is_ok()),
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
