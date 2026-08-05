use std::io::Write;

use baudbound_storage::{
    NewSystemLog, PaginatedRecords, StoredSystemLog, SystemLogQuery, SystemLogSummary,
};
use chrono::Utc;
use tauri::State;

use super::{
    DesktopUiState,
    history::{ExportResult, cancelled_export, string_error, successful_export, write_atomic_with},
};

const REDACTED: &str = "[redacted]";

#[tauri::command]
pub(super) fn record_system_log(
    log: NewSystemLog,
    state: State<'_, DesktopUiState>,
) -> Result<StoredSystemLog, String> {
    state
        .store
        .append_system_log(sanitize_log(log))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) fn query_system_logs(
    query: SystemLogQuery,
    state: State<'_, DesktopUiState>,
) -> Result<PaginatedRecords<StoredSystemLog>, String> {
    state
        .store
        .query_system_logs(&query)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) fn get_system_log(
    id: String,
    state: State<'_, DesktopUiState>,
) -> Result<Option<StoredSystemLog>, String> {
    state
        .store
        .find_system_log(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) fn system_log_summary(
    state: State<'_, DesktopUiState>,
) -> Result<SystemLogSummary, String> {
    state
        .store
        .system_log_summary()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) fn mark_system_logs_read(
    state: State<'_, DesktopUiState>,
) -> Result<SystemLogSummary, String> {
    state
        .store
        .mark_system_logs_read()
        .map_err(|error| error.to_string())?;
    state
        .store
        .system_log_summary()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) fn clear_system_logs(state: State<'_, DesktopUiState>) -> Result<usize, String> {
    state
        .store
        .clear_system_logs()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) async fn export_system_logs<R: tauri::Runtime>(
    query: SystemLogQuery,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ExportResult, String> {
    let mut page_query = query;
    page_query.limit = 200;
    page_query.offset = 0;
    let first_page = state
        .store
        .query_system_logs(&page_query)
        .map_err(|error| error.to_string())?;
    if first_page.total == 0 {
        return Err("there are no system logs matching the current filters".to_owned());
    }

    let default_name = format!(
        "baudbound-system-logs-{}.json",
        Utc::now().format("%Y%m%d-%H%M%S")
    );
    let selected_path = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .add_filter("JSON file", &["json"])
        .set_file_name(&default_name)
        .save_file()
        .await;
    let Some(selected_path) = selected_path else {
        return Ok(cancelled_export());
    };
    let path = selected_path.path();
    let exported_count = first_page.total;
    write_system_log_export(
        path,
        &state.store,
        &page_query,
        first_page,
        state.store.schema_version().ok(),
    )?;
    Ok(successful_export(path, exported_count))
}

fn write_system_log_export(
    path: &std::path::Path,
    store: &baudbound_storage::SqliteRunnerStore,
    query: &SystemLogQuery,
    first_page: PaginatedRecords<StoredSystemLog>,
    schema_version: Option<i64>,
) -> Result<(), String> {
    write_atomic_with(path, |output| {
        let mut header = serde_json::to_vec(&serde_json::json!({
            "format": "baudbound.system-logs",
            "format_version": 1,
            "exported_at": Utc::now().to_rfc3339(),
            "runner": {
                "version": env!("CARGO_PKG_VERSION"),
                "operating_system": std::env::consts::OS,
                "architecture": std::env::consts::ARCH,
                "storage_schema_version": schema_version,
            },
            "query": query,
        }))
        .map_err(string_error)?;
        header.pop();
        output.write_all(&header).map_err(string_error)?;
        output.write_all(b",\"logs\":[").map_err(string_error)?;

        let total = first_page.total;
        let mut offset = 0;
        let mut page = first_page;
        let mut first_record = true;
        loop {
            for log in &page.items {
                if !first_record {
                    output.write_all(b",").map_err(string_error)?;
                }
                serde_json::to_writer(&mut *output, log).map_err(string_error)?;
                first_record = false;
            }
            offset += page.items.len();
            if offset >= total || page.items.is_empty() {
                break;
            }
            let mut next_query = query.clone();
            next_query.limit = 200;
            next_query.offset = offset;
            page = store
                .query_system_logs(&next_query)
                .map_err(|error| error.to_string())?;
        }
        output.write_all(b"]}").map_err(string_error)
    })
}

fn sanitize_log(mut log: NewSystemLog) -> NewSystemLog {
    log.source = redact_embedded_credentials(&log.source);
    log.title = redact_embedded_credentials(&log.title);
    log.message = redact_embedded_credentials(&log.message);
    for detail in &mut log.details {
        if sensitive_label(&detail.label) {
            detail.value = REDACTED.to_owned();
        } else {
            detail.value = redact_embedded_credentials(&detail.value);
        }
    }
    log
}

fn sensitive_label(label: &str) -> bool {
    let normalized = label
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    [
        "authorization",
        "cookie",
        "credential",
        "password",
        "privatekey",
        "secret",
        "token",
    ]
    .iter()
    .any(|sensitive| normalized.contains(sensitive))
}

fn redact_embedded_credentials(value: &str) -> String {
    let mut redacted = value.to_owned();
    for marker in ["authorization:", "cookie:", "set-cookie:"] {
        redacted = redact_line_value_after_marker(&redacted, marker);
    }
    for marker in [
        "access_key=",
        "bearer ",
        "api_key=",
        "apikey=",
        "credential=",
        "key=",
        "password=",
        "secret=",
        "sig=",
        "signature=",
        "token=",
    ] {
        redacted = redact_values_after_marker(&redacted, marker);
    }
    redacted
}

fn redact_line_value_after_marker(value: &str, marker: &str) -> String {
    let mut output = value.to_owned();
    let mut search_from = 0;
    loop {
        let lowercase = output.to_ascii_lowercase();
        let Some(relative_start) = lowercase[search_from..].find(marker) else {
            break;
        };
        let value_start = search_from + relative_start + marker.len();
        let value_end = output[value_start..]
            .find(['\r', '\n'])
            .map(|relative_end| value_start + relative_end)
            .unwrap_or(output.len());
        output.replace_range(value_start..value_end, REDACTED);
        search_from = value_start + REDACTED.len();
    }
    output
}

fn redact_values_after_marker(value: &str, marker: &str) -> String {
    let mut output = value.to_owned();
    let mut search_from = 0;
    loop {
        let lowercase = output.to_ascii_lowercase();
        let Some(relative_start) = lowercase[search_from..].find(marker) else {
            break;
        };
        let value_start = search_from + relative_start + marker.len();
        let value_end = output[value_start..]
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '&' | ',' | ';' | '"' | '\'')
            })
            .map(|relative_end| value_start + relative_end)
            .unwrap_or(output.len());
        if value_start == value_end {
            search_from = value_start;
            if search_from >= output.len() {
                break;
            }
            continue;
        }
        output.replace_range(value_start..value_end, REDACTED);
        search_from = value_start + REDACTED.len();
    }
    output
}

#[cfg(test)]
mod tests {
    use baudbound_storage::{NewSystemLog, SystemLogDetail, SystemLogSeverity};

    use super::*;

    #[test]
    fn sanitizes_sensitive_detail_fields_and_embedded_credentials() {
        let log = sanitize_log(NewSystemLog {
            details: vec![
                SystemLogDetail {
                    label: "Access token".to_owned(),
                    value: "private-token".to_owned(),
                },
                SystemLogDetail {
                    label: "Request".to_owned(),
                    value: "Authorization: Basic abc123\nstatus failed".to_owned(),
                },
            ],
            message: "request failed with token=abc123".to_owned(),
            severity: SystemLogSeverity::Error,
            source: "Network".to_owned(),
            title: "Request failed".to_owned(),
        });

        assert_eq!(log.details[0].value, REDACTED);
        assert!(!log.details[1].value.contains("abc123"));
        assert!(!log.message.contains("abc123"));
        assert!(log.message.contains(REDACTED));
    }
}
