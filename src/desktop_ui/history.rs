use std::{collections::BTreeMap, io::Write};

use baudbound_script::load_script_package;
use baudbound_storage::{
    InstalledScript, PaginatedRecords, RunHistoryQuery, RunLogQuery, ScriptStore,
    StoredRunLogRecord, StoredRunRecord, StoredVariableRecord,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;
use tempfile::NamedTempFile;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use super::DesktopUiState;

#[derive(Serialize)]
pub(super) struct VariableInventory {
    declared: Vec<DeclaredVariable>,
    script_names: BTreeMap<String, String>,
    stored: Vec<StoredVariableRecord>,
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct DeclaredVariable {
    description: String,
    name: String,
    scope: String,
    script_id: String,
    script_name: String,
    value: serde_json::Value,
    value_type: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LogExportFormat {
    Csv,
    Json,
}

#[derive(Serialize)]
pub(super) struct ExportResult {
    cancelled: bool,
    exported_count: usize,
    file_name: Option<String>,
}

#[tauri::command]
pub(super) fn query_runs(
    query: RunHistoryQuery,
    state: State<'_, DesktopUiState>,
) -> Result<PaginatedRecords<StoredRunRecord>, String> {
    state
        .store
        .query_run_history(&query)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) fn query_logs(
    query: RunLogQuery,
    state: State<'_, DesktopUiState>,
) -> Result<PaginatedRecords<StoredRunLogRecord>, String> {
    state
        .store
        .query_run_logs(&query)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) fn variable_inventory(
    state: State<'_, DesktopUiState>,
) -> Result<VariableInventory, String> {
    collect_variable_inventory(&state)
}

fn collect_variable_inventory(state: &DesktopUiState) -> Result<VariableInventory, String> {
    let stored = state
        .store
        .list_stored_variables()
        .map_err(|error| error.to_string())?;
    let mut declared = Vec::new();
    let mut script_names = BTreeMap::new();
    let mut warnings = Vec::new();
    for script in state
        .store
        .list_scripts()
        .map_err(|error| error.to_string())?
    {
        script_names.insert(script.id.clone(), script.name.clone());
        let package = match load_script_package(&script.package_path) {
            Ok(package) => package,
            Err(error) => {
                warnings.push(format!(
                    "Could not read variable declarations for {}: {error}",
                    script.name
                ));
                continue;
            }
        };
        declared.extend(
            package
                .manifest
                .variables
                .into_iter()
                .map(|variable| DeclaredVariable {
                    description: variable.description,
                    name: variable.name,
                    scope: variable.scope,
                    script_id: script.id.clone(),
                    script_name: script.name.clone(),
                    value: variable.value,
                    value_type: variable.value_type,
                }),
        );
    }
    declared.sort_by(|left, right| {
        left.script_name
            .cmp(&right.script_name)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(VariableInventory {
        declared,
        script_names,
        stored,
        warnings,
    })
}

#[tauri::command]
pub(super) async fn export_variables<R: tauri::Runtime>(
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ExportResult, String> {
    let inventory = collect_variable_inventory(&state)?;
    let exported_count = inventory.stored.len() + inventory.declared.len();
    let default_name = format!(
        "baudbound-variables-{}.json",
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
    let document = serde_json::json!({
        "format": "baudbound.variables",
        "format_version": 1,
        "exported_at": Utc::now().to_rfc3339(),
        "runner": {
            "version": env!("CARGO_PKG_VERSION"),
            "operating_system": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
            "storage_schema_version": state.store.schema_version().ok(),
        },
        "variables": inventory,
    });
    write_atomic(
        path,
        &serde_json::to_vec_pretty(&document).map_err(string_error)?,
    )?;
    Ok(successful_export(path, exported_count))
}

#[tauri::command]
pub(super) async fn export_runs<R: tauri::Runtime>(
    run_ids: Vec<String>,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ExportResult, String> {
    if run_ids.is_empty() {
        return Err("select at least one run to export".to_owned());
    }
    let mut seen = std::collections::HashSet::new();
    let run_ids = run_ids
        .into_iter()
        .filter(|run_id| seen.insert(run_id.clone()))
        .collect::<Vec<_>>();
    let scripts = state
        .store
        .list_scripts()
        .map_err(|error| error.to_string())?;
    let multiple = run_ids.len() > 1;
    let extension = if multiple { "zip" } else { "json" };
    let default_name = if multiple {
        format!("baudbound-runs-{}.zip", Utc::now().format("%Y%m%d-%H%M%S"))
    } else {
        format!("baudbound-run-{}.json", safe_file_component(&run_ids[0]))
    };
    let selected_path = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .add_filter(
            if multiple { "ZIP archive" } else { "JSON file" },
            &[extension],
        )
        .set_file_name(&default_name)
        .save_file()
        .await;
    let Some(selected_path) = selected_path else {
        return Ok(cancelled_export());
    };
    let path = selected_path.path();
    let schema_version = state.store.schema_version().ok();
    if multiple {
        write_run_archive(path, &state.store, &scripts, &run_ids, schema_version)?;
    } else {
        let run = state
            .store
            .find_run_record_by_id(&run_ids[0])
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("run {} is no longer available", run_ids[0]))?;
        let document = run_export_document(&run, &scripts, schema_version);
        write_atomic(
            path,
            &serde_json::to_vec_pretty(&document).map_err(string_error)?,
        )?;
    }
    Ok(successful_export(path, run_ids.len()))
}

#[tauri::command]
pub(super) async fn export_logs<R: tauri::Runtime>(
    format: LogExportFormat,
    query: RunLogQuery,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ExportResult, String> {
    let mut page_query = query;
    page_query.limit = 200;
    page_query.offset = 0;
    let first_page = state
        .store
        .query_run_logs(&page_query)
        .map_err(|error| error.to_string())?;
    if first_page.total == 0 {
        return Err("there are no logs matching the current filters".to_owned());
    }
    let (extension, label) = match format {
        LogExportFormat::Csv => ("csv", "CSV file"),
        LogExportFormat::Json => ("json", "JSON file"),
    };
    let default_name = format!(
        "baudbound-logs-{}.{}",
        Utc::now().format("%Y%m%d-%H%M%S"),
        extension
    );
    let selected_path = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .add_filter(label, &[extension])
        .set_file_name(&default_name)
        .save_file()
        .await;
    let Some(selected_path) = selected_path else {
        return Ok(cancelled_export());
    };
    let path = selected_path.path();
    let exported_count = first_page.total;
    write_log_export(
        path,
        format,
        &state.store,
        &page_query,
        first_page,
        state.store.schema_version().ok(),
    )?;
    Ok(successful_export(path, exported_count))
}

fn write_run_archive(
    path: &std::path::Path,
    store: &baudbound_storage::SqliteRunnerStore,
    scripts: &[InstalledScript],
    run_ids: &[String],
    schema_version: Option<i64>,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "export path has no parent directory".to_owned())?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(string_error)?;
    {
        let mut archive = ZipWriter::new(&mut temporary);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for run_id in run_ids {
            let run = store
                .find_run_record_by_id(run_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| format!("run {run_id} is no longer available"))?;
            let document = run_export_document(&run, scripts, schema_version);
            archive
                .start_file(
                    format!("runs/{}.json", safe_file_component(run_id)),
                    options,
                )
                .map_err(string_error)?;
            archive
                .write_all(&serde_json::to_vec_pretty(&document).map_err(string_error)?)
                .map_err(string_error)?;
        }
        archive
            .start_file("manifest.json", options)
            .map_err(string_error)?;
        archive
            .write_all(
                &serde_json::to_vec_pretty(&serde_json::json!({
                    "format": "baudbound.run-export-archive",
                    "format_version": 1,
                    "exported_at": Utc::now().to_rfc3339(),
                    "run_count": run_ids.len(),
                    "run_ids": run_ids,
                }))
                .map_err(string_error)?,
            )
            .map_err(string_error)?;
        archive.finish().map_err(string_error)?;
    }
    temporary.as_file_mut().flush().map_err(string_error)?;
    temporary
        .persist(path)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

fn run_export_document(
    run: &StoredRunRecord,
    scripts: &[InstalledScript],
    schema_version: Option<i64>,
) -> serde_json::Value {
    let script = scripts.iter().find(|script| script.id == run.script_id);
    serde_json::json!({
        "format": "baudbound.run",
        "format_version": 1,
        "exported_at": Utc::now().to_rfc3339(),
        "runner": {
            "version": env!("CARGO_PKG_VERSION"),
            "operating_system": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
            "storage_schema_version": schema_version,
        },
        "script": script,
        "run": run,
    })
}

fn write_log_export(
    path: &std::path::Path,
    format: LogExportFormat,
    store: &baudbound_storage::SqliteRunnerStore,
    query: &RunLogQuery,
    first_page: PaginatedRecords<StoredRunLogRecord>,
    schema_version: Option<i64>,
) -> Result<(), String> {
    write_atomic_with(path, |output| {
        match format {
            LogExportFormat::Json => write_log_json_header(output, query, schema_version)?,
            LogExportFormat::Csv => write_log_csv_header(output)?,
        }
        let total = first_page.total;
        let mut offset = 0;
        let mut page = first_page;
        let mut first_json_record = true;
        loop {
            for log in &page.items {
                match format {
                    LogExportFormat::Json => {
                        if !first_json_record {
                            output.write_all(b",").map_err(string_error)?;
                        }
                        serde_json::to_writer(&mut *output, log).map_err(string_error)?;
                        first_json_record = false;
                    }
                    LogExportFormat::Csv => write_log_csv_row(output, log, schema_version)?,
                }
            }
            offset += page.items.len();
            if offset >= total || page.items.is_empty() {
                break;
            }
            let mut next_query = query.clone();
            next_query.limit = 200;
            next_query.offset = offset;
            page = store
                .query_run_logs(&next_query)
                .map_err(|error| error.to_string())?;
        }
        if matches!(format, LogExportFormat::Json) {
            output.write_all(b"]}").map_err(string_error)?;
        }
        Ok(())
    })
}

fn write_log_json_header(
    output: &mut std::fs::File,
    query: &RunLogQuery,
    schema_version: Option<i64>,
) -> Result<(), String> {
    let mut header = serde_json::to_vec(&serde_json::json!({
        "format": "baudbound.logs",
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
    output.write_all(b",\"logs\":[").map_err(string_error)
}

fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    write_atomic_with(path, |output| output.write_all(bytes).map_err(string_error))
}

fn write_atomic_with(
    path: &std::path::Path,
    write: impl FnOnce(&mut std::fs::File) -> Result<(), String>,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "export path has no parent directory".to_owned())?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(string_error)?;
    write(temporary.as_file_mut())?;
    temporary.as_file_mut().flush().map_err(string_error)?;
    temporary
        .persist(path)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

fn write_log_csv_header(output: &mut std::fs::File) -> Result<(), String> {
    output.write_all(b"runner_version,operating_system,architecture,storage_schema_version,timestamp_unix_ms,level,script_name,script_id,run_id,log_index,node_id,action_type,message\r\n").map_err(string_error)
}

fn write_log_csv_row(
    output: &mut std::fs::File,
    log: &StoredRunLogRecord,
    storage_schema_version: Option<i64>,
) -> Result<(), String> {
    let fields = [
        env!("CARGO_PKG_VERSION").to_owned(),
        std::env::consts::OS.to_owned(),
        std::env::consts::ARCH.to_owned(),
        storage_schema_version
            .map(|value| value.to_string())
            .unwrap_or_default(),
        log.timestamp_unix_ms.to_string(),
        log.level.clone(),
        log.script_name.clone(),
        log.script_id.clone(),
        log.run_id.clone(),
        log.log_index.to_string(),
        log.node_id.clone().unwrap_or_default(),
        log.action_type.clone().unwrap_or_default(),
        log.message.clone(),
    ];
    output
        .write_all(format!("{}\r\n", fields.map(|field| csv_field(&field)).join(",")).as_bytes())
        .map_err(string_error)
}

fn csv_field(value: &str) -> String {
    let spreadsheet_safe =
        if value.chars().next().is_some_and(|character| {
            matches!(character, '=' | '+' | '-' | '@' | '\t' | '\r' | '\n')
        }) {
            format!("'{value}")
        } else {
            value.to_owned()
        };
    format!("\"{}\"", spreadsheet_safe.replace('"', "\"\""))
}

fn safe_file_component(value: &str) -> String {
    let safe = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .take(120)
        .collect::<String>();
    if safe.is_empty() {
        "run".to_owned()
    } else {
        safe
    }
}

fn cancelled_export() -> ExportResult {
    ExportResult {
        cancelled: true,
        exported_count: 0,
        file_name: None,
    }
}

fn successful_export(path: &std::path::Path, exported_count: usize) -> ExportResult {
    ExportResult {
        cancelled: false,
        exported_count,
        file_name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned()),
    }
}

fn string_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::{csv_field, safe_file_component};

    #[test]
    fn csv_fields_preserve_multiline_diagnostics() {
        assert_eq!(
            csv_field("line 1\n\"line 2\""),
            "\"line 1\n\"\"line 2\"\"\""
        );
    }

    #[test]
    fn csv_fields_neutralize_spreadsheet_formulas() {
        for value in [
            "=HYPERLINK(\"https://example.invalid\")",
            "+1",
            "-1",
            "@SUM(1,2)",
            "\tformula",
        ] {
            assert!(csv_field(value).starts_with("\"'"));
        }
        assert_eq!(csv_field("ordinary value"), "\"ordinary value\"");
    }

    #[test]
    fn exported_run_names_cannot_create_paths() {
        assert_eq!(safe_file_component("../run:1"), "___run_1");
    }
}
