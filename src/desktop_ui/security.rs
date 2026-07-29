use anyhow::Result;
use baudbound_storage::NetworkTriggerType;
use serde::Deserialize;
use tauri::State;
use zeroize::Zeroize;

use super::{
    ActionPayload, DesktopUiState, GeneratedTriggerTokenPayload, build_dashboard_payload,
    command_guard::{SensitiveOperation, SensitiveOperationGuard},
    consume_sensitive_operation, current_core, run_locked_action, run_locked_value,
    trigger_type_label,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SwitchSecretStorageRequest {
    mode: super::secret_vault::SecretStorageMode,
    password: Option<String>,
}

#[tauri::command]
pub(super) async fn unlock_secret_storage(
    password: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    let operation_lock = state.operation_lock.clone();
    let controller = state.secret_vault.clone();
    let store = state.store.clone();
    let mut message = tauri::async_runtime::spawn_blocking(move || {
        let mut password = password;
        let result = (|| {
            let _operation = operation_lock
                .lock()
                .map_err(|_| anyhow::anyhow!("runner operation lock is poisoned"))?;
            controller.unlock_password(&password, &store)?;
            Ok::<_, anyhow::Error>("Password protected secret storage is unlocked.".to_owned())
        })();
        password.zeroize();
        result
    })
    .await
    .map_err(|error| format!("secret storage unlock worker failed: {error}"))?
    .map_err(|error| error.to_string())?;
    match super::desktop_config::start_deferred_background_runner(&state) {
        Ok(Some(started)) => {
            message.push(' ');
            message.push_str(&started);
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(
                %error,
                "failed to start the background runner after unlocking secret storage"
            );
            message.push_str(
                " The configured background runner could not start. Check the Service page for details.",
            );
        }
    }
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload { dashboard, message })
}

#[tauri::command]
pub(super) async fn lock_secret_storage(
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    let operation_lock = state.operation_lock.clone();
    let controller = state.secret_vault.clone();
    let store = state.store.clone();
    let background_runner = state.background_runner.clone();
    let active_runs = state.active_runs.clone();
    let message = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("runner operation lock is poisoned"))?;
        ensure_secret_storage_idle(&background_runner, &active_runs)?;
        controller.lock_password(&store)?;
        Ok::<_, anyhow::Error>("Password protected secret storage is locked.".to_owned())
    })
    .await
    .map_err(|error| format!("secret storage lock worker failed: {error}"))?
    .map_err(|error| error.to_string())?;
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload { dashboard, message })
}

#[tauri::command]
pub(super) async fn switch_secret_storage<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    request: SwitchSecretStorageRequest,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    let SwitchSecretStorageRequest { mode, mut password } = request;
    let operation = SensitiveOperation::SwitchSecretStorage {
        mode,
        password: password.clone(),
    };
    consume_sensitive_operation(&confirmation_id, &operation, &guard, &state, &window)?;
    drop(operation);

    let operation_lock = state.operation_lock.clone();
    let controller = state.secret_vault.clone();
    let store = state.store.clone();
    let background_runner = state.background_runner.clone();
    let active_runs = state.active_runs.clone();
    let message = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            let _operation = operation_lock
                .lock()
                .map_err(|_| anyhow::anyhow!("runner operation lock is poisoned"))?;
            ensure_secret_storage_idle(&background_runner, &active_runs)?;
            let cleared = controller.switch(mode, password.as_deref(), &store)?;
            Ok::<_, anyhow::Error>(format!(
                "Secret storage changed to {}. {cleared} saved secret values were reset.",
                match mode {
                    super::secret_vault::SecretStorageMode::OperatingSystem => {
                        "the operating system vault"
                    }
                    super::secret_vault::SecretStorageMode::Password => {
                        "password protected storage"
                    }
                }
            ))
        })();
        if let Some(password) = password.as_mut() {
            password.zeroize();
        }
        result
    })
    .await
    .map_err(|error| format!("secret storage switch worker failed: {error}"))?
    .map_err(|error: anyhow::Error| error.to_string())?;
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload { dashboard, message })
}

fn ensure_secret_storage_idle(
    background_runner: &super::background::DesktopRunnerSupervisor,
    active_runs: &super::active_runs::ActiveRunRegistry,
) -> Result<()> {
    if background_runner.snapshot()?.running {
        anyhow::bail!("stop the desktop background runner before changing secret storage");
    }
    if !active_runs.snapshot().runs.is_empty() {
        anyhow::bail!("stop all running scripts before changing secret storage");
    }
    Ok(())
}

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
