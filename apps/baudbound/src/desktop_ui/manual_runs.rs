use std::sync::{Arc, Mutex};

use baudbound_core::RunnerCore;
use baudbound_storage::SqliteRunnerStore;
use tauri::State;

use super::{ActionPayload, DesktopUiState, build_dashboard_payload};

#[tauri::command]
pub(super) async fn run_script(
    reference: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    let core = Arc::clone(&state.core);
    let store = state.store.clone();
    let operation_lock = Arc::clone(&state.operation_lock);

    let message = tauri::async_runtime::spawn_blocking(move || {
        execute_manual_run(core, store, operation_lock, reference)
    })
    .await
    .map_err(|error| format!("manual script worker failed: {error}"))??;

    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload { dashboard, message })
}

fn execute_manual_run(
    core: Arc<Mutex<RunnerCore>>,
    store: SqliteRunnerStore,
    operation_lock: Arc<Mutex<()>>,
    reference: String,
) -> Result<String, String> {
    let _guard = operation_lock
        .lock()
        .map_err(|_| "desktop UI operation lock is poisoned".to_owned())?;
    let core = core
        .lock()
        .map_err(|_| "runner core lock is poisoned".to_owned())?
        .clone();
    let report = core
        .run_installed(&store, &reference)
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "Run {} completed for {reference}.",
        report.identity.run_id
    ))
}
