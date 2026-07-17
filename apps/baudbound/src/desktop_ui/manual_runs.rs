use std::sync::{Arc, Mutex};

use baudbound_core::{CoreError, RunnerCore};
use baudbound_runtime::RuntimeError;
use baudbound_storage::SqliteRunnerStore;
use tauri::{Runtime, State, WebviewWindow};

use super::{
    ActionPayload, DesktopUiState, build_dashboard_payload,
    command_guard::{SensitiveOperation, SensitiveOperationGuard},
    consume_sensitive_operation,
};

#[tauri::command]
pub(super) async fn run_script<R: Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    reference: String,
    state: State<'_, DesktopUiState>,
    window: WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::RunScript {
            reference: reference.clone(),
        },
        &guard,
        &state,
        &window,
    )?;
    let core = Arc::clone(&state.core);
    let store = state.store.clone();

    let message =
        tauri::async_runtime::spawn_blocking(move || execute_manual_run(core, store, reference))
            .await
            .map_err(|error| format!("manual script worker failed: {error}"))??;

    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload { dashboard, message })
}

fn execute_manual_run(
    core: Arc<Mutex<RunnerCore>>,
    store: SqliteRunnerStore,
    reference: String,
) -> Result<String, String> {
    let core = core
        .lock()
        .map_err(|_| "runner core lock is poisoned".to_owned())?
        .clone();
    match core.run_installed(&store, &reference) {
        Ok(report) => Ok(format!(
            "Run {} completed for {reference}.",
            report.identity.run_id
        )),
        Err(CoreError::Runtime(RuntimeError::Cancelled)) => {
            Ok(format!("Stopped the active run for {reference}."))
        }
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub(super) fn stop_run(
    run_id: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    if !state.active_runs.stop_run(&run_id) {
        return Err(format!("Run {run_id} is no longer active."));
    }
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload {
        dashboard,
        message: format!("Stop requested for run {run_id}."),
    })
}

#[tauri::command]
pub(super) fn stop_script_runs(
    reference: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    let stopped = state.active_runs.stop_script_runs(&reference);
    if stopped == 0 {
        return Err(format!("{reference} has no active runs to stop."));
    }
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload {
        dashboard,
        message: format!(
            "Stop requested for {stopped} active {} of {reference}.",
            if stopped == 1 { "run" } else { "runs" }
        ),
    })
}
