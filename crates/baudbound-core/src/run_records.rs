use std::time::{SystemTime, UNIX_EPOCH};

use baudbound_runtime::unix_timestamp_millis_now;
use baudbound_script::ScriptPackage;
use baudbound_storage::{RunLogEntry, ScriptStore, StorageError, StoredRunRecord};

use crate::RunReport;

pub(crate) fn stored_run_record_from_report(report: &RunReport) -> StoredRunRecord {
    StoredRunRecord {
        completed_at_unix: current_unix_timestamp(),
        logs: report
            .logs
            .iter()
            .map(|log| RunLogEntry {
                action_type: log.action_type.clone(),
                level: log.level.clone(),
                message: log.message.clone(),
                node_id: log.node_id.clone(),
                timestamp_unix_ms: log.timestamp_unix_ms,
            })
            .collect(),
        run_id: report.identity.run_id.clone(),
        script_id: report.identity.script_id.clone(),
        status: "completed".to_owned(),
        trigger_node_id: report.identity.trigger_node_id.clone(),
        variables: report.variables.clone(),
    }
}

pub(crate) fn append_failed_run_record(
    store: &impl ScriptStore,
    package: &ScriptPackage,
    selected_trigger_node_id: Option<&str>,
    message: String,
) -> Result<(), StorageError> {
    store.append_run_record(failed_run_record(
        package,
        selected_trigger_node_id,
        message,
    ))
}

pub(crate) fn append_cancelled_run_record(
    store: &impl ScriptStore,
    package: &ScriptPackage,
    selected_trigger_node_id: Option<&str>,
) -> Result<(), StorageError> {
    store.append_run_record(terminal_run_record(
        package,
        selected_trigger_node_id,
        "cancelled",
        "warning",
        "Runtime execution was cancelled.".to_owned(),
    ))
}

pub(crate) fn failed_run_record(
    package: &ScriptPackage,
    selected_trigger_node_id: Option<&str>,
    message: String,
) -> StoredRunRecord {
    terminal_run_record(
        package,
        selected_trigger_node_id,
        "failed",
        "error",
        message,
    )
}

fn terminal_run_record(
    package: &ScriptPackage,
    selected_trigger_node_id: Option<&str>,
    status: &str,
    level: &str,
    message: String,
) -> StoredRunRecord {
    let trigger_node_id = selected_trigger_node_id
        .map(ToOwned::to_owned)
        .or_else(|| trigger_node_id(&package.program))
        .unwrap_or_else(|| "unknown".to_owned());
    StoredRunRecord {
        completed_at_unix: current_unix_timestamp(),
        logs: vec![RunLogEntry {
            action_type: None,
            level: level.to_owned(),
            message,
            node_id: None,
            timestamp_unix_ms: unix_timestamp_millis_now(),
        }],
        run_id: create_run_id(&package.manifest.id, &trigger_node_id),
        script_id: package.manifest.id.clone(),
        status: status.to_owned(),
        trigger_node_id,
        variables: Default::default(),
    }
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
