use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::{AutoLaunchManager, ManagerExt};

use super::DesktopUiState;

pub fn autostart_registration(manager: &AutoLaunchManager) -> Result<bool> {
    manager
        .is_enabled()
        .context("failed to read operating-system login startup registration")
}

pub fn set_autostart_registration(manager: &AutoLaunchManager, enabled: bool) -> Result<()> {
    let current = autostart_registration(manager)?;
    if enabled {
        // Rewriting an enabled entry keeps its executable path current after an update or move.
        manager.enable().context("failed to enable login startup")
    } else if current {
        manager.disable().context("failed to disable login startup")
    } else {
        Ok(())
    }
}

pub fn reconcile_autostart_registration(app: &AppHandle) {
    let state = app.state::<DesktopUiState>();
    let desired = match state.runner_config.lock() {
        Ok(config) => config.desktop.launch_at_login,
        Err(_) => {
            tracing::warn!("runner config lock is poisoned during login startup reconciliation");
            return;
        }
    };

    if let Err(error) = set_autostart_registration(&app.autolaunch(), desired) {
        tracing::warn!(error = %error, "failed to reconcile login startup registration");
    }
    remember_autostart_registration(&state, &app.autolaunch());
}

pub fn remember_autostart_registration(state: &DesktopUiState, manager: &AutoLaunchManager) {
    let actual = match autostart_registration(manager) {
        Ok(actual) => Some(actual),
        Err(error) => {
            tracing::warn!(%error, "failed to refresh login startup registration state");
            None
        }
    };
    match state.login_startup_registered.lock() {
        Ok(mut stored) => *stored = actual,
        Err(_) => tracing::warn!("login startup state lock is poisoned"),
    }
}

pub fn start_configured_background_runner(app: &AppHandle) {
    let state = app.state::<DesktopUiState>();
    let should_start = match state.runner_config.lock() {
        Ok(config) => config.desktop.start_background_runner_on_launch,
        Err(_) => {
            tracing::warn!("runner config lock is poisoned during background runner startup");
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
