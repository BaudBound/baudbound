use anyhow::{Context, Result, anyhow};
use baudbound_storage::ApplicationSettings;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::AutoLaunchManager;
use tauri_plugin_autostart::ManagerExt;

use super::DesktopUiState;

#[derive(Serialize)]
pub struct ApplicationSettingsPayload {
    pub launch_at_login_registered: bool,
    pub settings: ApplicationSettings,
}

#[derive(Serialize)]
pub struct SettingsActionPayload {
    pub message: String,
    pub payload: ApplicationSettingsPayload,
}

pub fn read_settings_payload(
    autostart: &AutoLaunchManager,
    state: &DesktopUiState,
) -> Result<ApplicationSettingsPayload> {
    let settings = state
        .application_settings
        .lock()
        .map_err(|_| anyhow!("application settings lock is poisoned"))?
        .clone();
    let launch_at_login_registered = autostart
        .is_enabled()
        .context("failed to read operating-system login startup registration")?;
    Ok(ApplicationSettingsPayload {
        launch_at_login_registered,
        settings,
    })
}

pub fn save_settings(
    autostart: &AutoLaunchManager,
    state: &DesktopUiState,
    settings: ApplicationSettings,
) -> Result<SettingsActionPayload> {
    let _operation_guard = state
        .operation_lock
        .lock()
        .map_err(|_| anyhow!("desktop UI operation lock is poisoned"))?;
    let previous_registration = autostart
        .is_enabled()
        .context("failed to read operating-system login startup registration")?;

    set_autostart_registration(autostart, settings.desktop.launch_at_login)
        .context("failed to update operating-system login startup registration")?;

    if let Err(error) = state.store.write_application_settings(&settings) {
        if let Err(rollback_error) = set_autostart_registration(autostart, previous_registration) {
            return Err(anyhow!(
                "failed to save desktop settings: {error}; login startup rollback also failed: {rollback_error}"
            ));
        }
        return Err(anyhow!("failed to save desktop settings: {error}"));
    }

    *state
        .application_settings
        .lock()
        .map_err(|_| anyhow!("application settings lock is poisoned"))? = settings;

    Ok(SettingsActionPayload {
        message: "Saved settings.".to_owned(),
        payload: read_settings_payload(autostart, state)?,
    })
}

pub fn reconcile_autostart_registration(app: &AppHandle) {
    let state = app.state::<DesktopUiState>();
    let desired = match state.application_settings.lock() {
        Ok(settings) => settings.desktop.launch_at_login,
        Err(_) => {
            tracing::warn!("desktop settings lock is poisoned during login startup reconciliation");
            return;
        }
    };

    if let Err(error) = set_autostart_registration(&app.autolaunch(), desired) {
        tracing::warn!(error = %error, "failed to reconcile login startup registration");
    }
}

pub fn start_configured_background_runner(app: &AppHandle) {
    let state = app.state::<DesktopUiState>();
    let should_start = match state.application_settings.lock() {
        Ok(settings) => settings.desktop.start_background_runner_on_launch,
        Err(_) => {
            tracing::warn!("desktop settings lock is poisoned during background runner startup");
            return;
        }
    };
    if !should_start {
        return;
    }

    if let Err(error) =
        super::run_locked_message(&state, || super::start_background_runner_message(&state))
    {
        tracing::warn!(
            error,
            "failed to start configured desktop background runner"
        );
    }
}

fn set_autostart_registration(manager: &AutoLaunchManager, enabled: bool) -> Result<()> {
    let current = manager
        .is_enabled()
        .context("failed to inspect login startup registration")?;
    if enabled {
        // Rewriting an enabled entry keeps its executable path current after an update or move.
        manager.enable().context("failed to enable login startup")
    } else if current {
        manager.disable().context("failed to disable login startup")
    } else {
        Ok(())
    }
}
