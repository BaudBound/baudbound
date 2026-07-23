use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use baudbound_core::RunnerConfig;
use baudbound_storage::SqliteRunnerStore;
use tauri::Emitter;

use super::check_script_update;

pub(crate) const SCRIPT_UPDATE_EVENT: &str = "runner-script-update-state-changed";
const MAX_POLL_INTERVAL: Duration = Duration::from_secs(60);
const MIN_POLL_INTERVAL: Duration = Duration::from_secs(10);
const MAX_STARTUP_JITTER_SECONDS: u64 = 15;
type WorkerControl = Arc<(Mutex<bool>, Condvar)>;

#[derive(Default)]
pub(crate) struct ScriptUpdateWorker {
    control: Mutex<Option<WorkerControl>>,
}

impl ScriptUpdateWorker {
    pub(crate) fn start<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        config_path: PathBuf,
        store: SqliteRunnerStore,
    ) {
        let mut current = self
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if current.is_some() {
            return;
        }
        let control = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_control = Arc::clone(&control);
        match thread::Builder::new()
            .name("baudbound-script-update-check".to_owned())
            .spawn(move || run_worker(app, config_path, store, worker_control))
        {
            Ok(_) => *current = Some(control),
            Err(error) => tracing::warn!(%error, "failed to start script update checker"),
        }
    }

    pub(crate) fn wake(&self) {
        let current = self
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(control) = current.as_ref() {
            control.1.notify_all();
        }
    }
}

impl Drop for ScriptUpdateWorker {
    fn drop(&mut self) {
        let current = self
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(control) = current.as_ref() {
            let mut shutdown = control.0.lock().unwrap_or_else(|error| error.into_inner());
            *shutdown = true;
            control.1.notify_all();
        }
    }
}

fn run_worker<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    config_path: PathBuf,
    store: SqliteRunnerStore,
    control: WorkerControl,
) {
    let startup_jitter = Duration::from_secs(startup_jitter_seconds());
    if wait_or_shutdown(&control, startup_jitter) {
        return;
    }

    loop {
        let poll_interval = match RunnerConfig::load_or_default(&config_path) {
            Ok(config) => {
                check_due_scripts(&app, &store, &config);
                Duration::from_secs(config.updates.check_interval_hours.saturating_mul(60 * 60))
                    .clamp(MIN_POLL_INTERVAL, MAX_POLL_INTERVAL)
            }
            Err(error) => {
                tracing::debug!(%error, "failed to load configuration for script update checks");
                MAX_POLL_INTERVAL
            }
        };
        if wait_or_shutdown(&control, poll_interval) {
            return;
        }
    }
}

fn check_due_scripts<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SqliteRunnerStore,
    config: &RunnerConfig,
) {
    let states = match store.list_script_update_states() {
        Ok(states) => states,
        Err(error) => {
            tracing::debug!(%error, "failed to load script update preferences");
            return;
        }
    };
    let now = unix_timestamp();
    let interval = config.updates.check_interval_hours.saturating_mul(60 * 60);
    for state in states {
        if !state.automatic_checks_enabled {
            continue;
        }
        if state
            .last_checked_at_unix
            .is_some_and(|checked| now.saturating_sub(checked) < interval)
        {
            continue;
        }
        if let Err(error) = check_script_update(
            store,
            config.limits.max_file_download_bytes,
            &state.script_id,
        ) {
            tracing::debug!(script_id = %state.script_id, %error, "automatic script update check failed");
        }
        if let Err(error) = app.emit(SCRIPT_UPDATE_EVENT, &state.script_id) {
            tracing::debug!(%error, "failed to publish script update state event");
        }
    }
}

fn wait_or_shutdown(control: &WorkerControl, duration: Duration) -> bool {
    let (shutdown, wake) = &**control;
    let shutdown = shutdown.lock().unwrap_or_else(|error| error.into_inner());
    if *shutdown {
        return true;
    }
    let (shutdown, _) = wake
        .wait_timeout(shutdown, duration)
        .unwrap_or_else(|error| error.into_inner());
    *shutdown
}

fn startup_jitter_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos() as u64 % (MAX_STARTUP_JITTER_SECONDS + 1))
        .unwrap_or(0)
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
