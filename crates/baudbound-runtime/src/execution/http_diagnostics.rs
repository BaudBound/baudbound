use serde_json::{Map, Value};

use crate::runtime::config_string;

use super::{RuntimeExecutor, RuntimeNode};

impl RuntimeExecutor<'_> {
    pub(super) fn log_http_request(&mut self, node: &RuntimeNode, config: &Map<String, Value>) {
        let method = config_string(config, "method").unwrap_or_else(|| "GET".to_owned());
        let url = config_string(config, "url").unwrap_or_default();
        self.push_runtime_log(
            "info",
            format!("Sending HTTP {method} request to {url}."),
            Some(node.id.clone()),
        );

        let headers = display_headers(config.get("headers"));
        self.push_runtime_log(
            "debug",
            format!("HTTP request headers: {headers}"),
            Some(node.id.clone()),
        );

        let body = config_string(config, "body").unwrap_or_default();
        self.push_runtime_log(
            "debug",
            format!("HTTP request body ({} bytes): {body}", body.len(),),
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
            format!("HTTP {method} {url} returned {status} in {duration_ms} ms."),
            Some(node.id.clone()),
        );

        if let Some(body) = output.get("body").and_then(Value::as_str) {
            self.push_runtime_log(
                "debug",
                format!("HTTP response body ({} bytes): {body}", body.len(),),
                Some(node.id.clone()),
            );
        }
    }
}

fn display_headers(headers: Option<&Value>) -> String {
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
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(", ")
    }
}

fn push_header(output: &mut Vec<String>, name: &str, value: &str) {
    if name.trim().is_empty() {
        return;
    }
    let value = if is_sensitive_header(name) {
        "[REDACTED]".to_owned()
    } else {
        value.to_owned()
    };
    output.push(format!("{name}: {value}"));
}

fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "authorization" | "cookie" | "proxy-authorization" | "set-cookie" | "x-api-key" | "api-key"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn displays_headers_and_redacts_credentials() {
        let headers = json!([
            {"name": "Content-Type", "value": "application/json"},
            {"name": "Authorization", "value": "Bearer secret"}
        ]);

        assert_eq!(
            display_headers(Some(&headers)),
            "Content-Type: application/json, Authorization: [REDACTED]"
        );
    }
}
