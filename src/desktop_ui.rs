use std::{
    path::PathBuf,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

use anyhow::{Result, anyhow};
use baudbound_core::{RunnerConfig, RunnerCore};
use baudbound_storage::SqliteRunnerStore;
use baudbound_triggers::WebSocketConnectionRegistry;
use tauri::{Emitter, Manager, State};

const BLACKLIST_CHANGED_EVENT: &str = "runner-blacklist-changed";

use crate::service::ServeOptions;
use crate::trigger_monitor::TriggerMonitor;

mod active_runs;
mod background;
mod command_guard;
mod config;
mod coordinate_picker;
mod dashboard;
mod desktop_config;
mod history;
mod lifecycle;
mod manual_runs;
mod packages;
mod repositories;
mod scripts;
mod secret_vault;
mod security;
mod tools;

use active_runs::ActiveRunRegistry;
use background::DesktopRunnerSupervisor;
use command_guard::{
    SensitiveOperation, SensitiveOperationGuard, ensure_main_window, ensure_main_window_source,
};
use config::*;
use dashboard::*;
use packages::*;
macro_rules! desktop_command_handler {
    () => {
        tauri::generate_handler![
            packages::approve_script,
            security::check_official_blacklist,
            repositories::add_script_repository,
            scripts::clear_run_history,
            scripts::clear_run_logs,
            scripts::reset_stored_variables,
            packages::check_script_update,
            packages::check_script_updates,
            packages::cancel_remote_script_package_preparation,
            prepare_sensitive_operation,
            dashboard_state,
            history::export_logs,
            history::export_runs,
            history::export_variables,
            history::query_logs,
            history::query_runs,
            history::variable_inventory,
            coordinate_picker::cancel_coordinate_picker,
            tools::discover_monitors,
            packages::import_script_package,
            packages::install_remote_script_package,
            packages::discard_remote_package_review,
            packages::prepare_remote_script_package,
            packages::prepare_discovered_script_update,
            repositories::prepare_repository_script,
            repositories::preview_script_repository,
            prepare_for_update,
            scripts::remove_script,
            repositories::remove_script_repository,
            repositories::repository_sources,
            repositories::repository_script_filter_options,
            repositories::repository_script_details,
            repositories::query_repository_scripts,
            repositories::refresh_all_script_repositories,
            repositories::refresh_script_repository,
            packages::revoke_script_approval,
            reload_background_runner,
            retry_secret_vault,
            security::unlock_secret_storage,
            security::lock_secret_storage,
            security::switch_secret_storage,
            config::read_runner_config,
            manual_runs::run_script,
            manual_runs::stop_run,
            manual_runs::stop_manual_script_runs,
            manual_runs::stop_script_runs,
            config::save_runner_config,
            config::save_runner_config_model,
            coordinate_picker::select_coordinate_picker,
            tools::scan_serial_ports,
            security::set_script_secret,
            security::remove_script_secret,
            config::reset_runner_config,
            security::rotate_network_trigger_token,
            packages::select_package_file,
            scripts::set_script_enabled,
            scripts::set_script_automatic_update_checks,
            repositories::set_script_repository_enabled,
            security::set_personal_repository_block,
            security::set_network_trigger_auth_enabled,
            start_background_runner,
            coordinate_picker::start_coordinate_picker,
            stop_background_runner,
            should_check_for_update,
            start_trigger_monitor,
            stop_trigger_monitor,
            clear_trigger_monitor,
            trigger_monitor_state,
            record_update_check,
            packages::update_script_package,
        ]
    };
}

fn secured_desktop_command_handler()
-> Box<dyn Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync> {
    let handler: Box<dyn Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync> =
        Box::new(desktop_command_handler!());
    Box::new(move |invoke| {
        let command = invoke.message.command();
        let window_label = invoke.message.webview_ref().label();
        if !desktop_command_is_allowed(window_label, command) {
            invoke
                .resolver
                .reject("this window is not authorized to invoke runner management commands");
            true
        } else {
            handler(invoke)
        }
    })
}

fn desktop_command_is_allowed(window_label: &str, command: &str) -> bool {
    window_label == "main"
        || (window_label.starts_with("coordinate-picker-")
            && matches!(
                command,
                "select_coordinate_picker" | "cancel_coordinate_picker"
            ))
}

pub fn run_desktop_ui(
    config_path: PathBuf,
    core: RunnerCore,
    store: SqliteRunnerStore,
    runner_config: RunnerConfig,
    websocket_registry: Arc<WebSocketConnectionRegistry>,
    blacklist: Arc<crate::blacklist::BlacklistService>,
    launched_from_autostart: bool,
) -> Result<()> {
    let runner_home = store
        .database_path()
        .parent()
        .ok_or_else(|| anyhow!("runner database path does not have a parent directory"))?
        .to_path_buf();
    if let Err(error) = crate::script_repositories::ensure_official_repository(&store) {
        tracing::warn!(%error, "failed to register the official script repository");
    }
    let active_runs = Arc::new(ActiveRunRegistry::default());
    let trigger_monitor = TriggerMonitor::default();
    let core = core.with_run_observer(Arc::clone(&active_runs));
    let serial_connections = core.serial_connections();
    let background_options = desktop_background_options(
        &runner_config,
        Arc::clone(&websocket_registry),
        config_path.clone(),
        serial_connections,
        trigger_monitor.clone(),
    );
    let background_runner = DesktopRunnerSupervisor::default();
    let secret_vault = secret_vault::SecretVaultController::new(runner_home);
    let password_unlock_required = secret_vault.snapshot().mode
        == secret_vault::SecretStorageMode::Password
        && store.stored_secret_value_count()? > 0;
    let defer_background_start_for_secret_vault =
        runner_config.desktop.start_background_runner_on_launch
            && crate::service::enabled_scripts_require_secret_access(&core, &store)?;
    let autostart_args = [
        "--config".to_owned(),
        config_path.display().to_string(),
        "--gui".to_owned(),
        "--autostart".to_owned(),
    ];
    tauri::Builder::default()
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("BaudBound")
                .args(autostart_args)
                .build(),
        )
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(coordinate_picker::CoordinatePickerState::default())
        .manage(SensitiveOperationGuard::default())
        .manage(crate::script_updates::RemotePreparationRegistry::default())
        .manage(crate::script_updates::RemotePackageReviews::default())
        .manage(DesktopUiState {
            background_options: Mutex::new(background_options),
            active_runs,
            background_runner: background_runner.clone(),
            blacklist,
            config_path,
            login_startup_registered: Mutex::new(None),
            runner_config: Mutex::new(runner_config),
            core: Arc::new(Mutex::new(core)),
            repository_refresh_worker: repositories::RepositoryRefreshWorker::default(),
            secret_vault: secret_vault.clone(),
            script_update_worker: crate::script_updates::ScriptUpdateWorker::default(),
            store,
            websocket_registry,
            operation_lock: Arc::new(Mutex::new(())),
            trigger_monitor,
            deferred_background_start: AtomicBool::new(defer_background_start_for_secret_vault),
        })
        .setup(move |app| {
            app.state::<DesktopUiState>()
                .active_runs
                .connect_event_sink(app.handle().clone());
            app.state::<DesktopUiState>()
                .background_runner
                .connect_event_sink(app.handle().clone());
            app.state::<DesktopUiState>()
                .trigger_monitor
                .connect_event_sink(app.handle().clone())
                .map_err(|error| anyhow!(error))?;
            let blacklist_event_app = app.handle().clone();
            app.state::<DesktopUiState>()
                .blacklist
                .set_change_callback(Arc::new(move || {
                    if let Err(error) = blacklist_event_app.emit(BLACKLIST_CHANGED_EVENT, ()) {
                        tracing::warn!(%error, "failed to publish blacklist change event");
                    }
                }));
            desktop_config::reconcile_autostart_registration(app.handle());
            lifecycle::configure_desktop_lifecycle(
                app,
                launched_from_autostart,
                password_unlock_required,
            )?;
            let state = app.state::<DesktopUiState>();
            let variable_event_app = app.handle().clone();
            state.store.set_variable_change_observer(move |change| {
                if let Err(error) = variable_event_app.emit("runner-variable-changed", change) {
                    tracing::warn!(%error, "failed to publish variable change event");
                }
            });
            state
                .secret_vault
                .start(app.handle().clone(), state.store.clone());
            state.script_update_worker.start(
                app.handle().clone(),
                state.config_path.clone(),
                state.store.clone(),
            );
            state.repository_refresh_worker.start(
                app.handle().clone(),
                state.config_path.clone(),
                state.store.clone(),
            );
            if !defer_background_start_for_secret_vault {
                desktop_config::start_configured_background_runner(app.handle());
            }
            if let Some(window) = app.get_webview_window("main") {
                window
                    .set_title("BaudBound")
                    .map_err(|source| anyhow!("failed to set window title: {source}"))?;
            }
            Ok(())
        })
        .invoke_handler(secured_desktop_command_handler())
        .run(tauri::generate_context!())
        .map_err(|source| anyhow!("desktop UI failed: {source}"))
}

pub(super) struct DesktopUiState {
    active_runs: Arc<ActiveRunRegistry>,
    background_options: Mutex<ServeOptions>,
    background_runner: DesktopRunnerSupervisor,
    blacklist: Arc<crate::blacklist::BlacklistService>,
    config_path: PathBuf,
    login_startup_registered: Mutex<Option<bool>>,
    runner_config: Mutex<RunnerConfig>,
    core: Arc<Mutex<RunnerCore>>,
    repository_refresh_worker: repositories::RepositoryRefreshWorker,
    secret_vault: secret_vault::SecretVaultController,
    script_update_worker: crate::script_updates::ScriptUpdateWorker,
    store: SqliteRunnerStore,
    websocket_registry: Arc<WebSocketConnectionRegistry>,
    operation_lock: Arc<Mutex<()>>,
    trigger_monitor: TriggerMonitor,
    deferred_background_start: AtomicBool,
}

#[tauri::command]
fn trigger_monitor_state(
    state: State<'_, DesktopUiState>,
) -> crate::trigger_monitor::TriggerMonitorState {
    state.trigger_monitor.state()
}

#[tauri::command]
fn start_trigger_monitor(
    state: State<'_, DesktopUiState>,
) -> crate::trigger_monitor::TriggerMonitorState {
    state.trigger_monitor.start()
}

#[tauri::command]
fn stop_trigger_monitor(
    state: State<'_, DesktopUiState>,
) -> crate::trigger_monitor::TriggerMonitorState {
    state.trigger_monitor.stop()
}

#[tauri::command]
fn clear_trigger_monitor(
    state: State<'_, DesktopUiState>,
) -> crate::trigger_monitor::TriggerMonitorState {
    state.trigger_monitor.clear()
}

#[tauri::command]
fn prepare_sensitive_operation<R: tauri::Runtime>(
    operation: SensitiveOperation,
    guard: State<'_, SensitiveOperationGuard>,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<command_guard::ConfirmationChallenge, String> {
    ensure_main_window(&window)?;
    guard.prepare(&operation, &state)
}

fn consume_sensitive_operation<R: tauri::Runtime>(
    confirmation_id: &str,
    operation: &SensitiveOperation,
    guard: &SensitiveOperationGuard,
    state: &DesktopUiState,
    window: &tauri::WebviewWindow<R>,
) -> Result<(), String> {
    ensure_main_window(window)?;
    guard.consume(confirmation_id, operation, state)
}

fn consume_package_selection<R: tauri::Runtime>(
    confirmation_id: &str,
    operation: &SensitiveOperation,
    guard: &SensitiveOperationGuard,
    state: &DesktopUiState,
    window: &tauri::WebviewWindow<R>,
) -> Result<(), String> {
    ensure_main_window_source(window)?;
    guard.consume_package_selection(confirmation_id, operation, state)
}

#[tauri::command]
fn dashboard_state(state: State<'_, DesktopUiState>) -> Result<DashboardPayload, String> {
    build_dashboard_payload(&state).map_err(|error| error.to_string())
}

#[tauri::command]
fn retry_secret_vault<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    let result = state.secret_vault.start(app, state.store.clone());
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    let message = match result {
        secret_vault::StartResult::Started => "Credential vault connection started.",
        secret_vault::StartResult::AlreadyInitializing => {
            "Credential vault connection is already in progress."
        }
        secret_vault::StartResult::AlreadyAvailable => "Credential vault is already available.",
        secret_vault::StartResult::PasswordLocked => "Password protected secret storage is locked.",
    };
    Ok(ActionPayload {
        dashboard,
        message: message.to_owned(),
    })
}

#[tauri::command]
fn should_check_for_update(state: State<'_, DesktopUiState>) -> Result<bool, String> {
    let config = current_runner_config(&state).map_err(|error| error.to_string())?;
    if !config.updates.automatic_checks {
        return Ok(false);
    }
    crate::updates::check_is_due(&state.store, config.updates.check_interval_hours)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn record_update_check(
    latest_version: Option<String>,
    release_notes: Option<String>,
    state: State<'_, DesktopUiState>,
) -> Result<(), String> {
    crate::updates::record_desktop_check(&state.store, latest_version.as_deref(), release_notes)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn start_background_runner(state: State<'_, DesktopUiState>) -> Result<ActionPayload, String> {
    run_locked_action(&state, || start_background_runner_message(&state))
}

#[tauri::command]
fn reload_background_runner(state: State<'_, DesktopUiState>) -> Result<ActionPayload, String> {
    run_locked_action(&state, || reload_background_runner_message(&state))
}

#[tauri::command]
fn stop_background_runner(state: State<'_, DesktopUiState>) -> Result<ActionPayload, String> {
    run_locked_action(&state, || stop_background_runner_message(&state))
}

#[tauri::command]
fn prepare_for_update<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::PrepareForUpdate,
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        let message = state
            .background_runner
            .stop_and_wait(std::time::Duration::from_secs(5))?;
        if state.background_runner.snapshot()?.running {
            return Err(anyhow!(
                "desktop background runner did not stop before the update deadline"
            ));
        }
        Ok(message)
    })
}

fn run_locked_action(
    state: &DesktopUiState,
    action: impl FnOnce() -> Result<String>,
) -> Result<ActionPayload, String> {
    let message = run_locked_message(state, action)?;
    let dashboard = build_dashboard_payload(state).map_err(|error| error.to_string())?;
    Ok(ActionPayload { dashboard, message })
}

fn run_locked_value<T>(
    state: &DesktopUiState,
    action: impl FnOnce() -> Result<T>,
) -> Result<T, String> {
    let _guard = state
        .operation_lock
        .lock()
        .map_err(|_| "desktop UI operation lock is poisoned".to_owned())?;
    action().map_err(|error| error.to_string())
}

pub(super) fn run_locked_message(
    state: &DesktopUiState,
    action: impl FnOnce() -> Result<String>,
) -> Result<String, String> {
    let _guard = state
        .operation_lock
        .lock()
        .map_err(|_| "desktop UI operation lock is poisoned".to_owned())?;
    action().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
