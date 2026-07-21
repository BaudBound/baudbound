use std::collections::BTreeMap;

use serde_json::{Map, Number, Value};

use crate::{RuntimeError, RuntimeNode};

pub(crate) const DERIVED_VARIABLE_METADATA_SUFFIXES: [&str; 4] =
    [".$length", ".$count", ".$type", ".$is_empty"];

pub(crate) fn validate_variable_name(node: &RuntimeNode, name: &str) -> Result<(), RuntimeError> {
    if name.starts_with("system_")
        || name.starts_with("manifest_")
        || DERIVED_VARIABLE_METADATA_SUFFIXES
            .iter()
            .any(|suffix| name.ends_with(suffix))
    {
        return Err(RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!("{name} is read-only or reserved"),
        });
    }

    let mut bytes = name.bytes();
    let valid = bytes.next().is_some_and(is_identifier_start) && bytes.all(is_identifier_continue);
    if !valid {
        return Err(RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!(
                "invalid variable name {name:?}; names must start with a letter or underscore and contain only letters, numbers, or underscores"
            ),
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
        "string" | "file_content" | "file_path" => Ok(Value::String(value_to_string(&value))),
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
        "object" | "http_response" | "http_headers" | "datetime" | "duration" => {
            coerce_json_container(node, value, true)
        }
        "list" => coerce_json_container(node, value, false),
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
    let segments =
        parse_object_path(field_path).map_err(|message| RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message,
        })?;
    set_path_value(target, &segments, value);
    Ok(())
}

#[derive(Debug)]
enum ObjectPathSegment {
    Field(String),
    Index(usize),
}

fn parse_object_path(path: &str) -> Result<Vec<ObjectPathSegment>, String> {
    let path = path.trim();
    let bytes = path.as_bytes();
    let mut segments = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if !is_identifier_start(bytes[index]) {
            return Err(format!("invalid object field path {path:?}"));
        }
        let start = index;
        index += 1;
        while index < bytes.len() && is_identifier_continue(bytes[index]) {
            index += 1;
        }
        segments.push(ObjectPathSegment::Field(path[start..index].to_owned()));

        while index < bytes.len() && bytes[index] == b'[' {
            index += 1;
            let number_start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if number_start == index || index >= bytes.len() || bytes[index] != b']' {
                return Err(format!("invalid object field path {path:?}"));
            }
            if bytes[number_start] == b'0' && index - number_start > 1 {
                return Err(format!("invalid object field path {path:?}"));
            }
            let array_index = path[number_start..index]
                .parse::<usize>()
                .map_err(|_| format!("invalid object field path {path:?}"))?;
            segments.push(ObjectPathSegment::Index(array_index));
            index += 1;
        }

        if index == bytes.len() {
            break;
        }
        if bytes[index] != b'.' {
            return Err(format!("invalid object field path {path:?}"));
        }
        index += 1;
        if index == bytes.len() {
            return Err(format!("invalid object field path {path:?}"));
        }
    }

    if segments.is_empty() {
        Err("object field path is required".to_owned())
    } else {
        Ok(segments)
    }
}

fn set_path_value(target: &mut Value, segments: &[ObjectPathSegment], value: Value) {
    let Some((segment, remaining)) = segments.split_first() else {
        *target = value;
        return;
    };

    match segment {
        ObjectPathSegment::Field(field) => {
            if !target.is_object() {
                *target = Value::Object(Map::new());
            }
            let child = target
                .as_object_mut()
                .expect("target was converted to an object")
                .entry(field.clone())
                .or_insert(Value::Null);
            set_path_value(child, remaining, value);
        }
        ObjectPathSegment::Index(index) => {
            if !target.is_array() {
                *target = Value::Array(Vec::new());
            }
            let items = target
                .as_array_mut()
                .expect("target was converted to an array");
            if items.len() <= *index {
                items.resize(*index + 1, Value::Null);
            }
            set_path_value(&mut items[*index], remaining, value);
        }
    }
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

pub(crate) fn refresh_derived_variable_metadata(
    variables: &mut BTreeMap<String, Value>,
    name: &str,
) {
    let length_key = format!("{name}.$length");
    let count_key = format!("{name}.$count");
    let type_key = format!("{name}.$type");
    let empty_key = format!("{name}.$is_empty");
    variables.remove(&length_key);
    variables.remove(&count_key);
    variables.remove(&type_key);
    variables.remove(&empty_key);

    let Some(value) = variables.get(name) else {
        return;
    };
    let length = match value {
        Value::String(value) => value.encode_utf16().count(),
        Value::Array(values) => values.len(),
        Value::Object(fields) => fields.len(),
        Value::Null | Value::Bool(_) | Value::Number(_) => 0,
    };
    let is_empty = match value {
        Value::Null => true,
        Value::String(value) => value.is_empty(),
        Value::Array(values) => values.is_empty(),
        Value::Object(fields) => fields.is_empty(),
        Value::Bool(_) | Value::Number(_) => false,
    };
    let value_type = value_kind(value).to_owned();

    variables.insert(length_key, Value::Number(length.into()));
    variables.insert(count_key, Value::Number(length.into()));
    variables.insert(type_key, Value::String(value_type));
    variables.insert(empty_key, Value::Bool(is_empty));
}

pub(crate) fn empty_value_for_type(value_type: &str) -> Value {
    match value_type {
        "number" | "http_status_code" | "duration_ms" | "process_id" | "exit_code" => {
            Value::Number(0.into())
        }
        "boolean" => Value::Bool(false),
        "object" | "http_headers" => Value::Object(Map::new()),
        "http_response" => serde_json::json!({
            "type": "http_response",
            "status": 0,
            "headers": {},
            "body": ""
        }),
        "datetime" => serde_json::json!({
            "type": "datetime",
            "value": "1970-01-01T00:00:00.000Z"
        }),
        "duration" => serde_json::json!({
            "type": "duration",
            "unit": "seconds",
            "value": 0
        }),
        "list" => Value::Array(Vec::new()),
        _ => Value::String(String::new()),
    }
}

fn coerce_json_container(
    node: &RuntimeNode,
    value: Value,
    expect_object: bool,
) -> Result<Value, RuntimeError> {
    let value = match value {
        Value::String(text) => {
            serde_json::from_str(text.trim()).map_err(|source| RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("expected valid JSON: {source}"),
            })?
        }
        value => value,
    };
    let valid = if expect_object {
        value.is_object()
    } else {
        value.is_array()
    };
    if valid {
        Ok(value)
    } else {
        Err(RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!(
                "expected {}, found {}",
                if expect_object { "object" } else { "list" },
                value_kind(&value)
            ),
        })
    }
}

pub(crate) fn number_from_value(value: Option<&Value>) -> Option<f64> {
    let value = match value {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(value)) => value.parse::<f64>().ok(),
        _ => None,
    }?;
    value.is_finite().then_some(value)
}

pub(crate) fn number_value(node: &RuntimeNode, value: f64) -> Result<Value, RuntimeError> {
    Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(|| RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!("{value} cannot be represented as a JSON number"),
        })
}

pub(crate) fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => number_to_display_string(value),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn number_to_display_string(number: &Number) -> String {
    if let Some(value) = number.as_i64() {
        return value.to_string();
    }
    if let Some(value) = number.as_u64() {
        return value.to_string();
    }

    number
        .as_f64()
        .map(|value| ryu_js::Buffer::new().format(value).to_owned())
        .unwrap_or_else(|| number.to_string())
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::value_to_string;

    #[test]
    fn formats_numbers_like_editor_text_templates() {
        for (value, expected) in [
            (json!(1), "1"),
            (json!(1.0), "1"),
            (json!(1.5), "1.5"),
            (json!(-0.0), "0"),
            (json!(-42.0), "-42"),
            (json!(1e-7), "1e-7"),
            (json!(1e20), "100000000000000000000"),
            (json!(1e21), "1e+21"),
        ] {
            assert_eq!(value_to_string(&value), expected);
        }
    }
}
