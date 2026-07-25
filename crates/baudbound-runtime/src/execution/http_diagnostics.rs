use std::fmt::Write;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use url::{Url, form_urlencoded};

use crate::runtime::config_string;

use super::{RuntimeExecutor, RuntimeNode};

const PREVIEW_MAX_BYTES: usize = 2 * 1024;
const HEADER_COUNT_LIMIT: usize = 128;
const REDACTED: &str = "[REDACTED]";

impl RuntimeExecutor<'_> {
    pub(super) fn log_http_request(&mut self, node: &RuntimeNode, config: &Map<String, Value>) {
        let method = config_string(config, "method").unwrap_or_else(|| "GET".to_owned());
        let url = config_string(config, "url").unwrap_or_default();
        let safe_url = sanitized_url(&url);
        self.push_runtime_log(
            "info",
            format!("Sending HTTP {method} request to {safe_url}."),
            Some(node.id.clone()),
        );

        let headers = headers_from_config(config.get("headers"));
        self.push_runtime_log(
            "debug",
            format!(
                "HTTP request headers: {}",
                self.redact_text(&display_headers(&headers))
            ),
            Some(node.id.clone()),
        );

        let body = config_string(config, "body").unwrap_or_default();
        let content_type = header_value(&headers, "content-type");
        self.push_runtime_log(
            "debug",
            format!(
                "HTTP request body: {}",
                self.redact_text(&body_diagnostic(&body, content_type))
            ),
            Some(node.id.clone()),
        );
    }

    pub(super) fn log_http_response(
        &mut self,
        node: &RuntimeNode,
        config: &Map<String, Value>,
        output: &Map<String, Value>,
    ) {
        let method = config_string(config, "method").unwrap_or_else(|| "GET".to_owned());
        let url = config_string(config, "url").unwrap_or_default();
        let safe_url = sanitized_url(&url);
        let status_code = output
            .get("status_code")
            .and_then(Value::as_u64)
            .map_or_else(|| "unknown".to_owned(), |value| value.to_string());
        let status_text = output
            .get("status_text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let duration_ms = output
            .get("duration_ms")
            .and_then(Value::as_u64)
            .map_or_else(|| "unknown".to_owned(), |value| value.to_string());
        let status = if status_text.is_empty() {
            status_code
        } else {
            format!("{status_code} {status_text}")
        };

        self.push_runtime_log(
            "info",
            format!("HTTP {method} {safe_url} returned {status} in {duration_ms} ms."),
            Some(node.id.clone()),
        );

        let headers = headers_from_output(output.get("headers"));
        self.push_runtime_log(
            "debug",
            format!(
                "HTTP response headers: {}",
                self.redact_text(&display_headers(&headers))
            ),
            Some(node.id.clone()),
        );
        if let Some(body) = output.get("body").and_then(Value::as_str) {
            let content_type = header_value(&headers, "content-type");
            self.push_runtime_log(
                "debug",
                format!(
                    "HTTP response body: {}",
                    self.redact_text(&body_diagnostic(body, content_type))
                ),
                Some(node.id.clone()),
            );
        }
    }
}

fn sanitized_url(value: &str) -> String {
    let Ok(url) = Url::parse(value) else {
        return "[INVALID URL]".to_owned();
    };
    let mut output = String::new();
    output.push_str(url.scheme());
    output.push_str("://");
    if let Some(host) = url.host_str() {
        output.push_str(host);
    }
    if let Some(port) = url.port() {
        let _ = write!(output, ":{port}");
    }
    output.push_str(url.path());
    let query_names = url
        .query_pairs()
        .map(|(name, _)| format!("{}={REDACTED}", escape_controls(&name)))
        .collect::<Vec<_>>();
    if !query_names.is_empty() {
        output.push('?');
        output.push_str(&query_names.join("&"));
    }
    output
}

fn body_diagnostic(body: &str, content_type: Option<&str>) -> String {
    let bytes = body.as_bytes();
    let hash = hex_sha256(bytes);
    let content_type = content_type
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let preview = body_preview(body, content_type);
    let content_type = content_type.unwrap_or("unknown");
    format!(
        "{} bytes, sha256 {hash}, content type {content_type}, preview: {preview}",
        bytes.len()
    )
}

fn body_preview(body: &str, content_type: Option<&str>) -> String {
    if body.is_empty() {
        return "[EMPTY]".to_owned();
    }
    if is_json_content_type(content_type)
        && let Ok(mut value) = serde_json::from_str::<Value>(body)
    {
        redact_structured_value(&mut value);
        return bounded_preview(&escape_controls(&value.to_string()));
    }
    if is_form_content_type(content_type) {
        let values = form_urlencoded::parse(body.as_bytes())
            .map(|(name, value)| {
                let value = if is_sensitive_name(&name) {
                    REDACTED.to_owned()
                } else {
                    escape_controls(&value)
                };
                format!("{}={value}", escape_controls(&name))
            })
            .collect::<Vec<_>>();
        return bounded_preview(&values.join("&"));
    }
    if is_binary_content_type(content_type) {
        return "[BINARY CONTENT OMITTED]".to_owned();
    }
    bounded_preview(&escape_controls(body))
}

fn redact_structured_value(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                redact_structured_value(value);
            }
        }
        Value::Object(values) => {
            for (name, value) in values {
                if is_sensitive_name(name) {
                    *value = Value::String(REDACTED.to_owned());
                } else {
                    redact_structured_value(value);
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn is_sensitive_name(name: &str) -> bool {
    let normalized = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "authorization"
            | "cookie"
            | "credential"
            | "credentials"
            | "passphrase"
            | "password"
            | "privatekey"
            | "proxyauthorization"
            | "setcookie"
    ) || normalized.ends_with("apikey")
        || normalized.ends_with("secret")
        || normalized.ends_with("token")
}

fn is_json_content_type(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|value| {
        value.eq_ignore_ascii_case("application/json")
            || value
                .to_ascii_lowercase()
                .strip_prefix("application/")
                .is_some_and(|subtype| subtype.ends_with("+json"))
    })
}

fn is_form_content_type(content_type: Option<&str>) -> bool {
    content_type
        .is_some_and(|value| value.eq_ignore_ascii_case("application/x-www-form-urlencoded"))
}

fn is_binary_content_type(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|value| {
        let value = value.to_ascii_lowercase();
        value.starts_with("audio/")
            || value.starts_with("font/")
            || value.starts_with("image/")
            || value.starts_with("video/")
            || value == "application/octet-stream"
            || value.starts_with("multipart/")
    })
}

fn bounded_preview(value: &str) -> String {
    if value.len() <= PREVIEW_MAX_BYTES {
        return value.to_owned();
    }
    let mut end = PREVIEW_MAX_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{} [TRUNCATED: preview limited to {PREVIEW_MAX_BYTES} bytes]",
        &value[..end]
    )
}

fn escape_controls(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\n' => "\\n".to_owned(),
            '\r' => "\\r".to_owned(),
            '\t' => "\\t".to_owned(),
            character if character.is_control() => format!("\\u{{{:x}}}", character as u32),
            character => character.to_string(),
        })
        .collect()
}

fn hex_sha256(value: &[u8]) -> String {
    let hash = Sha256::digest(value);
    hash.iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        })
}

fn headers_from_config(headers: Option<&Value>) -> Vec<(String, String)> {
    let mut values = Vec::new();
    match headers {
        Some(Value::Array(rows)) => {
            for row in rows {
                let Some(row) = row.as_object() else {
                    continue;
                };
                let name = row.get("name").and_then(Value::as_str).unwrap_or_default();
                let value = row.get("value").and_then(Value::as_str).unwrap_or_default();
                push_header(&mut values, name, value);
            }
        }
        Some(Value::Object(headers)) => {
            for (name, value) in headers {
                push_header(&mut values, name, value.as_str().unwrap_or_default());
            }
        }
        _ => {}
    }
    values
}

fn push_header(output: &mut Vec<(String, String)>, name: &str, value: &str) {
    if output.len() >= HEADER_COUNT_LIMIT || name.trim().is_empty() {
        return;
    }
    output.push((name.trim().to_owned(), value.trim().to_owned()));
}

fn headers_from_output(headers: Option<&Value>) -> Vec<(String, String)> {
    let Some(Value::Object(headers)) = headers else {
        return Vec::new();
    };
    headers
        .iter()
        .take(HEADER_COUNT_LIMIT)
        .filter_map(|(name, value)| {
            value
                .as_str()
                .map(|value| (name.to_owned(), value.to_owned()))
        })
        .collect()
}

fn display_headers(headers: &[(String, String)]) -> String {
    if headers.is_empty() {
        return "none".to_owned();
    }
    let displayed = headers
        .iter()
        .map(|(name, value)| {
            let value = if is_sensitive_name(name) {
                REDACTED.to_owned()
            } else {
                bounded_preview(&escape_controls(value))
            };
            format!("{}: {value}", escape_controls(name))
        })
        .collect::<Vec<_>>()
        .join(", ");
    bounded_preview(&displayed)
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header, _)| header.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn sanitizes_urls_headers_and_control_characters() {
        assert_eq!(
            sanitized_url("https://user:password@example.com:8443/test?token=secret&name=value"),
            "https://example.com:8443/test?token=[REDACTED]&name=[REDACTED]"
        );
        assert_eq!(
            display_headers(&[
                ("Content-Type".to_owned(), "application/json".to_owned()),
                ("Authorization".to_owned(), "Bearer secret".to_owned()),
                ("X-Trace".to_owned(), "line\r\nnext".to_owned()),
            ]),
            "Content-Type: application/json, Authorization: [REDACTED], X-Trace: line\\r\\nnext"
        );
    }

    #[test]
    fn redacts_nested_json_and_form_values() {
        let json_preview = body_preview(
            r#"{"user":"test","password":"hidden","nested":{"api_key":"private"},"items":[{"accessToken":"token"}]}"#,
            Some("application/json"),
        );
        assert!(json_preview.contains(r#""user":"test""#));
        assert!(!json_preview.contains("hidden"));
        assert!(!json_preview.contains("private"));
        assert!(!json_preview.contains(r#""accessToken":"token""#));
        assert_eq!(
            body_preview(
                "username=test&password=hidden&api-key=private",
                Some("application/x-www-form-urlencoded")
            ),
            "username=test&password=[REDACTED]&api-key=[REDACTED]"
        );
    }

    #[test]
    fn bounds_previews_and_omits_binary_content() {
        let preview = body_preview(&"x".repeat(PREVIEW_MAX_BYTES + 100), Some("text/plain"));
        assert!(preview.contains("[TRUNCATED:"));
        assert_eq!(
            body_preview("not-really-an-image", Some("image/png")),
            "[BINARY CONTENT OMITTED]"
        );
        assert_eq!(
            body_diagnostic("", Some("text/plain")),
            format!(
                "0 bytes, sha256 {}, content type text/plain, preview: [EMPTY]",
                hex_sha256(b"")
            )
        );
    }

    #[test]
    fn recursively_redacts_common_sensitive_key_spellings() {
        let mut value = json!({
            "Password": "one",
            "api-key": "two",
            "refresh_token": "three",
            "safe": [{"privateKey": "four"}]
        });
        redact_structured_value(&mut value);
        let serialized = value.to_string();
        for secret in ["one", "two", "three", "four"] {
            assert!(!serialized.contains(secret));
        }
    }
}
