//! The variable type vocabulary, shared with the editor through
//! `contracts/type-vocabulary.json`.
//!
//! A type is a rule that rejects at least one value. Names that describe where
//! a value came from are not types and are not present here.

use std::str::FromStr;

use serde_json::Value;

/// Largest whole number representable without loss, matching the editor.
pub(crate) use baudbound_script::MAX_SAFE_INTEGER;

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
            "hotkey" => Ok(Self::KeyboardKey),
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
        ValueType::KeyboardKey => "hotkey",
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
        ValueType::KeyboardKey => validate_hotkey(value),
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
    if !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&whole) {
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

fn validate_hotkey(value: &Value) -> Result<(), String> {
    let Some(text) = value.as_str() else {
        return Err(format!("expected keyboard key, found {}", describe(value)));
    };
    match baudbound_script::hotkey_error(text) {
        Some(reason) => Err(format!("expected keyboard key, {reason}")),
        None => Ok(()),
    }
}

/// Normalizes a single key-expression token the same way the editor does:
/// trim, lowercase, then drop spaces and underscores.
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

    #[test]
    fn hotkeys_must_exist_in_the_shared_contract() {
        let valid = serde_json::json!("F5");
        let invalid = serde_json::json!("NotARealKey");

        assert!(validate_value(&valid, ValueType::KeyboardKey).is_ok());
        assert!(validate_value(&invalid, ValueType::KeyboardKey).is_err());
    }

    #[test]
    fn integer_range_check_handles_i64_min_without_overflow() {
        assert!(validate_value(&serde_json::json!(i64::MIN), ValueType::Integer).is_err());
        assert!(validate_value(&serde_json::json!(MAX_SAFE_INTEGER), ValueType::Integer).is_ok());
        assert!(validate_value(&serde_json::json!(-MAX_SAFE_INTEGER), ValueType::Integer).is_ok());
        assert!(
            validate_value(&serde_json::json!(MAX_SAFE_INTEGER + 1), ValueType::Integer).is_err()
        );
        assert!(
            validate_value(
                &serde_json::json!(-MAX_SAFE_INTEGER - 1),
                ValueType::Integer
            )
            .is_err()
        );
    }
    #[test]
    fn the_integer_rule_matches_the_shared_package_rule() {
        // The manifest validator and Script Settings decide integer-ness with
        // baudbound_script::is_safe_integer. If the two ever disagree, a package
        // could install carrying a value the runtime then refuses.
        for value in [
            serde_json::json!(0),
            serde_json::json!(42),
            serde_json::json!(-42),
            serde_json::json!(MAX_SAFE_INTEGER),
            serde_json::json!(-MAX_SAFE_INTEGER),
            serde_json::json!(MAX_SAFE_INTEGER + 1),
            serde_json::json!(i64::MIN),
            serde_json::json!(u64::MAX),
            serde_json::json!(3.7),
            serde_json::json!("42"),
            serde_json::json!(null),
        ] {
            assert_eq!(
                validate_value(&value, ValueType::Integer).is_ok(),
                baudbound_script::is_safe_integer(&value),
                "the two integer rules disagree about {value}"
            );
        }
    }
}
