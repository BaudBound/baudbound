use std::collections::BTreeSet;

use base64::{Engine, engine::general_purpose};
use serde_json::Value;

use crate::runtime::{DERIVED_VARIABLE_METADATA_SUFFIXES, value_to_string};

use super::{RunReport, RuntimeExecutor};

impl RuntimeExecutor<'_> {
    pub(super) fn has_secrets(&self) -> bool {
        !self.secret_values.is_empty()
    }

    pub(super) fn redact_report(&self, mut report: RunReport) -> RunReport {
        for name in &self.secret_names {
            report.variables.remove(name);
            report.variable_scopes.remove(name);
            for suffix in DERIVED_VARIABLE_METADATA_SUFFIXES {
                report.variables.remove(&format!("{name}{suffix}"));
                report.variable_scopes.remove(&format!("{name}{suffix}"));
            }
        }
        for name in &self.transient_sensitive_variable_names {
            report.variables.remove(name);
            report.variable_scopes.remove(name);
        }
        for value in report.variables.values_mut() {
            self.redact_value(value);
        }
        for log in &mut report.logs {
            log.message = self.redact_text(&log.message);
        }
        report
    }

    fn redact_value(&self, value: &mut Value) {
        redact_value_with_secrets(value, &self.secret_values);
    }

    pub(super) fn value_contains_transient_sensitive(&self, value: &Value) -> bool {
        let mut redacted = value.clone();
        redact_value_with_secrets(&mut redacted, &self.transient_sensitive_values);
        redacted != *value
    }

    pub(super) fn redact_text(&self, text: &str) -> String {
        redact_secret_text(text, &self.secret_values)
    }

    pub(super) fn value_contains_sensitive(&self, value: &Value) -> bool {
        let mut redacted = value.clone();
        self.redact_value(&mut redacted);
        redacted != *value
    }
}

fn redact_value_with_secrets(value: &mut Value, secret_values: &[Value]) {
    if secret_values.iter().any(|secret| secret == value) {
        *value = Value::String("[REDACTED]".to_owned());
        return;
    }
    match value {
        Value::String(text) => *text = redact_secret_text(text, secret_values),
        Value::Array(values) => {
            for value in values {
                redact_value_with_secrets(value, secret_values);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                redact_value_with_secrets(value, secret_values);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn redact_secret_text(text: &str, secret_values: &[Value]) -> String {
    secret_text_variants(secret_values)
        .into_iter()
        .fold(text.to_owned(), |redacted, sensitive| {
            redacted.replace(&sensitive, "[REDACTED]")
        })
}

fn secret_text_variants(secret_values: &[Value]) -> Vec<String> {
    let mut raw_values = BTreeSet::new();
    for value in secret_values {
        collect_secret_text(value, &mut raw_values);
    }

    let mut variants = BTreeSet::new();
    for raw in raw_values {
        if raw.is_empty() {
            continue;
        }
        variants.insert(raw.clone());

        let json = serde_json::to_string(&raw).unwrap_or_default();
        if let Some(escaped) = json
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            && escaped != raw
        {
            variants.insert(escaped.to_owned());
        }

        if raw.len() >= 8 {
            variants.insert(url::form_urlencoded::byte_serialize(raw.as_bytes()).collect());
            variants.insert(percent_encode(raw.as_bytes()));
            variants.insert(general_purpose::STANDARD.encode(raw.as_bytes()));
            variants.insert(general_purpose::STANDARD_NO_PAD.encode(raw.as_bytes()));
            variants.insert(general_purpose::URL_SAFE.encode(raw.as_bytes()));
            variants.insert(general_purpose::URL_SAFE_NO_PAD.encode(raw.as_bytes()));
            variants.insert(
                raw.as_bytes()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
            );
        }
    }

    let mut variants = variants.into_iter().collect::<Vec<_>>();
    variants.sort_by_key(|value| std::cmp::Reverse(value.len()));
    variants
}

fn percent_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len());
    for byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(*byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn collect_secret_text(value: &Value, values: &mut BTreeSet<String>) {
    let rendered = value_to_string(value);
    if !rendered.is_empty() {
        values.insert(rendered);
    }
    match value {
        Value::Array(items) => {
            for item in items {
                collect_secret_text(item, values);
            }
        }
        Value::Object(entries) => {
            for item in entries.values() {
                collect_secret_text(item, values);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::redact_secret_text;
    use serde_json::json;

    #[test]
    fn redacts_encoded_and_escaped_secret_representations() {
        let secret = "api token/with+symbols";
        let text = [
            secret.to_owned(),
            "api%20token%2Fwith%2Bsymbols".to_owned(),
            "YXBpIHRva2VuL3dpdGgrc3ltYm9scw==".to_owned(),
            "61706920746f6b656e2f776974682b73796d626f6c73".to_owned(),
        ]
        .join(" ");

        let redacted = redact_secret_text(&text, &[json!(secret)]);
        assert!(!redacted.contains("token"));
        assert!(!redacted.contains("YXBp"));
        assert!(!redacted.contains("617069"));
    }

    #[test]
    fn redacts_scalar_values_extracted_from_structured_secrets() {
        let redacted = redact_secret_text(
            "password=deep-secret-value",
            &[json!({"credentials": {"password": "deep-secret-value"}})],
        );
        assert_eq!(redacted, "password=[REDACTED]");
    }
}
