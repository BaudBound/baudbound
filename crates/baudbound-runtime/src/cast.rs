//! Conversion between value types.
//!
//! One implementation, used by the Convert Value node and by the inline cast.
//! The two differ only in how a failure is handled: the node takes its failure
//! output, the inline cast stops the run.

use baudbound_script::is_safe_integer;
use serde_json::{Number, Value};

use crate::{ValueType, validate_value, value_type_name};

/// Converts a value to a target type.
///
/// `null` fails for every target without exception. It is what an unset
/// optional variable and a missing object key both resolve to, and silently
/// producing an empty string or the text "null" would hide a missing value.
pub fn cast_value(value: &Value, target: ValueType) -> Result<Value, String> {
    let name = value_type_name(target);
    if value.is_null() {
        return Err(format!(
            "cannot cast to {name} because the value is not set"
        ));
    }

    match target {
        ValueType::String => Ok(Value::String(match value {
            Value::String(text) => text.clone(),
            other => serde_json::to_string(other)
                .map_err(|source| format!("cannot cast to string: {source}"))?,
        })),
        ValueType::Integer => cast_integer(value),
        ValueType::Float => cast_float(value),
        ValueType::Boolean => cast_boolean(value),
        ValueType::List => cast_parsed(value, target, Value::is_array),
        ValueType::Object => cast_parsed(value, target, Value::is_object),
        ValueType::Color | ValueType::KeyboardKey => cast_checked_string(value, target),
        ValueType::DateTime => cast_datetime(value),
        ValueType::Duration => cast_duration(value),
    }
}

fn numeric_input(value: &Value, name: &str) -> Result<f64, String> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) if !text.trim().is_empty() => text.trim().parse::<f64>().ok(),
        _ => None,
    }
    .filter(|parsed| parsed.is_finite())
    .ok_or_else(|| format!("cannot cast to {name} because the value is not a finite number"))
}

fn cast_integer(value: &Value) -> Result<Value, String> {
    let parsed = numeric_input(value, "integer")?;
    if parsed.fract() != 0.0 {
        return Err("cannot cast to integer because the value is fractional".to_owned());
    }
    // is_safe_integer is the same rule the manifest validator and Script
    // Settings use, so a cast and a declared value can never disagree about
    // which whole numbers are safe integers.
    let whole = Value::Number(Number::from(parsed as i64));
    if is_safe_integer(&whole) {
        Ok(whole)
    } else {
        Err("cannot cast to integer because the value is outside the safe range".to_owned())
    }
}

fn cast_float(value: &Value) -> Result<Value, String> {
    let parsed = numeric_input(value, "float")?;
    // from_f64 always produces the Float variant, so casting the integer 42
    // yields 42.0 and renders with a decimal.
    Number::from_f64(parsed)
        .map(Value::Number)
        .ok_or_else(|| "cannot cast to float because the value is not finite".to_owned())
}

fn cast_boolean(value: &Value) -> Result<Value, String> {
    match value {
        Value::Bool(flag) => Ok(Value::Bool(*flag)),
        Value::String(text) if text.trim().eq_ignore_ascii_case("true") => Ok(Value::Bool(true)),
        Value::String(text) if text.trim().eq_ignore_ascii_case("false") => Ok(Value::Bool(false)),
        _ => Err("cannot cast to boolean because the value is not true or false".to_owned()),
    }
}

fn cast_parsed(
    value: &Value,
    target: ValueType,
    accept: fn(&Value) -> bool,
) -> Result<Value, String> {
    let name = value_type_name(target);
    let parsed = match value {
        Value::String(text) => serde_json::from_str(text).unwrap_or_else(|_| value.clone()),
        other => other.clone(),
    };
    if accept(&parsed) {
        Ok(parsed)
    } else {
        Err(format!(
            "cannot cast to {name} because the value is not one"
        ))
    }
}

fn cast_checked_string(value: &Value, target: ValueType) -> Result<Value, String> {
    let name = value_type_name(target);
    let Value::String(_) = value else {
        return Err(format!(
            "cannot cast to {name} because the value is not text"
        ));
    };
    validate_value(value, target).map(|()| value.clone())
}

fn cast_datetime(value: &Value) -> Result<Value, String> {
    if validate_value(value, ValueType::DateTime).is_ok() {
        return Ok(value.clone());
    }
    let Value::String(text) = value else {
        return Err("cannot cast to datetime because the value is not a date".to_owned());
    };
    // RFC 3339 parsing is what the rest of the runtime already uses to decide
    // whether a datetime string is genuine (see variable_operations.rs), so a
    // string such as "not a date" is rejected instead of accepted as-is.
    if chrono::DateTime::parse_from_rfc3339(text).is_err() {
        return Err("cannot cast to datetime because the value is not a date".to_owned());
    }
    Ok(serde_json::json!({ "type": "datetime", "value": text }))
}

fn cast_duration(value: &Value) -> Result<Value, String> {
    validate_value(value, ValueType::Duration)
        .map(|()| value.clone())
        .map_err(|_| "cannot cast to duration because the value has no unit".to_owned())
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::ValueType;

    #[derive(Deserialize)]
    struct CastConformance {
        cases: Vec<CastCase>,
        version: u32,
    }

    #[derive(Deserialize)]
    struct CastCase {
        #[serde(default)]
        error: bool,
        reason: String,
        #[serde(default)]
        result: Option<serde_json::Value>,
        target: String,
        value: serde_json::Value,
    }

    #[test]
    fn shared_cast_fixtures_conform() {
        let conformance: CastConformance =
            serde_json::from_str(include_str!("../../../contracts/cast-conformance.json"))
                .expect("shared cast fixtures should parse");
        assert_eq!(conformance.version, 1);

        for case in conformance.cases {
            let target: ValueType = case
                .target
                .parse()
                .unwrap_or_else(|_| panic!("unknown target {}", case.target));
            let outcome = cast_value(&case.value, target);

            if case.error {
                assert!(
                    outcome.is_err(),
                    "{} to {} should fail: {}",
                    case.value,
                    case.target,
                    case.reason
                );
            } else {
                let expected = case
                    .result
                    .expect("a successful case must declare a result");
                assert_eq!(
                    outcome.expect("cast should succeed"),
                    expected,
                    "{} to {}: {}",
                    case.value,
                    case.target,
                    case.reason
                );
            }
        }
    }

    #[test]
    fn a_cast_result_always_satisfies_its_target() {
        for (value, target) in [
            (serde_json::json!(42), ValueType::String),
            (serde_json::json!("42"), ValueType::Integer),
            (serde_json::json!(42), ValueType::Float),
            (serde_json::json!("#ff0000"), ValueType::Color),
        ] {
            let cast = cast_value(&value, target).expect("cast should succeed");
            assert!(
                crate::validate_value(&cast, target).is_ok(),
                "casting to {target:?} must produce a value that satisfies it"
            );
        }
    }
}
