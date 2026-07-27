use anyhow::{Result, anyhow};
use baudbound_storage::ScriptStore;
use tauri::State;

use super::{
    ActionPayload, DesktopUiState,
    command_guard::{SensitiveOperation, SensitiveOperationGuard},
    consume_sensitive_operation, current_core, run_locked_action,
};

#[tauri::command]
pub(super) fn remove_script<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    reference: String,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::RemoveScript {
            reference: reference.clone(),
        },
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        let script = current_core(&state)?.remove_installed(&state.store, &reference)?;
        state.blacklist.remove_script_state(&script.id)?;
        Ok(format!("Removed {} ({}).", script.name, script.id))
    })
}

#[tauri::command]
pub(super) fn clear_run_history<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::ClearRunHistory,
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        let deleted = state.store.clear_run_records()?;
        Ok(match deleted {
            0 => "Run history is already empty.".to_owned(),
            1 => "Cleared 1 stored run.".to_owned(),
            count => format!("Cleared {count} stored runs."),
        })
    })
}

#[tauri::command]
pub(super) fn clear_run_logs<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::ClearRunLogs,
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        let updated = state.store.clear_run_logs()?;
        Ok(match updated {
            0 => "Stored run logs are already empty.".to_owned(),
            1 => "Cleared stored logs from 1 run.".to_owned(),
            count => format!("Cleared stored logs from {count} runs."),
        })
    })
}

#[tauri::command]
pub(super) fn set_script_enabled(
    reference: String,
    enabled: bool,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    run_locked_action(&state, || {
        current_core(&state)?.set_installed_enabled(&state.store, &reference, enabled)?;
        Ok(format!(
            "{} {reference}.",
            if enabled { "Enabled" } else { "Disabled" }
        ))
    })
}

#[tauri::command]
pub(super) fn set_script_automatic_update_checks<R: tauri::Runtime>(
    confirmation_id: String,
    enabled: bool,
    guard: State<'_, SensitiveOperationGuard>,
    reference: String,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::SetScriptAutomaticUpdateChecks {
            reference: reference.clone(),
            enabled,
        },
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        if enabled {
            let installed = state.store.verify_script_package_hash(&reference)?;
            let decision = state
                .blacklist
                .script_decision(&installed.id, &installed.package_hash);
            if decision.blocks_update_source() {
                return Err(anyhow!(
                    "automatic update checks cannot be enabled because this script is restricted by the Official blacklist"
                ));
            }
            let package = baudbound_script::load_script_package(&installed.package_path)?;
            if package.manifest.repository_url.trim().is_empty() {
                return Err(anyhow!("this script does not provide a repository URL"));
            }
        }
        state
            .store
            .set_script_automatic_update_checks(&reference, enabled)?;
        state.script_update_worker.wake();
        Ok(format!(
            "Automatic update checks are {} for {reference}.",
            if enabled { "enabled" } else { "disabled" }
        ))
    })
}
