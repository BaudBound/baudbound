use std::{collections::BTreeMap, time::Duration};

use serde_json::{Map, Number, Value};

use crate::{RuntimeError, RuntimeNode};
pub(crate) fn validate_variable_name(node: &RuntimeNode, name: &str) -> Result<(), RuntimeError> {
    if name.starts_with("system_")
        || name.starts_with("manifest_")
        || name.ends_with(".$length")
        || name.ends_with(".$count")
    {
        return Err(RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!("{name} is read-only or reserved"),
        });
    }
    Ok(())
}

pub(crate) fn coerce_variable_value(
    node: &RuntimeNode,
    value: Value,
    value_type: &str,
) -> Result<Value, RuntimeError> {
    match value_type {
        "string" | "file_content" | "file_path" | "datetime" | "duration" => {
            Ok(Value::String(value_to_string(&value)))
        }
        "number" | "http_status_code" | "duration_ms" | "process_id" | "exit_code" => {
            number_from_value(Some(&value))
                .and_then(Number::from_f64)
                .map(Value::Number)
                .ok_or_else(|| RuntimeError::VariableOperation {
                    node_id: node.id.clone(),
                    message: format!("expected number, found {}", value_kind(&value)),
                })
        }
        "boolean" => match value {
            Value::Bool(value) => Ok(Value::Bool(value)),
            Value::String(value) if value.eq_ignore_ascii_case("true") => Ok(Value::Bool(true)),
            Value::String(value) if value.eq_ignore_ascii_case("false") => Ok(Value::Bool(false)),
            other => Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("expected boolean, found {}", value_kind(&other)),
            }),
        },
        "object" | "http_response" | "http_headers" => match value {
            Value::Object(_) => Ok(value),
            other => Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("expected object, found {}", value_kind(&other)),
            }),
        },
        "list" => match value {
            Value::Array(_) => Ok(value),
            other => Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("expected list, found {}", value_kind(&other)),
            }),
        },
        "keyboard_key" => Ok(Value::String(value_to_string(&value))),
        _ => Err(RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!("unsupported variable type {value_type}"),
        }),
    }
}

pub(crate) fn set_object_field(
    node: &RuntimeNode,
    target: &mut Value,
    field_path: &str,
    value: Value,
) -> Result<(), RuntimeError> {
    let segments = field_path
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return Err(RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: "fieldPath must contain at least one segment".to_owned(),
        });
    }

    let mut current = target;
    for segment in &segments[..segments.len() - 1] {
        if !current.is_object() {
            *current = Value::Object(Map::new());
        }
        current = current
            .as_object_mut()
            .expect("current value was just converted to object")
            .entry((*segment).to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
    }

    if !current.is_object() {
        *current = Value::Object(Map::new());
    }
    current
        .as_object_mut()
        .expect("current value was just converted to object")
        .insert(
            segments
                .last()
                .expect("segments is known to be non-empty")
                .to_string(),
            value,
        );
    Ok(())
}

pub(crate) fn refresh_derived_variable_metadata(
    variables: &mut BTreeMap<String, Value>,
    name: &str,
) {
    let length_key = format!("{name}.$length");
    let count_key = format!("{name}.$count");
    variables.remove(&length_key);
    variables.remove(&count_key);

    let derived = variables.get(name).and_then(|value| match value {
        Value::String(value) => Some((Some(value.len()), None)),
        Value::Array(values) => Some((Some(values.len()), Some(values.len()))),
        Value::Object(fields) => Some((Some(fields.len()), Some(fields.len()))),
        _ => None,
    });

    if let Some((length, count)) = derived {
        if let Some(length) = length {
            variables.insert(length_key, Value::Number(length.into()));
        }
        if let Some(count) = count {
            variables.insert(count_key, Value::Number(count.into()));
        }
    }
}

pub(crate) fn empty_value_for_type(value_type: &str) -> Value {
    match value_type {
        "number" | "http_status_code" | "duration_ms" | "process_id" | "exit_code" => {
            Value::Number(0.into())
        }
        "boolean" => Value::Bool(false),
        "object" | "http_response" | "http_headers" => Value::Object(Map::new()),
        "list" => Value::Array(Vec::new()),
        _ => Value::String(String::new()),
    }
}

pub(crate) fn number_from_value(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(value)) => value.parse::<f64>().ok(),
        _ => None,
    }
}

pub(crate) fn number_value(node: &RuntimeNode, value: f64) -> Result<Value, RuntimeError> {
    Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(|| RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!("{value} cannot be represented as a JSON number"),
        })
}

pub(crate) fn duration_from_amount(amount: f64, unit: &str) -> Duration {
    let milliseconds = match unit {
        "millisecond" | "milliseconds" | "ms" => amount,
        "minute" | "minutes" => amount * 60_000.0,
        "hour" | "hours" => amount * 3_600_000.0,
        _ => amount * 1_000.0,
    };

    Duration::from_millis(milliseconds.max(0.0).round() as u64)
}

pub(crate) fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

pub(crate) fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "list",
        Value::Object(_) => "object",
    }
}
