use serde_json::Value;

use crate::compile_safe_regex;
use crate::runtime::{number_from_value, value_to_string};

#[cfg(test)]
pub(crate) fn compare_condition_values(
    left: &Value,
    operator: &str,
    right: &Value,
) -> Result<bool, String> {
    compare_condition_values_with_end(left, operator, right, None)
}

pub(crate) fn compare_condition_values_with_end(
    left: &Value,
    operator: &str,
    right: &Value,
    right_end: Option<&Value>,
) -> Result<bool, String> {
    let left_text = value_to_string(left);
    let right_text = value_to_string(right);
    let left_number = number_from_value(Some(left));
    let right_number = number_from_value(Some(right));
    let right_end_number = number_from_value(right_end);

    match operator {
        "==" => Ok(values_equal_for_condition(left, right)),
        ">" => compare_numbers(left_number, right_number, |left, right| left > right),
        ">=" => compare_numbers(left_number, right_number, |left, right| left >= right),
        "<" => compare_numbers(left_number, right_number, |left, right| left < right),
        "<=" => compare_numbers(left_number, right_number, |left, right| left <= right),
        "is_between" => compare_range(left_number, right_number, right_end_number),
        "contains" => Ok(left_text.contains(&right_text)),
        "equals_ignore_case" => Ok(left_text.to_lowercase() == right_text.to_lowercase()),
        "contains_ignore_case" => Ok(left_text
            .to_lowercase()
            .contains(&right_text.to_lowercase())),
        "starts_with" => Ok(left_text.starts_with(&right_text)),
        "ends_with" => Ok(left_text.ends_with(&right_text)),
        "regex_match" => safe_regex_match(&left_text, &right_text),
        "is_empty" => Ok(is_value_empty(left)),
        "is_true" => Ok(left.as_bool() == Some(true)),
        "is_false" => Ok(left.as_bool() == Some(false)),
        "is_numeric" => Ok(number_from_value(Some(left)).is_some()),
        "is_string" => Ok(left.is_string()),
        "is_boolean" => Ok(left.is_boolean()),
        "is_list" => Ok(left.is_array()),
        "is_object" => Ok(left.is_object()),
        "has_key" => Ok(left
            .as_object()
            .is_some_and(|fields| fields.contains_key(&right_text))),
        "contains_item" => Ok(left.as_array().is_some_and(|values| {
            values
                .iter()
                .any(|value| values_equal_for_condition(value, right))
        })),
        "is_null_or_missing" => {
            Err("null or missing checks require an unresolved variable expression".to_owned())
        }
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

fn compare_range(value: Option<f64>, start: Option<f64>, end: Option<f64>) -> Result<bool, String> {
    let (Some(value), Some(start), Some(end)) = (value, start, end) else {
        return Err("between comparison requires numeric input, start, and end values".to_owned());
    };
    if start > end {
        return Err("between comparison start must be less than or equal to end".to_owned());
    }

    Ok(value >= start && value <= end)
}

fn safe_regex_match(value: &str, pattern: &str) -> Result<bool, String> {
    compile_safe_regex(pattern).map(|regex| regex.is_match(value))
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
