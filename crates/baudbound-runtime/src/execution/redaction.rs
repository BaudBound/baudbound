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
        for value in report.variables.values_mut() {
            self.redact_value(value);
        }
        for log in &mut report.logs {
            log.message = self.redact_text(&log.message);
        }
        report
    }

    fn redact_value(&self, value: &mut Value) {
        if self.secret_values.iter().any(|secret| secret == value) {
            *value = Value::String("[REDACTED]".to_owned());
            return;
        }
        match value {
            Value::String(text) => *text = self.redact_text(text),
            Value::Array(values) => {
                for value in values {
                    self.redact_value(value);
                }
            }
            Value::Object(values) => {
                for value in values.values_mut() {
                    self.redact_value(value);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }

    pub(super) fn redact_text(&self, text: &str) -> String {
        self.secret_values
            .iter()
            .fold(text.to_owned(), |redacted, value| {
                let sensitive = value_to_string(value);
                if sensitive.is_empty() {
                    redacted
                } else {
                    redacted.replace(&sensitive, "[REDACTED]")
                }
            })
    }
}
