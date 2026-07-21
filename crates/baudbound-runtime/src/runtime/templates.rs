use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::runtime::value_to_string;
pub(crate) fn render_template(template: &str, variables: &BTreeMap<String, Value>) -> String {
    let mut output = String::new();
    let mut remaining = template;

    while let Some(start_index) = remaining.find("{{") {
        let (before, after_start) = remaining.split_at(start_index);
        output.push_str(before);
        let after_start = &after_start[2..];

        let Some(end_index) = after_start.find("}}") else {
            output.push_str("{{");
            output.push_str(after_start);
            return output;
        };

        let expression = after_start[..end_index].trim();
        if let Some(value) = resolve_variable_expression(expression, variables) {
            output.push_str(&value_to_string(value));
        } else {
            output.push_str("{{");
            output.push_str(expression);
            output.push_str("}}");
        }

        remaining = &after_start[end_index + 2..];
    }

    output.push_str(remaining);
    output
}

pub(crate) fn resolve_template_value(template: &str, variables: &BTreeMap<String, Value>) -> Value {
    let trimmed = template.trim();
    if let Some(expression) = trimmed
        .strip_prefix("{{")
        .and_then(|value| value.strip_suffix("}}"))
        .filter(|expression| !expression.contains("{{") && !expression.contains("}}"))
    {
        return resolve_variable_expression(expression.trim(), variables)
            .cloned()
            .unwrap_or_else(|| Value::String(trimmed.to_owned()));
    }

    Value::String(render_template(template, variables))
}

pub(crate) fn render_json_template(
    template: &str,
    variables: &BTreeMap<String, Value>,
) -> Result<String, serde_json::Error> {
    let value = serde_json::from_str::<Value>(template)?;
    serde_json::to_string(&resolve_json_template_value(value, variables))
}

fn resolve_json_template_value(value: Value, variables: &BTreeMap<String, Value>) -> Value {
    match value {
        Value::String(value) => resolve_template_value(&value, variables),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| resolve_json_template_value(value, variables))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| {
                    (
                        render_template(&key, variables),
                        resolve_json_template_value(value, variables),
                    )
                })
                .collect(),
        ),
        value => value,
    }
}

pub(crate) fn resolve_config_map(
    config: &Map<String, Value>,
    variables: &BTreeMap<String, Value>,
) -> Map<String, Value> {
    config
        .iter()
        .map(|(key, value)| (key.clone(), resolve_config_value(value, variables)))
        .collect()
}

fn resolve_config_value(value: &Value, variables: &BTreeMap<String, Value>) -> Value {
    match value {
        Value::String(value) => resolve_template_value(value, variables),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| resolve_config_value(value, variables))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(resolve_config_map(fields, variables)),
        other => other.clone(),
    }
}

fn resolve_variable_expression<'a>(
    expression: &str,
    variables: &'a BTreeMap<String, Value>,
) -> Option<&'a Value> {
    let mut best_name = None;
    for name in variables.keys() {
        if (expression == name || expression.starts_with(&format!("{name}.")))
            && best_name.is_none_or(|best: &String| name.len() > best.len())
        {
            best_name = Some(name);
        }
    }

    let name = best_name?;
    let value = variables.get(name)?;
    let suffix = expression.strip_prefix(name.as_str()).unwrap_or_default();
    if suffix == "." {
        return None;
    }
    let path = suffix.strip_prefix('.').unwrap_or_default();
    resolve_value_path(value, path)
}

fn resolve_value_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }

    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }
        current = match current {
            Value::Object(fields) => fields.get(segment)?,
            Value::Array(values) => values.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}
