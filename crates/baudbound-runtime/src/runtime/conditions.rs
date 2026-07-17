use regex::Regex;
use serde_json::Value;

use crate::runtime::{number_from_value, value_to_string};
pub(crate) fn compare_condition_values(
    left: &Value,
    operator: &str,
    right: &Value,
) -> Result<bool, String> {
    let left_text = value_to_string(left);
    let right_text = value_to_string(right);
    let left_number = number_from_value(Some(left));
    let right_number = number_from_value(Some(right));

    match operator {
        "==" => Ok(values_equal_for_condition(left, right)),
        "!=" => Ok(!values_equal_for_condition(left, right)),
        ">" => compare_numbers(left_number, right_number, |left, right| left > right),
        ">=" => compare_numbers(left_number, right_number, |left, right| left >= right),
        "<" => compare_numbers(left_number, right_number, |left, right| left < right),
        "<=" => compare_numbers(left_number, right_number, |left, right| left <= right),
        "contains" => Ok(left_text.contains(&right_text)),
        "starts_with" => Ok(left_text.starts_with(&right_text)),
        "ends_with" => Ok(left_text.ends_with(&right_text)),
        "regex_match" => safe_regex_match(&left_text, &right_text),
        "is_empty" => Ok(is_value_empty(left)),
        "is_null" => Ok(left.is_null()),
        other => Err(format!("unsupported comparison operator {other}")),
    }
}

pub(crate) fn values_equal_for_condition(left: &Value, right: &Value) -> bool {
    if left == right {
        return true;
    }

    if (left.is_number() || right.is_number())
        && let (Some(left), Some(right)) = (
            number_from_value(Some(left)),
            number_from_value(Some(right)),
        )
    {
        return left == right;
    }

    value_to_string(left) == value_to_string(right)
}

fn compare_numbers(
    left: Option<f64>,
    right: Option<f64>,
    compare: impl FnOnce(f64, f64) -> bool,
) -> Result<bool, String> {
    match (left, right) {
        (Some(left), Some(right)) => Ok(compare(left, right)),
        _ => Err("numeric comparison requires numeric values".to_owned()),
    }
}

fn safe_regex_match(value: &str, pattern: &str) -> Result<bool, String> {
    const MAX_REGEX_PATTERN_LENGTH: usize = 256;
    if pattern.len() > MAX_REGEX_PATTERN_LENGTH {
        return Err(format!(
            "regex pattern exceeds {MAX_REGEX_PATTERN_LENGTH} characters"
        ));
    }

    Regex::new(pattern)
        .map(|regex| regex.is_match(value))
        .map_err(|source| format!("invalid regex pattern: {source}"))
}

fn is_value_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(value) => value.is_empty(),
        Value::Array(values) => values.is_empty(),
        Value::Object(fields) => fields.is_empty(),
        Value::Bool(_) | Value::Number(_) => false,
    }
}
