use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use baudbound_storage::ScriptStore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tauri::State;

use super::{
    ActionPayload, DesktopUiState,
    command_guard::{ScriptSettingDigest, SensitiveOperation, SensitiveOperationGuard},
    consume_sensitive_operation, current_core, run_locked_action,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveScriptSettingInput {
    name: String,
    value: String,
    value_digest: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveScriptSettingsRequest {
    reference: String,
    settings: Vec<SaveScriptSettingInput>,
}

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
pub(super) fn reset_stored_variables<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::ResetStoredVariables,
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        let (persistent, global) = state.store.clear_stored_variables()?;
        let deleted = persistent + global;
        Ok(match deleted {
            0 => "Stored persistent and global variables are already empty.".to_owned(),
            1 => "Reset 1 stored variable.".to_owned(),
            count => format!("Reset {count} stored variables."),
        })
    })
}

#[tauri::command]
pub(super) fn set_script_enabled<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    reference: String,
    enabled: bool,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::SetScriptEnabled {
            reference: reference.clone(),
            enabled,
        },
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        current_core(&state)?.set_installed_enabled(&state.store, &reference, enabled)?;
        Ok(format!(
            "{} {reference}.",
            if enabled { "Enabled" } else { "Disabled" }
        ))
    })
}

#[tauri::command]
pub(super) fn save_script_settings<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    request: SaveScriptSettingsRequest,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    let reviewed_settings = request
        .settings
        .iter()
        .map(|setting| ScriptSettingDigest {
            name: setting.name.clone(),
            value_digest: setting.value_digest.clone(),
        })
        .collect::<Vec<_>>();
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::SaveScriptSettings {
            reference: request.reference.clone(),
            settings: reviewed_settings,
        },
        &guard,
        &state,
        &window,
    )?;

    let mut values = BTreeMap::new();
    for setting in request.settings {
        verify_setting_value_digest(&setting.value, &setting.value_digest)?;
        if values.insert(setting.name.clone(), setting.value).is_some() {
            return Err(format!(
                "Script Setting {:?} appears more than once",
                setting.name
            ));
        }
    }
    let configured_count = values.len();
    run_locked_action(&state, || {
        current_core(&state)?.save_installed_script_settings_from_text(
            &state.store,
            &request.reference,
            &values,
        )?;
        Ok(format!(
            "Saved Script Settings for {}. {} configured value{}.",
            request.reference,
            configured_count,
            if configured_count == 1 { "" } else { "s" }
        ))
    })
}

fn verify_setting_value_digest(value: &str, expected: &str) -> Result<(), String> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(value.as_bytes());
    let mut actual = String::with_capacity(digest.len() * 2);
    for byte in digest {
        actual.push(DIGITS[(byte >> 4) as usize] as char);
        actual.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    if actual == expected {
        Ok(())
    } else {
        Err("script setting value changed after it was reviewed".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::verify_setting_value_digest;

    #[test]
    fn setting_confirmation_digest_is_bound_to_the_submitted_value() {
        let reviewed = "reviewed value";
        let digest = "b67f020d23e4da9bec60de630856d182a250685c51364cad8b3be0f8692b25f6";

        verify_setting_value_digest(reviewed, digest).expect("matching value should be accepted");
        assert!(
            verify_setting_value_digest("changed value", digest)
                .expect_err("changed value must be rejected")
                .contains("changed after it was reviewed")
        );
    }
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
