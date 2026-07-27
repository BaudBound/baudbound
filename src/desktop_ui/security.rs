use anyhow::Result;
use baudbound_storage::NetworkTriggerType;
use serde::Deserialize;
use tauri::State;

use super::{
    ActionPayload, DesktopUiState, GeneratedTriggerTokenPayload, build_dashboard_payload,
    command_guard::{SensitiveOperation, SensitiveOperationGuard},
    consume_sensitive_operation, current_core, run_locked_action, run_locked_value,
    trigger_type_label,
};

#[tauri::command]
pub(super) fn rotate_network_trigger_token<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    reference: String,
    node_id: String,
    trigger_type: NetworkTriggerType,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<GeneratedTriggerTokenPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::RotateNetworkTriggerToken {
            reference: reference.clone(),
            node_id: node_id.clone(),
            trigger_type,
        },
        &guard,
        &state,
        &window,
    )?;
    let generated = run_locked_value(&state, || {
        Ok(current_core(&state)?.rotate_trigger_token(
            &state.store,
            &reference,
            &node_id,
            trigger_type,
        )?)
    })?;
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(GeneratedTriggerTokenPayload {
        dashboard,
        message: format!(
            "Generated a new {} token for {reference}:{node_id}. Save it now because it cannot be shown again.",
            trigger_type_label(&generated.status.trigger_type)
        ),
        status: generated.status,
        token: generated.token,
    })
}

#[tauri::command]
pub(super) fn set_network_trigger_auth_enabled<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    request: SetNetworkTriggerAuthEnabledRequest,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    let SetNetworkTriggerAuthEnabledRequest {
        enabled,
        node_id,
        reference,
        trigger_type,
    } = request;
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::SetNetworkTriggerAuthEnabled {
            reference: reference.clone(),
            node_id: node_id.clone(),
            trigger_type,
            enabled,
        },
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        current_core(&state)?.set_trigger_auth_enabled(
            &state.store,
            &reference,
            &node_id,
            trigger_type,
            enabled,
        )?;
        Ok(format!(
            "{} authentication for {reference}:{node_id}.",
            if enabled { "Enabled" } else { "Disabled" }
        ))
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SetNetworkTriggerAuthEnabledRequest {
    enabled: bool,
    node_id: String,
    reference: String,
    trigger_type: NetworkTriggerType,
}

#[tauri::command]
pub(super) fn set_script_secret<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    reference: String,
    name: String,
    value: String,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::SetScriptSecret {
            reference: reference.clone(),
            name: name.clone(),
            value: value.clone(),
        },
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        current_core(&state)?.set_installed_secret_from_text(
            &state.store,
            &reference,
            &name,
            &value,
        )?;
        Ok(format!("Configured {name} for {reference}."))
    })
}

#[tauri::command]
pub(super) fn remove_script_secret<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    reference: String,
    name: String,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::RemoveScriptSecret {
            reference: reference.clone(),
            name: name.clone(),
        },
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        let removed =
            current_core(&state)?.remove_installed_secret(&state.store, &reference, &name)?;
        Ok(if removed {
            format!("Removed {name} from {reference}.")
        } else {
            format!("{name} was not configured for {reference}.")
        })
    })
}

#[tauri::command]
pub(super) fn check_official_blacklist(
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    run_locked_value(&state, || {
        state.blacklist.refresh_now(Some(&state.store))?;
        let dashboard = build_dashboard_payload(&state)?;
        Ok(ActionPayload {
            dashboard,
            message: "Official blacklist checked.".to_owned(),
        })
    })
}

#[tauri::command]
pub(super) fn set_personal_repository_block(
    blocked: bool,
    state: State<'_, DesktopUiState>,
    url: String,
) -> Result<ActionPayload, String> {
    run_locked_action(&state, || {
        state
            .blacklist
            .set_personal_repository_block(&url, blocked)?;
        if blocked {
            state
                .store
                .set_repository_enabled(&url, false)
                .map_err(anyhow::Error::from)?;
        }
        state.repository_refresh_worker.wake();
        Ok(if blocked {
            "Repository added to your block list.".to_owned()
        } else {
            "Repository removed from your block list.".to_owned()
        })
    })
}
