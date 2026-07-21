use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::{render_json_template, resolve_config_map};

pub(crate) fn resolve_http_request_config(
    config: &Map<String, Value>,
    variables: &BTreeMap<String, Value>,
) -> Result<Map<String, Value>, String> {
    let mut resolved = resolve_config_map(config, variables);
    if !uses_json_body(config, &resolved)? {
        return Ok(resolved);
    }

    let body = config
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if body.trim().is_empty() {
        return Ok(resolved);
    }

    let body = render_json_template(body, variables)
        .map_err(|error| format!("invalid JSON HTTP request body: {error}"))?;
    resolved.insert("body".to_owned(), Value::String(body));
    Ok(resolved)
}

fn uses_json_body(
    original: &Map<String, Value>,
    resolved: &Map<String, Value>,
) -> Result<bool, String> {
    match original
        .get("bodyFormat")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
    {
        "json" => Ok(true),
        "text" => Ok(false),
        "" => Ok(has_json_content_type(resolved.get("headers"))),
        value => Err(format!(
            "HTTP request body format must be json or text, found {value}"
        )),
    }
}

fn has_json_content_type(headers: Option<&Value>) -> bool {
    match headers {
        Some(Value::Array(rows)) => rows.iter().any(|row| {
            let Some(row) = row.as_object() else {
                return false;
            };
            is_content_type(row.get("name"), row.get("value"))
        }),
        Some(Value::Object(values)) => values
            .iter()
            .any(|(name, value)| is_content_type(Some(&Value::String(name.clone())), Some(value))),
        _ => false,
    }
}

fn is_content_type(name: Option<&Value>, value: Option<&Value>) -> bool {
    let Some(name) = name.and_then(Value::as_str) else {
        return false;
    };
    let Some(value) = value.and_then(Value::as_str) else {
        return false;
    };

    name.eq_ignore_ascii_case("content-type") && is_json_media_type(value)
}

fn is_json_media_type(value: &str) -> bool {
    let media_type = value.split(';').next().unwrap_or_default().trim();
    media_type.eq_ignore_ascii_case("application/json")
        || media_type
            .to_ascii_lowercase()
            .strip_prefix("application/")
            .is_some_and(|subtype| subtype.ends_with("+json"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn json_body_escapes_control_characters_and_quotes() {
        let config = json!({
            "bodyFormat": "json",
            "body": "{\"data\":\"{{scanner.data}}\"}"
        })
        .as_object()
        .expect("config should be an object")
        .clone();
        let variables = BTreeMap::from([(
            "scanner.data".to_owned(),
            Value::String("value\r\n\"quoted\"".to_owned()),
        )]);

        let resolved = resolve_http_request_config(&config, &variables)
            .expect("JSON body should resolve safely");
        let body = resolved
            .get("body")
            .and_then(Value::as_str)
            .expect("body should remain text for the HTTP client");

        assert_eq!(
            serde_json::from_str::<Value>(body).expect("resolved body should be valid JSON"),
            json!({"data": "value\r\n\"quoted\""})
        );
        assert!(!body.contains('\r'));
        assert!(!body.contains('\n'));
    }

    #[test]
    fn whole_value_references_keep_their_json_types() {
        let config = json!({
            "bodyFormat": "json",
            "body": "{\"enabled\":\"{{enabled}}\",\"payload\":\"{{payload}}\"}"
        })
        .as_object()
        .expect("config should be an object")
        .clone();
        let variables = BTreeMap::from([
            ("enabled".to_owned(), Value::Bool(true)),
            ("payload".to_owned(), json!({"count": 2})),
        ]);

        let resolved = resolve_http_request_config(&config, &variables)
            .expect("typed JSON variables should resolve");
        let body = resolved.get("body").and_then(Value::as_str).unwrap();

        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({"enabled": true, "payload": {"count": 2}})
        );
    }

    #[test]
    fn missing_format_infers_json_from_content_type() {
        let config = json!({
            "headers": [{"name": "Content-Type", "value": "application/problem+json; charset=utf-8"}],
            "body": "{\"data\":\"{{data}}\"}"
        })
        .as_object()
        .expect("config should be an object")
        .clone();
        let variables = BTreeMap::from([("data".to_owned(), Value::String("line\r".to_owned()))]);

        let resolved = resolve_http_request_config(&config, &variables)
            .expect("legacy JSON body should be inferred");
        let body = resolved.get("body").and_then(Value::as_str).unwrap();

        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({"data": "line\r"})
        );
    }

    #[test]
    fn text_body_preserves_raw_template_rendering() {
        let config = json!({"bodyFormat": "text", "body": "value={{data}}"})
            .as_object()
            .expect("config should be an object")
            .clone();
        let variables = BTreeMap::from([("data".to_owned(), Value::String("line\r".to_owned()))]);

        let resolved =
            resolve_http_request_config(&config, &variables).expect("text body should resolve");

        assert_eq!(
            resolved.get("body"),
            Some(&Value::String("value=line\r".to_owned()))
        );
    }

    #[test]
    fn invalid_json_template_is_rejected_before_dispatch() {
        let config = json!({"bodyFormat": "json", "body": "{invalid}"})
            .as_object()
            .expect("config should be an object")
            .clone();

        let error = resolve_http_request_config(&config, &BTreeMap::new())
            .expect_err("invalid JSON must fail");

        assert!(error.contains("invalid JSON HTTP request body"));
    }
}
