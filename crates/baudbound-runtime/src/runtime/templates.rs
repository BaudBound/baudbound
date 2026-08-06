use std::borrow::Cow;
use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::ValueType;
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
            output.push_str(&value_to_string(&value));
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

pub fn resolve_template_value(template: &str, variables: &BTreeMap<String, Value>) -> Value {
    if let Some(expression) = template
        .strip_prefix("{{")
        .and_then(|value| value.strip_suffix("}}"))
        .filter(|expression| !expression.contains("{{") && !expression.contains("}}"))
    {
        return resolve_variable_expression(expression.trim(), variables)
            .map(Cow::into_owned)
            .unwrap_or_else(|| Value::String(template.to_owned()));
    }

    Value::String(render_template(template, variables))
}

pub(crate) fn template_value_is_defined(
    template: &str,
    variables: &BTreeMap<String, Value>,
) -> bool {
    let Some(expression) = template
        .strip_prefix("{{")
        .and_then(|value| value.strip_suffix("}}"))
        .filter(|expression| !expression.contains("{{") && !expression.contains("}}"))
    else {
        return true;
    };

    resolve_variable_expression(expression.trim(), variables).is_some()
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

pub(crate) fn resolve_config_value(value: &Value, variables: &BTreeMap<String, Value>) -> Value {
    match value {
        Value::String(value) => resolve_template_value(value, variables),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| resolve_config_value(value, variables))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| {
                    (
                        render_template(key, variables),
                        resolve_config_value(value, variables),
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Splits a template expression into its reference and optional cast target.
///
/// The split happens outside quote context so that a pipe inside a quoted
/// accessor, as in `node["a|b"]`, stays part of the key. Only the last
/// unquoted pipe is treated as the cast separator.
pub(crate) fn split_cast(expression: &str) -> (&str, Option<&str>) {
    let mut quote = None;
    let mut escaped = false;
    let mut separator = None;

    for (index, character) in expression.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        match quote {
            Some(active) if character == active => quote = None,
            Some(_) => {}
            None if character == '"' || character == '\'' => quote = Some(character),
            None if character == '|' => separator = Some(index),
            None => {}
        }
    }

    match separator {
        Some(index) => (
            expression[..index].trim(),
            Some(expression[index + 1..].trim()),
        ),
        None => (expression.trim(), None),
    }
}

fn resolve_variable_expression<'a>(
    expression: &str,
    variables: &'a BTreeMap<String, Value>,
) -> Option<Cow<'a, Value>> {
    let (reference, target) = split_cast(expression);
    let resolved = resolve_reference(reference, variables)?;
    let Some(target) = target else {
        return Some(Cow::Borrowed(resolved));
    };
    let target = target.parse::<ValueType>().ok()?;
    crate::cast_value(resolved, target).ok().map(Cow::Owned)
}

pub(crate) fn resolve_reference<'a>(
    expression: &str,
    variables: &'a BTreeMap<String, Value>,
) -> Option<&'a Value> {
    let mut best_name = None;
    for name in variables.keys() {
        if (expression == name
            || expression
                .strip_prefix(name.as_str())
                .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('[')))
            && best_name.is_none_or(|best: &String| name.len() > best.len())
        {
            best_name = Some(name);
        }
    }

    let name = best_name?;
    let value = variables.get(name)?;
    let suffix = expression.strip_prefix(name.as_str()).unwrap_or_default();
    resolve_value_path(value, suffix)
}

fn resolve_value_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }

    let mut current = value;
    let mut remaining = path;
    while !remaining.is_empty() {
        if let Some(after_dot) = remaining.strip_prefix('.') {
            let segment_end = after_dot.find(['.', '[']).unwrap_or(after_dot.len());
            let segment = &after_dot[..segment_end];
            if segment.is_empty() {
                return None;
            }
            current = current.as_object()?.get(segment)?;
            remaining = &after_dot[segment_end..];
            continue;
        }

        let after_open = remaining.strip_prefix('[')?;
        let close_index = find_bracket_end(after_open)?;
        let accessor = after_open[..close_index].trim();
        current = if let Ok(index) = accessor.parse::<usize>() {
            current.as_array()?.get(index)?
        } else {
            let key = parse_quoted_accessor(accessor)?;
            current.as_object()?.get(&key)?
        };
        remaining = &after_open[close_index + 1..];
    }
    Some(current)
}

fn find_bracket_end(value: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        match quote {
            Some(active) if character == active => quote = None,
            Some(_) => {}
            None if character == '"' || character == '\'' => quote = Some(character),
            None if character == ']' => return Some(index),
            None => {}
        }
    }
    None
}

fn parse_quoted_accessor(value: &str) -> Option<String> {
    if value.starts_with('"') {
        return serde_json::from_str::<String>(value).ok();
    }
    let inner = value.strip_prefix('\'')?.strip_suffix('\'')?;
    let mut output = String::with_capacity(inner.len());
    let mut escaped = false;
    for character in inner.chars() {
        if escaped {
            output.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            output.push(character);
        }
    }
    (!escaped).then_some(output)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn resolves_dot_array_and_quoted_object_paths() {
        let variables = BTreeMap::from([(
            "node.query_parameters".to_owned(),
            json!([
                {
                    "name": "token",
                    "headers": {
                        "content-type": "application/json"
                    }
                }
            ]),
        )]);

        assert_eq!(
            resolve_template_value(
                "{{node.query_parameters[0].headers[\"content-type\"]}}",
                &variables
            ),
            json!("application/json")
        );
        assert_eq!(
            resolve_template_value("{{node.query_parameters[0]['name']}}", &variables),
            json!("token")
        );
    }

    #[test]
    fn resolves_nested_config_values_and_user_defined_keys() {
        let variables = BTreeMap::from([
            ("field".to_owned(), json!("dynamic")),
            ("value".to_owned(), json!({"nested": true})),
        ]);
        let config = json!({
            "static": {
                "{{field}}": ["{{value}}"]
            }
        });

        assert_eq!(
            resolve_config_map(config.as_object().expect("config object"), &variables),
            json!({
                "static": {
                    "dynamic": [{"nested": true}]
                }
            })
            .as_object()
            .expect("expected object")
            .clone()
        );
    }

    #[test]
    fn exact_value_references_do_not_consume_surrounding_whitespace() {
        let variables = BTreeMap::from([("value".to_owned(), json!({"nested": true}))]);

        assert_eq!(
            resolve_template_value("{{value}}", &variables),
            json!({"nested": true})
        );
        assert_eq!(
            resolve_template_value(" {{value}} ", &variables),
            json!(" {\"nested\":true} ")
        );
        assert!(template_value_is_defined(" {{missing}} ", &variables));
        assert!(!template_value_is_defined("{{missing}}", &variables));
    }

    #[test]
    fn splits_a_cast_suffix_outside_quotes() {
        assert_eq!(split_cast("item"), ("item", None));
        assert_eq!(split_cast("item|string"), ("item", Some("string")));
        assert_eq!(split_cast(" item | string "), ("item", Some("string")));
        assert_eq!(
            split_cast("node.items[0].name|string"),
            ("node.items[0].name", Some("string"))
        );

        // A pipe inside a quoted accessor is part of the key, not a cast.
        assert_eq!(split_cast("node[\"a|b\"]"), ("node[\"a|b\"]", None));
        assert_eq!(
            split_cast("node[\"a|b\"]|string"),
            ("node[\"a|b\"]", Some("string"))
        );
    }
}
