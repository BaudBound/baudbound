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
        manager.enable().context("failed to enable login startup")?;
        correct_linux_autostart_comment()
    } else if current {
        manager.disable().context("failed to disable login startup")
    } else {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn correct_linux_autostart_comment() -> Result<()> {
    let home = std::env::var_os("HOME").context("HOME is unavailable")?;
    let path = std::path::PathBuf::from(home)
        .join(".config")
        .join("autostart")
        .join("BaudBound.desktop");
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read login startup entry {}", path.display()))?;
    let corrected = corrected_linux_autostart_entry(&contents)?;
    if corrected == contents {
        return Ok(());
    }

    let temporary_path = path.with_extension(format!("desktop.tmp-{}", std::process::id()));
    std::fs::write(&temporary_path, corrected).with_context(|| {
        format!(
            "failed to write temporary login startup entry {}",
            temporary_path.display()
        )
    })?;
    std::fs::rename(&temporary_path, &path)
        .with_context(|| format!("failed to replace login startup entry {}", path.display()))
}

#[cfg(not(target_os = "linux"))]
fn correct_linux_autostart_comment() -> Result<()> {
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
fn corrected_linux_autostart_entry(contents: &str) -> Result<String> {
    const COMMENT: &str = "Comment=BaudBound startup script";
    let mut found_comment = false;
    let mut lines = contents
        .lines()
        .map(|line| {
            if line.starts_with("Comment=") {
                found_comment = true;
                COMMENT
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    anyhow::ensure!(
        found_comment,
        "generated login startup entry does not contain a Comment field"
    );
    if contents.ends_with('\n') {
        lines.push('\n');
    }
    Ok(lines)
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

#[cfg(test)]
mod tests {
    use super::corrected_linux_autostart_entry;

    #[test]
    fn corrects_generated_linux_autostart_comment_spacing() {
        let contents = "[Desktop Entry]\nComment=BaudBoundstartup script\nExec=baudbound --gui";

        assert_eq!(
            corrected_linux_autostart_entry(contents).expect("desktop entry should be corrected"),
            "[Desktop Entry]\nComment=BaudBound startup script\nExec=baudbound --gui"
        );
    }
}
