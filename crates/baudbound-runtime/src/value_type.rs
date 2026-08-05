//! The variable type vocabulary, shared with the editor through
//! `contracts/type-vocabulary.json`.
//!
//! A type is a rule that rejects at least one value. Names that describe where
//! a value came from are not types and are not present here.

use std::str::FromStr;

use serde_json::Value;

/// Largest whole number representable without loss, matching the editor.
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    String,
    Integer,
    Float,
    Boolean,
    Object,
    List,
    Color,
    KeyboardKey,
    DateTime,
    Duration,
}

impl FromStr for ValueType {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "string" => Ok(Self::String),
            "integer" => Ok(Self::Integer),
            "float" => Ok(Self::Float),
            "boolean" => Ok(Self::Boolean),
            "object" => Ok(Self::Object),
            "list" => Ok(Self::List),
            "color" => Ok(Self::Color),
            "keyboard_key" => Ok(Self::KeyboardKey),
            "datetime" => Ok(Self::DateTime),
            "duration" => Ok(Self::Duration),
            other => Err(format!("unknown value type {other}")),
        }
    }
}

#[must_use]
pub fn value_type_name(value_type: ValueType) -> &'static str {
    match value_type {
        ValueType::String => "string",
        ValueType::Integer => "integer",
        ValueType::Float => "float",
        ValueType::Boolean => "boolean",
        ValueType::Object => "object",
        ValueType::List => "list",
        ValueType::Color => "color",
        ValueType::KeyboardKey => "keyboard_key",
        ValueType::DateTime => "datetime",
        ValueType::Duration => "duration",
    }
}

/// Reports whether a value satisfies a type.
///
/// Matching is exact. An integer does not satisfy `float` and a color does not
/// satisfy `string`, because there is no subtyping. Moving between types is
/// done with an explicit cast.
pub fn validate_value(value: &Value, value_type: ValueType) -> Result<(), String> {
    let name = value_type_name(value_type);
    if value.is_null() {
        return Err(format!("expected {name}, found no value"));
    }

    match value_type {
        ValueType::String => expect(value.is_string(), value, name),
        ValueType::Boolean => expect(value.is_boolean(), value, name),
        ValueType::Object => expect(value.is_object(), value, name),
        ValueType::List => expect(value.is_array(), value, name),
        ValueType::Integer => validate_integer(value),
        ValueType::Float => validate_float(value),
        ValueType::Color => validate_color(value),
        ValueType::KeyboardKey => validate_keyboard_key(value),
        ValueType::DateTime => validate_tagged(value, "datetime", &["type", "value"]),
        ValueType::Duration => validate_tagged(value, "duration", &["type", "unit", "value"]),
    }
}

fn expect(ok: bool, value: &Value, name: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("expected {name}, found {}", describe(value)))
    }
}

fn validate_integer(value: &Value) -> Result<(), String> {
    let Some(number) = value.as_number() else {
        return Err(format!("expected integer, found {}", describe(value)));
    };
    if number.as_f64().is_some() && number.as_i64().is_none() && number.as_u64().is_none() {
        return Err("expected integer, found a fractional number".to_owned());
    }
    let whole = number
        .as_i64()
        .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
        .ok_or_else(|| "expected integer, found a number outside the safe range".to_owned())?;
    if whole.abs() > MAX_SAFE_INTEGER {
        return Err("expected integer, found a number outside the safe range".to_owned());
    }
    Ok(())
}

fn validate_float(value: &Value) -> Result<(), String> {
    let Some(number) = value.as_number() else {
        return Err(format!("expected float, found {}", describe(value)));
    };
    if number.as_i64().is_some() || number.as_u64().is_some() {
        return Err("expected float, found an integer".to_owned());
    }
    if number.as_f64().is_some_and(f64::is_finite) {
        Ok(())
    } else {
        Err("expected float, found a value that is not a finite number".to_owned())
    }
}

fn validate_color(value: &Value) -> Result<(), String> {
    let Some(text) = value.as_str() else {
        return Err(format!("expected color, found {}", describe(value)));
    };
    let bytes = text.as_bytes();
    if bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit) {
        Ok(())
    } else {
        Err("expected color in #RRGGBB format".to_owned())
    }
}

fn validate_keyboard_key(value: &Value) -> Result<(), String> {
    let Some(text) = value.as_str() else {
        return Err(format!("expected keyboard key, found {}", describe(value)));
    };
    if text.trim().is_empty() {
        return Err("expected keyboard key, found an empty value".to_owned());
    }
    Ok(())
}

fn validate_tagged(value: &Value, tag: &str, fields: &[&str]) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Err(format!("expected {tag}, found {}", describe(value)));
    };
    if object.get("type").and_then(Value::as_str) != Some(tag) {
        return Err(format!("expected {tag}, found a differently tagged value"));
    }
    for field in fields {
        if !object.contains_key(*field) {
            return Err(format!("expected {tag}, missing field {field}"));
        }
    }
    Ok(())
}

fn describe(value: &Value) -> &'static str {
    match value {
        Value::Null => "no value",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "a list",
        Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct TypeConformance {
        cases: Vec<TypeCase>,
        version: u32,
    }

    #[derive(Deserialize)]
    struct TypeCase {
        reason: String,
        #[serde(rename = "type")]
        value_type: String,
        valid: bool,
        value: serde_json::Value,
    }

    #[test]
    fn shared_type_fixtures_conform() {
        let conformance: TypeConformance =
            serde_json::from_str(include_str!("../../../contracts/type-conformance.json"))
                .expect("shared type fixtures should parse");
        assert_eq!(conformance.version, 1);

        for case in conformance.cases {
            let value_type: ValueType = case
                .value_type
                .parse()
                .unwrap_or_else(|_| panic!("unknown type {}", case.value_type));
            assert_eq!(
                validate_value(&case.value, value_type).is_ok(),
                case.valid,
                "{} as {}: {}",
                case.value,
                case.value_type,
                case.reason
            );
        }
    }

    #[test]
    fn deleted_type_names_are_rejected() {
        for name in [
            "number",
            "file_content",
            "file_path",
            "http_headers",
            "process_id",
            "exit_code",
            "http_status_code",
            "duration_ms",
        ] {
            assert!(
                name.parse::<ValueType>().is_err(),
                "{name} was deleted and must not parse"
            );
        }
    }
}
