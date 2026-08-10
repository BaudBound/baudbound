use std::collections::BTreeMap;

use serde_json::{Map, Number, Value};

use baudbound_script::is_user_identifier;

use crate::{RuntimeError, RuntimeNode};

pub(crate) const DERIVED_VARIABLE_METADATA_SUFFIXES: [&str; 4] =
    [".$length", ".$count", ".$type", ".$is_empty"];
const MAX_AUTO_EXPANDED_LIST_ITEMS: usize = 100_000;

pub(crate) fn validate_variable_name(node: &RuntimeNode, name: &str) -> Result<(), RuntimeError> {
    if name.starts_with("system_")
        || name.starts_with("manifest_")
        || derived_suffixes().any(|suffix| name.ends_with(suffix))
    {
        return Err(RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!("{name} is read-only or reserved"),
        });
    }

    if !is_user_identifier(name) {
        return Err(RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!(
                "invalid variable name {name:?}; names may contain only ASCII letters, numbers, hyphens, and underscores"
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
        "string" => Ok(Value::String(value_to_string(&value))),
        // An integer must not be widened to f64 here. `Number::from_f64` always
        // produces the float variant, which would make every integer variable a
        // float the moment it was set.
        "integer" => integer_from_value(&value)
            .map(|whole| Value::Number(whole.into()))
            .ok_or_else(|| RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("expected integer, found {}", value_kind(&value)),
            }),
        "float" => number_from_value(Some(&value))
            .and_then(Number::from_f64)
            .map(Value::Number)
            .ok_or_else(|| RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("expected float, found {}", value_kind(&value)),
            }),
        "color" => Ok(Value::String(value_to_string(&value))),
        "boolean" => match value {
            Value::Bool(value) => Ok(Value::Bool(value)),
            Value::String(value) if value.trim().eq_ignore_ascii_case("true") => {
                Ok(Value::Bool(true))
            }
            Value::String(value) if value.trim().eq_ignore_ascii_case("false") => {
                Ok(Value::Bool(false))
            }
            other => Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("expected boolean, found {}", value_kind(&other)),
            }),
        },
        "object" | "datetime" | "duration" => coerce_json_container(node, value, true),
        "list" => coerce_json_container(node, value, false),
        "hotkey" => Ok(Value::String(value_to_string(&value))),
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

pub(crate) fn remove_object_field(
    node: &RuntimeNode,
    target: &mut Value,
    field_path: &str,
) -> Result<bool, RuntimeError> {
    let segments =
        parse_object_path(field_path).map_err(|message| RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message,
        })?;
    Ok(remove_path_value(target, &segments))
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
            if array_index >= MAX_AUTO_EXPANDED_LIST_ITEMS {
                return Err(format!(
                    "object field path index {array_index} exceeds the maximum supported index {}",
                    MAX_AUTO_EXPANDED_LIST_ITEMS - 1
                ));
            }
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

fn remove_path_value(target: &mut Value, segments: &[ObjectPathSegment]) -> bool {
    let Some((segment, remaining)) = segments.split_first() else {
        return false;
    };

    if remaining.is_empty() {
        return match segment {
            ObjectPathSegment::Field(field) => target
                .as_object_mut()
                .is_some_and(|object| object.remove(field).is_some()),
            ObjectPathSegment::Index(index) => target.as_array_mut().is_some_and(|items| {
                if *index >= items.len() {
                    return false;
                }
                items.remove(*index);
                true
            }),
        };
    }

    match segment {
        ObjectPathSegment::Field(field) => target
            .as_object_mut()
            .and_then(|object| object.get_mut(field))
            .is_some_and(|child| remove_path_value(child, remaining)),
        ObjectPathSegment::Index(index) => target
            .as_array_mut()
            .and_then(|items| items.get_mut(*index))
            .is_some_and(|child| remove_path_value(child, remaining)),
    }
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

/// Parts of a point in time. Singular, because a datetime has one hour.
pub(crate) const DATETIME_PART_SUFFIXES: [&str; 7] = [
    ".$year",
    ".$month",
    ".$day",
    ".$hour",
    ".$minute",
    ".$second",
    ".$weekday",
];

/// Parts of a span. Plural, because a duration is some number of hours.
///
/// A component breakdown rather than a total in each unit, so ninety seconds
/// is one minute and thirty seconds. Every part is then a whole number, and
/// `.$total_milliseconds` covers the how-long-altogether question that a
/// fractional `.$minutes` would otherwise have to answer.
pub(crate) const DURATION_PART_SUFFIXES: [&str; 6] = [
    ".$days",
    ".$hours",
    ".$minutes",
    ".$seconds",
    ".$milliseconds",
    ".$total_milliseconds",
];

/// Every suffix the runner owns, so none of them can be written by a script.
pub(crate) fn derived_suffixes() -> impl Iterator<Item = &'static str> {
    DERIVED_VARIABLE_METADATA_SUFFIXES
        .into_iter()
        .chain(DATETIME_PART_SUFFIXES)
        .chain(DURATION_PART_SUFFIXES)
}

/// The parts of a datetime value, read in the offset the value carries.
fn datetime_parts(value: &Value) -> Option<Vec<(&'static str, i64)>> {
    if value.get("type").and_then(Value::as_str) != Some("datetime") {
        return None;
    }
    let text = value.get("value").and_then(Value::as_str)?;
    let parsed = chrono::DateTime::parse_from_rfc3339(text).ok()?;

    // Read through the offset carried by the value rather than converting to
    // UTC, so the hour is the wall clock the value was written in.
    use chrono::{Datelike as _, Timelike as _};
    Some(vec![
        (".$year", i64::from(parsed.year())),
        (".$month", i64::from(parsed.month())),
        (".$day", i64::from(parsed.day())),
        (".$hour", i64::from(parsed.hour())),
        (".$minute", i64::from(parsed.minute())),
        (".$second", i64::from(parsed.second())),
        // Monday is 1 through Sunday is 7, the ISO numbering, so a comparison
        // does not depend on which day a locale calls the first.
        (
            ".$weekday",
            i64::from(parsed.weekday().number_from_monday()),
        ),
    ])
}

/// The parts of a duration value, broken into components.
fn duration_parts(value: &Value) -> Option<Vec<(&'static str, i64)>> {
    if value.get("type").and_then(Value::as_str) != Some("duration") {
        return None;
    }
    let amount = value.get("value").and_then(Value::as_f64)?;
    if !amount.is_finite() || amount < 0.0 {
        return None;
    }
    let unit_ms = match value.get("unit").and_then(Value::as_str)? {
        "milliseconds" => 1.0,
        "seconds" => 1_000.0,
        "minutes" => 60_000.0,
        "hours" => 3_600_000.0,
        "days" => 86_400_000.0,
        _ => return None,
    };
    let total = (amount * unit_ms).round();
    if total > i64::MAX as f64 {
        return None;
    }
    let total = total as i64;

    Some(vec![
        (".$days", total / 86_400_000),
        (".$hours", (total % 86_400_000) / 3_600_000),
        (".$minutes", (total % 3_600_000) / 60_000),
        (".$seconds", (total % 60_000) / 1_000),
        (".$milliseconds", total % 1_000),
        (".$total_milliseconds", total),
    ])
}

fn derived_length(value: &Value) -> usize {
    match value {
        Value::String(value) => value.encode_utf16().count(),
        Value::Array(values) => values.len(),
        Value::Object(fields) => fields.len(),
        Value::Null | Value::Bool(_) | Value::Number(_) => 0,
    }
}

fn derived_is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(value) => value.is_empty(),
        Value::Array(values) => values.is_empty(),
        Value::Object(fields) => fields.is_empty(),
        Value::Bool(_) | Value::Number(_) => false,
    }
}

/// The metadata a `$` segment names, computed rather than stored.
///
/// A top-level variable also carries these as flat sibling keys so the
/// Variables panel can list them, but a value reached through a path has no
/// key of its own. Computing here is what lets `{{settings.label.$length}}`
/// resolve at all.
pub(crate) fn derived_metadata_value(value: &Value, segment: &str) -> Option<Value> {
    match segment {
        "$length" | "$count" => Some(Value::Number(derived_length(value).into())),
        "$type" => Some(Value::String(value_kind(value).to_owned())),
        "$is_empty" => Some(Value::Bool(derived_is_empty(value))),
        _ => None,
    }
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

    // A value can change type between writes, so every part key is cleared
    // before the applicable ones are added back. Leaving a stale .$hour on a
    // value that is no longer a datetime would read as real.
    for suffix in DATETIME_PART_SUFFIXES
        .into_iter()
        .chain(DURATION_PART_SUFFIXES)
    {
        variables.remove(&format!("{name}{suffix}"));
    }

    let Some(value) = variables.get(name) else {
        return;
    };
    let length = derived_length(value);
    let parts = datetime_parts(value).or_else(|| duration_parts(value));
    let value_type = value_kind(value).to_owned();
    let is_empty = derived_is_empty(value);

    variables.insert(length_key, Value::Number(length.into()));
    variables.insert(count_key, Value::Number(length.into()));
    variables.insert(type_key, Value::String(value_type));
    variables.insert(empty_key, Value::Bool(is_empty));

    if let Some(parts) = parts {
        for (suffix, part) in parts {
            variables.insert(format!("{name}{suffix}"), Value::Number(part.into()));
        }
    }
}

/// The value a `clear` operation leaves behind for a type.
///
/// Returns `None` for a type that has no empty member. A keyboard key is the
/// only such type: every valid value names at least one real key, so there is
/// nothing to clear it to. Storing an empty string there would leave a value
/// that its own type rejects.
pub(crate) fn empty_value_for_declared_type(value_type: &str) -> Option<Value> {
    match value_type {
        "hotkey" => None,
        other => Some(empty_value_for_type(other)),
    }
}

pub(crate) fn empty_value_for_type(value_type: &str) -> Value {
    match value_type {
        "integer" => Value::Number(0.into()),
        // A float must stay a float. `0.into()` would build the integer
        // variant, leaving an integer inside a float variable.
        "float" => Number::from_f64(0.0)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        "color" => Value::String("#000000".to_owned()),
        "boolean" => Value::Bool(false),
        "object" => Value::Object(Map::new()),
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

/// Reads a value as a whole number without going through `f64`.
///
/// A fractional number is rejected rather than truncated, because `integer` and
/// `float` are disjoint types and silently dropping a fraction would be an
/// invisible conversion between them. A string is accepted only when it parses
/// as a whole number, matching the string handling in `number_from_value`.
pub(crate) fn integer_from_value(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(value) => value.trim().parse::<i64>().ok(),
        _ => None,
    }
}

pub(crate) fn number_from_value(value: Option<&Value>) -> Option<f64> {
    let value = match value {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(value)) => value.trim().parse::<f64>().ok(),
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
        .map(|value| {
            let rendered = ryu_js::Buffer::new().format(value).to_owned();
            // ryu-js follows JavaScript, which prints 42.0 as "42". A float must
            // stay visibly a float, otherwise the separation from integer
            // disappears the moment a value is used in text.
            if rendered.contains(['.', 'e', 'E', 'n']) {
                rendered
            } else {
                format!("{rendered}.0")
            }
        })
        .unwrap_or_else(|| number.to_string())
}

pub(crate) fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        // integer and float are separate types, so reporting a bare "number"
        // would name a type that no longer exists.
        Value::Number(number) if number.as_i64().is_some() || number.as_u64().is_some() => {
            "integer"
        }
        Value::Number(_) => "float",
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
            (json!(1.0), "1.0"),
            (json!(1.5), "1.5"),
            (json!(-0.0), "0.0"),
            (json!(-42.0), "-42.0"),
            (json!(1e-7), "1e-7"),
            (json!(1e20), "100000000000000000000.0"),
            (json!(1e21), "1e+21"),
        ] {
            assert_eq!(value_to_string(&value), expected);
        }
    }
}

#[cfg(test)]
mod float_rendering_tests {
    use super::value_to_string;

    #[test]
    fn whole_floats_render_with_a_decimal() {
        let float: serde_json::Value = serde_json::from_str("42.0").expect("float parses");
        assert_eq!(value_to_string(&float), "42.0");
    }

    #[test]
    fn fractional_floats_render_unchanged() {
        let float: serde_json::Value = serde_json::from_str("3.7").expect("float parses");
        assert_eq!(value_to_string(&float), "3.7");
    }

    #[test]
    fn integers_render_without_a_decimal() {
        let integer: serde_json::Value = serde_json::from_str("42").expect("integer parses");
        assert_eq!(value_to_string(&integer), "42");
    }
}

#[cfg(test)]
mod derived_part_tests {
    use super::*;
    use serde_json::json;

    fn parts_of(value: Value) -> BTreeMap<String, Value> {
        let mut variables = BTreeMap::from([("x".to_owned(), value)]);
        refresh_derived_variable_metadata(&mut variables, "x");
        variables
    }

    #[test]
    fn a_datetime_is_read_in_the_offset_it_carries() {
        // Not converted to UTC first: the hour is the wall clock the value was
        // written in, which is the one an author means.
        let variables =
            parts_of(json!({ "type": "datetime", "value": "2026-07-03T14:30:45+03:00" }));

        assert_eq!(variables["x.$year"], json!(2026));
        assert_eq!(variables["x.$month"], json!(7));
        assert_eq!(variables["x.$day"], json!(3));
        assert_eq!(variables["x.$hour"], json!(14));
        assert_eq!(variables["x.$minute"], json!(30));
        assert_eq!(variables["x.$second"], json!(45));
        // 3 July 2026 is a Friday, fifth day counting from Monday.
        assert_eq!(variables["x.$weekday"], json!(5));

        // A different offset must not change the reading. Converting to UTC
        // first would report 19 here rather than the wall clock written.
        let elsewhere =
            parts_of(json!({ "type": "datetime", "value": "2026-07-03T14:30:45-05:00" }));
        assert_eq!(elsewhere["x.$hour"], json!(14));
        assert_eq!(elsewhere["x.$day"], json!(3));
        assert_eq!(elsewhere["x.$weekday"], json!(5));
    }

    #[test]
    fn a_duration_breaks_into_whole_components() {
        // Ninety seconds is one minute and thirty seconds, not 1.5 minutes, so
        // every part stays a whole number.
        let variables = parts_of(json!({ "type": "duration", "unit": "seconds", "value": 90 }));

        assert_eq!(variables["x.$days"], json!(0));
        assert_eq!(variables["x.$hours"], json!(0));
        assert_eq!(variables["x.$minutes"], json!(1));
        assert_eq!(variables["x.$seconds"], json!(30));
        assert_eq!(variables["x.$milliseconds"], json!(0));
        assert_eq!(variables["x.$total_milliseconds"], json!(90_000));
    }

    #[test]
    fn a_long_duration_carries_days_and_a_remainder() {
        let variables = parts_of(json!({ "type": "duration", "unit": "hours", "value": 50 }));

        assert_eq!(variables["x.$days"], json!(2));
        assert_eq!(variables["x.$hours"], json!(2));
        assert_eq!(variables["x.$total_milliseconds"], json!(180_000_000));
    }

    #[test]
    fn a_value_that_is_not_a_datetime_gets_no_parts() {
        let variables = parts_of(json!("just text"));

        assert!(!variables.contains_key("x.$hour"));
        assert!(!variables.contains_key("x.$minutes"));
        // The metadata every value has is still there.
        assert_eq!(variables["x.$type"], json!("string"));
    }

    #[test]
    fn parts_do_not_survive_the_value_changing_type() {
        // A stale .$hour on something that is no longer a datetime would read
        // as real, so every part key is cleared before the new ones are added.
        let mut variables = BTreeMap::from([(
            "x".to_owned(),
            json!({ "type": "datetime", "value": "2026-07-03T14:30:45+03:00" }),
        )]);
        refresh_derived_variable_metadata(&mut variables, "x");
        assert_eq!(variables["x.$hour"], json!(14));

        variables.insert("x".to_owned(), json!("now a string"));
        refresh_derived_variable_metadata(&mut variables, "x");

        assert!(
            !variables.contains_key("x.$hour"),
            "the old part must be gone"
        );
        assert!(!variables.contains_key("x.$weekday"));
    }

    #[test]
    fn a_malformed_datetime_produces_no_parts_rather_than_wrong_ones() {
        let variables = parts_of(json!({ "type": "datetime", "value": "not a date" }));
        assert!(!variables.contains_key("x.$hour"));
    }

    #[test]
    fn every_part_suffix_is_reserved_from_writes() {
        let node = RuntimeNode {
            id: "n-1".to_owned(),
            action_type: "runtime.set_variable".to_owned(),
            node_type: "set_variable".to_owned(),
            action: None,
            config: serde_json::Map::new(),
        };
        for suffix in derived_suffixes() {
            let name = format!("mine{suffix}");
            assert!(
                validate_variable_name(&node, &name).is_err(),
                "{name} must not be writable"
            );
        }
    }
}

#[cfg(test)]
mod part_conformance_tests {
    use std::collections::BTreeMap;

    use serde::Deserialize;
    use serde_json::Value;

    use super::{
        DATETIME_PART_SUFFIXES, DURATION_PART_SUFFIXES, refresh_derived_variable_metadata,
    };

    #[derive(Deserialize)]
    struct PartConformance {
        cases: Vec<PartCase>,
        datetime_parts: Vec<String>,
        duration_parts: Vec<String>,
        version: u32,
    }

    #[derive(Deserialize)]
    struct PartCase {
        parts: BTreeMap<String, i64>,
        reason: String,
        value: Value,
    }

    fn conformance() -> PartConformance {
        serde_json::from_str(include_str!(
            "../../../../contracts/datetime-part-conformance.json"
        ))
        .expect("shared part fixtures should parse")
    }

    #[test]
    fn the_shared_fixtures_name_the_parts_this_runner_produces() {
        // Without this, a part added on one side only would go unnoticed: the
        // cases below assert what they list, not what is missing.
        let conformance = conformance();
        assert_eq!(conformance.version, 1);

        let named = |suffixes: &[&str]| {
            suffixes
                .iter()
                .map(|suffix| suffix.trim_start_matches('.').to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(conformance.datetime_parts, named(&DATETIME_PART_SUFFIXES));
        assert_eq!(conformance.duration_parts, named(&DURATION_PART_SUFFIXES));
    }

    #[test]
    fn shared_part_fixtures_conform() {
        for case in conformance().cases {
            // Through the real refresh, so the fixtures cover what a script
            // actually reads rather than a helper it never reaches.
            let mut variables = BTreeMap::from([("value".to_owned(), case.value.clone())]);
            refresh_derived_variable_metadata(&mut variables, "value");

            for (part, expected) in case.parts {
                let key = format!("value.{part}");
                assert_eq!(
                    variables.get(&key),
                    Some(&Value::from(expected)),
                    "{key} for {}: {}",
                    case.value,
                    case.reason
                );
            }
        }
    }
}
