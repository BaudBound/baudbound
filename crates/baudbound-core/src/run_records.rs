use std::{
    collections::BTreeMap,
    fmt::Write,
    time::{SystemTime, UNIX_EPOCH},
};

use baudbound_runtime::{RuntimeOutputLimits, unix_timestamp_millis_now};
use baudbound_script::ScriptPackage;
use baudbound_storage::{RunLogEntry, ScriptStore, StorageError, StoredRunRecord};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::RunReport;

pub(crate) fn stored_run_record_from_report(
    report: &RunReport,
    limits: RuntimeOutputLimits,
) -> StoredRunRecord {
    let variables = report
        .variables
        .iter()
        .map(|(name, value)| {
            (
                name.clone(),
                retained_variable_value(value, limits.max_retained_variable_bytes),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut logs = report
        .logs
        .iter()
        .map(|log| RunLogEntry {
            action_type: log.action_type.clone(),
            level: log.level.clone(),
            message: truncate_utf8(&log.message, limits.max_log_entry_bytes, "log entry"),
            node_id: log.node_id.clone(),
            timestamp_unix_ms: log.timestamp_unix_ms,
        })
        .collect::<Vec<_>>();
    enforce_log_limit(&mut logs, limits);
    let mut record = StoredRunRecord {
        completed_at_unix: current_unix_timestamp(),
        logs,
        run_id: report.identity.run_id.clone(),
        script_id: report.identity.script_id.clone(),
        status: "completed".to_owned(),
        trigger_node_id: report.identity.trigger_node_id.clone(),
        variable_scopes: report
            .variable_scopes
            .iter()
            .filter(|(name, _)| variables.contains_key(*name))
            .map(|(name, scope)| (name.clone(), scope.as_str().to_owned()))
            .collect(),
        variables,
    };
    enforce_record_limit(&mut record, limits);
    record
}

pub(crate) fn append_failed_run_record(
    store: &impl ScriptStore,
    package: &ScriptPackage,
    selected_trigger_node_id: Option<&str>,
    message: String,
    limits: RuntimeOutputLimits,
) -> Result<(), StorageError> {
    store.append_run_record(failed_run_record(
        package,
        selected_trigger_node_id,
        message,
        limits,
    ))
}

pub(crate) fn append_cancelled_run_record(
    store: &impl ScriptStore,
    package: &ScriptPackage,
    selected_trigger_node_id: Option<&str>,
    limits: RuntimeOutputLimits,
) -> Result<(), StorageError> {
    store.append_run_record(terminal_run_record(
        package,
        selected_trigger_node_id,
        "cancelled",
        "warning",
        "Runtime execution was cancelled.".to_owned(),
        limits,
    ))
}

pub(crate) fn failed_run_record(
    package: &ScriptPackage,
    selected_trigger_node_id: Option<&str>,
    message: String,
    limits: RuntimeOutputLimits,
) -> StoredRunRecord {
    terminal_run_record(
        package,
        selected_trigger_node_id,
        "failed",
        "error",
        message,
        limits,
    )
}

fn terminal_run_record(
    package: &ScriptPackage,
    selected_trigger_node_id: Option<&str>,
    status: &str,
    level: &str,
    message: String,
    limits: RuntimeOutputLimits,
) -> StoredRunRecord {
    let trigger_node_id = selected_trigger_node_id
        .map(ToOwned::to_owned)
        .or_else(|| trigger_node_id(&package.program))
        .unwrap_or_else(|| "unknown".to_owned());
    let mut record = StoredRunRecord {
        completed_at_unix: current_unix_timestamp(),
        logs: vec![RunLogEntry {
            action_type: None,
            level: level.to_owned(),
            message: truncate_utf8(&message, limits.max_log_entry_bytes, "log entry"),
            node_id: None,
            timestamp_unix_ms: unix_timestamp_millis_now(),
        }],
        run_id: create_run_id(&package.manifest.id, &trigger_node_id),
        script_id: package.manifest.id.clone(),
        status: status.to_owned(),
        trigger_node_id,
        variable_scopes: Default::default(),
        variables: Default::default(),
    };
    enforce_record_limit(&mut record, limits);
    record
}

fn retained_variable_value(value: &Value, max_bytes: usize) -> Value {
    let mut value = value.clone();
    redact_sensitive_fields(&mut value);
    let serialized = serde_json::to_vec(&value).unwrap_or_default();
    if serialized.len() <= max_bytes {
        return value;
    }
    let preview = bounded_preview(&escape_controls(&String::from_utf8_lossy(&serialized)), 512);
    json!({
        "baudbound_retention": "truncated",
        "original_bytes": serialized.len(),
        "sha256": hex_sha256(&serialized),
        "preview": preview,
    })
}

fn redact_sensitive_fields(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                redact_sensitive_fields(value);
            }
        }
        Value::Object(values) => {
            for (name, value) in values {
                if is_sensitive_name(name) {
                    *value = Value::String("[REDACTED]".to_owned());
                } else {
                    redact_sensitive_fields(value);
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

fn enforce_record_limit(record: &mut StoredRunRecord, limits: RuntimeOutputLimits) {
    let original_logs = record.logs.len();
    let original_variables = record.variables.len();
    while serialized_record_bytes(record) > limits.max_run_record_bytes {
        if !record.logs.is_empty() {
            record.logs.remove(0);
        } else if let Some((name, _)) = record.variables.pop_last() {
            record.variable_scopes.remove(&name);
        } else {
            break;
        }
    }
    let removed_logs = original_logs.saturating_sub(record.logs.len());
    let removed_variables = original_variables.saturating_sub(record.variables.len());
    if removed_logs == 0 && removed_variables == 0 {
        return;
    }
    let marker = RunLogEntry {
        action_type: None,
        level: "warning".to_owned(),
        message: truncate_utf8(
            &format!(
                "Stored run data was truncated to the configured {} byte record limit. Omitted {removed_logs} log entries and {removed_variables} variable values.",
                limits.max_run_record_bytes
            ),
            limits.max_log_entry_bytes,
            "log entry",
        ),
        node_id: None,
        timestamp_unix_ms: unix_timestamp_millis_now(),
    };
    record.logs.push(marker);
    while serialized_record_bytes(record) > limits.max_run_record_bytes {
        if record.logs.len() > 1 {
            record.logs.remove(0);
        } else if let Some((name, _)) = record.variables.pop_last() {
            record.variable_scopes.remove(&name);
        } else {
            break;
        }
    }
}

fn enforce_log_limit(logs: &mut Vec<RunLogEntry>, limits: RuntimeOutputLimits) {
    let original_count = logs.len();
    while serialized_log_bytes(logs) > limits.max_run_log_bytes && !logs.is_empty() {
        logs.remove(0);
    }
    let removed = original_count.saturating_sub(logs.len());
    if removed == 0 {
        return;
    }
    logs.insert(
        0,
        RunLogEntry {
            action_type: None,
            level: "warning".to_owned(),
            message: truncate_utf8(
                &format!(
                    "Stored run logs were truncated to the configured {} byte limit. Omitted {removed} earlier log entries.",
                    limits.max_run_log_bytes
                ),
                limits.max_log_entry_bytes,
                "log entry",
            ),
            node_id: None,
            timestamp_unix_ms: unix_timestamp_millis_now(),
        },
    );
    while serialized_log_bytes(logs) > limits.max_run_log_bytes && logs.len() > 1 {
        logs.remove(1);
    }
}

fn serialized_log_bytes(logs: &[RunLogEntry]) -> usize {
    serde_json::to_vec(logs).map_or(usize::MAX, |value| value.len())
}

fn serialized_record_bytes(record: &StoredRunRecord) -> usize {
    serde_json::to_vec(record).map_or(usize::MAX, |value| value.len())
}

fn bounded_preview(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{} [TRUNCATED PREVIEW]", &value[..end])
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

fn truncate_utf8(value: &str, max_bytes: usize, label: &str) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let marker = format!(" [TRUNCATED: {label} exceeded {max_bytes} bytes]");
    if marker.len() >= max_bytes {
        return marker.chars().take(max_bytes).collect();
    }
    let mut end = max_bytes - marker.len();
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{}", &value[..end], marker)
}

fn hex_sha256(value: &[u8]) -> String {
    let hash = Sha256::digest(value);
    hash.iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        })
}

fn trigger_node_id(program: &serde_json::Value) -> Option<String> {
    program
        .get("entry")?
        .get("trigger")?
        .get("id")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn create_run_id(script_id: &str, trigger_node_id: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{script_id}:{trigger_node_id}:{timestamp}")
}

#[cfg(test)]
mod tests {
    use baudbound_runtime::{RunIdentity, RunVariableScope, RuntimeLogEntry};

    use super::*;

    #[test]
    fn retained_values_redact_sensitive_fields_and_report_truncation() {
        let retained = retained_variable_value(
            &json!({
                "username": "safe",
                "password": "private",
                "nested": {"access_token": "token"},
                "large": "x".repeat(1024),
            }),
            128,
        );
        let serialized = retained.to_string();
        assert!(!serialized.contains("private"));
        assert!(!serialized.contains(r#""token""#));
        assert_eq!(retained["baudbound_retention"], "truncated");
        assert!(
            retained["sha256"]
                .as_str()
                .is_some_and(|hash| hash.len() == 64)
        );
    }

    #[test]
    fn stored_records_respect_log_variable_and_record_limits() {
        let report = RunReport {
            identity: RunIdentity {
                run_id: "run-1".to_owned(),
                script_id: "script-1".to_owned(),
                trigger_node_id: "trigger-1".to_owned(),
            },
            logs: (0..20)
                .map(|_| RuntimeLogEntry {
                    action_type: Some("action.log".to_owned()),
                    level: "info".to_owned(),
                    message: "x".repeat(200),
                    node_id: Some("node-1".to_owned()),
                    timestamp_unix_ms: 1,
                })
                .collect(),
            variable_scopes: BTreeMap::from([("value".to_owned(), RunVariableScope::NodeOutput)]),
            variables: BTreeMap::from([("value".to_owned(), json!("y".repeat(2000)))]),
        };
        let limits = RuntimeOutputLimits {
            max_log_entry_bytes: 128,
            max_runtime_variable_bytes: baudbound_runtime::ResourceLimit::limited(4096),
            max_retained_variable_bytes: 256,
            max_run_log_bytes: 1024,
            max_run_record_bytes: 2048,
        };

        let record = stored_run_record_from_report(&report, limits);

        assert!(serialized_record_bytes(&record) <= limits.max_run_record_bytes);
        assert!(record.logs.iter().all(|log| log.message.len() <= 128));
        assert_eq!(
            record.variables["value"]["baudbound_retention"],
            "truncated"
        );
    }
}
