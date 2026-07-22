use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use baudbound_core::{CoreError, RunnerCore, TriggerEvent};
use baudbound_runtime::RuntimeError;
use baudbound_storage::SqliteRunnerStore;
use tauri::{Runtime, State, WebviewWindow};

use super::{
    ActionPayload, DesktopUiState, build_dashboard_payload,
    command_guard::{SensitiveOperation, SensitiveOperationGuard},
    consume_sensitive_operation, current_core,
};
use crate::trigger_monitor::{TriggerMonitor, TriggerMonitorStatus};

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
    let trigger_monitor = state.trigger_monitor.clone();

    let message = tauri::async_runtime::spawn_blocking(move || {
        execute_manual_run(core, store, reference, trigger_monitor)
    })
    .await
    .map_err(|error| format!("manual script worker failed: {error}"))??;

    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload { dashboard, message })
}

fn execute_manual_run(
    core: Arc<Mutex<RunnerCore>>,
    store: SqliteRunnerStore,
    reference: String,
    trigger_monitor: TriggerMonitor,
) -> Result<String, String> {
    let core = core
        .lock()
        .map_err(|_| "runner core lock is poisoned".to_owned())?
        .clone();
    if let Ok(Some(registration)) = core
        .list_trigger_registrations(&store, Some(&reference))
        .map(|registrations| {
            registrations
                .into_iter()
                .find(|registration| registration.action_type == "trigger.manual")
        })
    {
        trigger_monitor.observe_submission(
            &TriggerEvent {
                action_type: registration.action_type,
                node_id: registration.node_id,
                payload: serde_json::Value::Null,
                script_id: registration.script_id,
            },
            "manual",
            TriggerMonitorStatus::Queued,
            None,
        );
    }
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

#[tauri::command]
pub(super) fn stop_manual_script_runs(
    reference: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    let manual_trigger_ids = current_core(&state)
        .map_err(|error| error.to_string())?
        .list_trigger_registrations(&state.store, Some(&reference))
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|trigger| trigger.action_type == "trigger.manual")
        .map(|trigger| trigger.node_id)
        .collect::<BTreeSet<_>>();
    let stopped = state
        .active_runs
        .stop_script_trigger_runs(&reference, &manual_trigger_ids);
    if stopped == 0 {
        return Err(format!("{reference} has no active manual runs to stop."));
    }
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload {
        dashboard,
        message: format!(
            "Stop requested for {stopped} active manual {} of {reference}.",
            if stopped == 1 { "run" } else { "runs" }
        ),
    })
}
