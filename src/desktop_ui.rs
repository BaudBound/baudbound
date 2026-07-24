use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::{Result, anyhow};
use baudbound_actions::DesktopActionHandler;
use baudbound_core::{RunnerConfig, RunnerCore, SerialDeviceSettings, TimeFormat};
use baudbound_storage::{
    GeneratedTriggerToken, NetworkTriggerType, ScriptStore, ScriptUpdateState, SqliteRunnerStore,
    StoredRunRecord, TriggerAuthStatus,
};
use baudbound_triggers::{SerialPortRebindSink, WebSocketConnectionRegistry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{Emitter, Manager, State};

use crate::commands::{
    doctor::{DoctorCheck, desktop_doctor_checks},
    service_health::service_health_document,
};
use crate::desktop_actions::SystemDesktopActionAdapter;
use crate::service::{ServeOptions, ServeOverrides, validate_serve_start};
use crate::trigger_monitor::TriggerMonitor;

mod active_runs;
mod background;
mod command_guard;
mod coordinate_picker;
mod desktop_config;
mod history;
mod lifecycle;
mod manual_runs;
mod repositories;
mod secret_vault;
mod tools;

use active_runs::{ActiveRunRegistry, ActiveRunSnapshot};
use background::{DesktopRunnerSnapshot, DesktopRunnerSupervisor};
use command_guard::{
    SensitiveOperation, SensitiveOperationGuard, ensure_main_window, ensure_main_window_source,
};
macro_rules! desktop_command_handler {
    () => {
        tauri::generate_handler![
            approve_script,
            repositories::add_script_repository,
            clear_run_history,
            clear_run_logs,
            check_script_update,
            check_script_updates,
            cancel_remote_script_package_preparation,
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
            import_script_package,
            install_remote_script_package,
            discard_remote_package_review,
            prepare_remote_script_package,
            prepare_discovered_script_update,
            repositories::prepare_repository_script,
            repositories::preview_script_repository,
            prepare_for_update,
            remove_script,
            repositories::remove_script_repository,
            repositories::repository_sources,
            repositories::repository_script_filter_options,
            repositories::repository_script_details,
            repositories::query_repository_scripts,
            repositories::refresh_all_script_repositories,
            repositories::refresh_script_repository,
            revoke_script_approval,
            reload_background_runner,
            retry_secret_vault,
            read_runner_config,
            manual_runs::run_script,
            manual_runs::stop_run,
            manual_runs::stop_manual_script_runs,
            manual_runs::stop_script_runs,
            save_runner_config,
            save_runner_config_model,
            coordinate_picker::select_coordinate_picker,
            tools::scan_serial_ports,
            set_script_secret,
            remove_script_secret,
            reset_runner_config,
            rotate_network_trigger_token,
            select_package_file,
            set_script_enabled,
            set_script_automatic_update_checks,
            repositories::set_script_repository_enabled,
            set_network_trigger_auth_enabled,
            start_background_runner,
            coordinate_picker::start_coordinate_picker,
            stop_background_runner,
            should_check_for_update,
            start_trigger_monitor,
            stop_trigger_monitor,
            clear_trigger_monitor,
            trigger_monitor_state,
            record_update_check,
            update_script_package,
        ]
    };
}

pub fn run_desktop_ui(
    config_path: PathBuf,
    core: RunnerCore,
    store: SqliteRunnerStore,
    runner_config: RunnerConfig,
    websocket_registry: Arc<WebSocketConnectionRegistry>,
    launched_from_autostart: bool,
) -> Result<()> {
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
    let secret_vault = secret_vault::SecretVaultController::default();
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
            desktop_config::reconcile_autostart_registration(app.handle());
            lifecycle::configure_desktop_lifecycle(app, launched_from_autostart)?;
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
            desktop_config::start_configured_background_runner(app.handle());
            if let Some(window) = app.get_webview_window("main") {
                window
                    .set_title("BaudBound")
                    .map_err(|source| anyhow!("failed to set window title: {source}"))?;
            }
            Ok(())
        })
        .invoke_handler(desktop_command_handler!())
        .run(tauri::generate_context!())
        .map_err(|source| anyhow!("desktop UI failed: {source}"))
}

pub(super) struct DesktopUiState {
    active_runs: Arc<ActiveRunRegistry>,
    background_options: Mutex<ServeOptions>,
    background_runner: DesktopRunnerSupervisor,
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
async fn check_script_update<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    reference: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    let store = state.store.clone();
    let package_limit = current_runner_config(&state)
        .map_err(|error| error.to_string())?
        .limits
        .max_file_download_bytes as u64;
    let reference_for_check = reference.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::script_updates::check_script_update(&store, package_limit, &reference_for_check)
    })
    .await
    .map_err(|error| format!("update check task failed: {error}"))?;
    if let Err(error) = app.emit(crate::script_updates::SCRIPT_UPDATE_EVENT, &reference) {
        tracing::debug!(%error, "failed to publish manual script update state event");
    }
    result.map_err(|error| error.to_string())?;
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload {
        dashboard,
        message: format!("Checked {reference} for updates."),
    })
}

#[derive(Serialize)]
struct ScriptUpdateBatchPayload {
    dashboard: DashboardPayload,
    errors: BTreeMap<String, String>,
}

#[tauri::command]
async fn check_script_updates<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    references: Vec<String>,
    state: State<'_, DesktopUiState>,
) -> Result<ScriptUpdateBatchPayload, String> {
    if references.is_empty() || references.len() > 1_000 {
        return Err("select between 1 and 1,000 scripts to check".to_owned());
    }
    let store = state.store.clone();
    let package_limit = current_runner_config(&state)
        .map_err(|error| error.to_string())?
        .limits
        .max_file_download_bytes as u64;
    let references_for_check = references.clone();
    let results = tauri::async_runtime::spawn_blocking(move || {
        crate::script_updates::check_script_updates(&store, package_limit, &references_for_check)
    })
    .await
    .map_err(|error| format!("update check task failed: {error}"))?;
    for reference in &references {
        if let Err(error) = app.emit(crate::script_updates::SCRIPT_UPDATE_EVENT, reference) {
            tracing::debug!(%error, "failed to publish batch script update state event");
        }
    }
    let errors = results
        .into_iter()
        .filter_map(|(script_id, result)| result.err().map(|error| (script_id, error)))
        .collect();
    Ok(ScriptUpdateBatchPayload {
        dashboard: build_dashboard_payload(&state).map_err(|error| error.to_string())?,
        errors,
    })
}

#[tauri::command]
async fn select_package_file<R: tauri::Runtime>(
    operation: PackageFileOperation,
    guard: State<'_, SensitiveOperationGuard>,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<Option<PackageFileSelection>, String> {
    ensure_main_window(&window)?;
    let selected = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .add_filter("BaudBound package", &["bbs"])
        .pick_file()
        .await;

    selected
        .map(|file| {
            let package_path = file
                .path()
                .to_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| "the selected package path is not valid UTF-8".to_owned())?;
            let sensitive_operation = operation.sensitive_operation(package_path.clone());
            let challenge = guard.prepare_package_selection(&sensitive_operation, &state)?;
            Ok(PackageFileSelection {
                confirmation_id: challenge.into_confirmation_id(),
                package_path,
            })
        })
        .transpose()
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PackageFileOperation {
    Import,
    Update,
}

impl PackageFileOperation {
    fn sensitive_operation(self, package_path: String) -> SensitiveOperation {
        match self {
            Self::Import => SensitiveOperation::ImportScriptPackage { package_path },
            Self::Update => SensitiveOperation::UpdateScriptPackage { package_path },
        }
    }
}

#[derive(Serialize)]
struct PackageFileSelection {
    confirmation_id: String,
    package_path: String,
}

#[tauri::command]
fn approve_script<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    reference: String,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<PackageActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::ApproveScript {
            reference: reference.clone(),
        },
        &guard,
        &state,
        &window,
    )?;
    let result = run_locked_value(&state, || {
        Ok(current_core(&state)?.approve_installed(&state.store, &reference)?)
    })?;
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(PackageActionPayload {
        dashboard,
        generated_trigger_tokens: result.generated_trigger_tokens,
        message: format!("Approved {reference}."),
    })
}

#[tauri::command]
fn revoke_script_approval(
    reference: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    run_locked_action(&state, || {
        let revoked = current_core(&state)?.revoke_approval(&state.store, &reference)?;
        Ok(if revoked.is_some() {
            format!("Revoked approval for {reference}.")
        } else {
            format!("No approval was stored for {reference}.")
        })
    })
}

#[tauri::command]
fn import_script_package<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    package_path: String,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_package_selection(
        &confirmation_id,
        &SensitiveOperation::ImportScriptPackage {
            package_path: package_path.clone(),
        },
        &guard,
        &state,
        &window,
    )?;
    let path = PathBuf::from(package_path);
    let script = run_locked_value(&state, || {
        Ok(current_core(&state)?.import_package(&state.store, &path)?)
    })?;
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload {
        dashboard,
        message: format!(
            "Imported {} ({}) as {}.",
            script.name, script.id, script.package_file_name
        ),
    })
}

#[tauri::command]
fn update_script_package<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    package_path: String,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_package_selection(
        &confirmation_id,
        &SensitiveOperation::UpdateScriptPackage {
            package_path: package_path.clone(),
        },
        &guard,
        &state,
        &window,
    )?;
    let path = PathBuf::from(package_path);
    let has_repository_url = !baudbound_script::load_script_package(&path)
        .map_err(|error| error.to_string())?
        .manifest
        .repository_url
        .trim()
        .is_empty();
    let script = run_locked_value(&state, || {
        let script = current_core(&state)?.update_package(&state.store, &path)?;
        crate::script_updates::reconcile_script_update_state_after_install(
            &state.store,
            &script.id,
            has_repository_url,
        )?;
        Ok(script)
    })?;
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload {
        dashboard,
        message: format!(
            "Updated {} ({}) as {}.",
            script.name, script.id, script.package_file_name
        ),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrepareRemotePackageRequest {
    operation: crate::script_updates::RemotePackageOperation,
    request_id: String,
    source: crate::script_updates::RemotePackageSource,
    url: String,
}

#[derive(Serialize)]
struct RemotePackageReviewPayload {
    review_id: String,
    #[serde(flatten)]
    review: crate::script_updates::RemotePackageReview,
}

const REMOTE_PACKAGE_PROGRESS_EVENT: &str = "runner-remote-package-progress";

#[derive(Clone, Serialize)]
struct RemotePackageProgressPayload {
    request_id: String,
    #[serde(flatten)]
    progress: crate::script_updates::RemotePreparationProgress,
}

#[tauri::command]
async fn prepare_remote_script_package<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    preparations: State<'_, crate::script_updates::RemotePreparationRegistry>,
    request: PrepareRemotePackageRequest,
    reviews: State<'_, crate::script_updates::RemotePackageReviews>,
    state: State<'_, DesktopUiState>,
) -> Result<RemotePackageReviewPayload, String> {
    let preparation = preparations.start(&request.request_id)?;
    let request_id = request.request_id.clone();
    let core = current_core(&state).map_err(|error| error.to_string())?;
    let store = state.store.clone();
    let package_limit = current_runner_config(&state)
        .map_err(|error| error.to_string())?
        .limits
        .max_file_download_bytes as u64;
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        let mut progress = |progress| {
            if preparation.is_cancelled() {
                return false;
            }
            let _ = app.emit(
                REMOTE_PACKAGE_PROGRESS_EVENT,
                RemotePackageProgressPayload {
                    request_id: request_id.clone(),
                    progress,
                },
            );
            !preparation.is_cancelled()
        };
        crate::script_updates::prepare_remote_package_with_progress(
            &core,
            &store,
            package_limit,
            request.operation,
            request.source,
            &request.url,
            &mut progress,
        )
    })
    .await
    .map_err(|error| format!("remote package preparation task failed: {error}"))?
    .map_err(|error| error.to_string())?;
    let review = prepared.review.clone();
    let review_id = reviews.insert(prepared)?;
    Ok(RemotePackageReviewPayload { review_id, review })
}

#[tauri::command]
async fn prepare_discovered_script_update<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    preparations: State<'_, crate::script_updates::RemotePreparationRegistry>,
    reference: String,
    request_id: String,
    reviews: State<'_, crate::script_updates::RemotePackageReviews>,
    state: State<'_, DesktopUiState>,
) -> Result<RemotePackageReviewPayload, String> {
    let preparation = preparations.start(&request_id)?;
    let core = current_core(&state).map_err(|error| error.to_string())?;
    let store = state.store.clone();
    let package_limit = current_runner_config(&state)
        .map_err(|error| error.to_string())?
        .limits
        .max_file_download_bytes as u64;
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        let mut progress = |progress| {
            if preparation.is_cancelled() {
                return false;
            }
            let _ = app.emit(
                REMOTE_PACKAGE_PROGRESS_EVENT,
                RemotePackageProgressPayload {
                    request_id: request_id.clone(),
                    progress,
                },
            );
            !preparation.is_cancelled()
        };
        crate::script_updates::prepare_discovered_update_with_progress(
            &core,
            &store,
            package_limit,
            &reference,
            &mut progress,
        )
    })
    .await
    .map_err(|error| format!("remote update preparation task failed: {error}"))?
    .map_err(|error| error.to_string())?;
    let review = prepared.review.clone();
    let review_id = reviews.insert(prepared)?;
    Ok(RemotePackageReviewPayload { review_id, review })
}

#[tauri::command]
fn cancel_remote_script_package_preparation(
    request_id: String,
    preparations: State<'_, crate::script_updates::RemotePreparationRegistry>,
) -> Result<bool, String> {
    preparations.cancel(&request_id)
}

#[tauri::command]
fn discard_remote_package_review(
    review_id: String,
    reviews: State<'_, crate::script_updates::RemotePackageReviews>,
) -> Result<bool, String> {
    reviews.discard(&review_id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallRemoteScriptPackageRequest {
    review_id: String,
    sha256: String,
}

#[tauri::command]
fn install_remote_script_package<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    request: InstallRemoteScriptPackageRequest,
    reviews: State<'_, crate::script_updates::RemotePackageReviews>,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    let InstallRemoteScriptPackageRequest { review_id, sha256 } = request;
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::InstallRemoteScriptPackage {
            review_id: review_id.clone(),
            sha256: sha256.clone(),
        },
        &guard,
        &state,
        &window,
    )?;
    let prepared = reviews.take(&review_id, &sha256)?;
    let actual_hash = sha256_path(prepared.download.file.path())?;
    if actual_hash != sha256 {
        return Err("the downloaded package changed after review".to_owned());
    }
    let operation = prepared.review.operation;
    let script_id = prepared.review.script_id.clone();
    let script_name = prepared.review.script_name.clone();
    let has_repository_url = !prepared.review.repository_url.trim().is_empty();
    run_locked_value(&state, || {
        let directory = tempfile::Builder::new()
            .prefix("baudbound-reviewed-package-")
            .tempdir()?;
        let package_path = directory.path().join(format!("{script_id}.bbs"));
        fs::copy(prepared.download.file.path(), &package_path)?;
        match operation {
            crate::script_updates::RemotePackageOperation::Import => {
                current_core(&state)?.import_package(&state.store, &package_path)?;
            }
            crate::script_updates::RemotePackageOperation::Update => {
                current_core(&state)?.update_package(&state.store, &package_path)?;
                crate::script_updates::reconcile_script_update_state_after_install(
                    &state.store,
                    &script_id,
                    has_repository_url,
                )?;
            }
        }
        Ok(())
    })?;
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    if let Err(error) = app.emit(repositories::REPOSITORY_CHANGED_EVENT, &script_id) {
        tracing::warn!(%error, "failed to publish repository script change event");
    }
    Ok(ActionPayload {
        dashboard,
        message: format!(
            "{} {script_name}. Review and approve the installed package before running it.",
            match operation {
                crate::script_updates::RemotePackageOperation::Import => "Imported",
                crate::script_updates::RemotePackageOperation::Update => "Updated",
            }
        ),
    })
}

fn sha256_path(path: &std::path::Path) -> Result<String, String> {
    use sha2::{Digest as _, Sha256};
    use std::io::Read as _;

    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
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
fn prepare_for_update(state: State<'_, DesktopUiState>) -> Result<ActionPayload, String> {
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

#[tauri::command]
fn remove_script(
    reference: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    run_locked_action(&state, || {
        let script = current_core(&state)?.remove_installed(&state.store, &reference)?;
        Ok(format!("Removed {} ({}).", script.name, script.id))
    })
}

#[tauri::command]
fn clear_run_history<R: tauri::Runtime>(
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
fn clear_run_logs<R: tauri::Runtime>(
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
fn set_script_enabled(
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
fn set_script_automatic_update_checks<R: tauri::Runtime>(
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

#[tauri::command]
fn rotate_network_trigger_token<R: tauri::Runtime>(
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
fn set_network_trigger_auth_enabled<R: tauri::Runtime>(
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
struct SetNetworkTriggerAuthEnabledRequest {
    enabled: bool,
    node_id: String,
    reference: String,
    trigger_type: NetworkTriggerType,
}

#[tauri::command]
fn set_script_secret<R: tauri::Runtime>(
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
fn remove_script_secret<R: tauri::Runtime>(
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
fn read_runner_config(
    autostart: State<'_, tauri_plugin_autostart::AutoLaunchManager>,
    state: State<'_, DesktopUiState>,
) -> Result<RunnerConfigPayload, String> {
    read_runner_config_payload(&autostart, &state).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_runner_config<R: tauri::Runtime>(
    autostart: State<'_, tauri_plugin_autostart::AutoLaunchManager>,
    confirmation_id: String,
    contents: String,
    guard: State<'_, SensitiveOperationGuard>,
    restart_background: bool,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::SaveRunnerConfig {
            contents: contents.clone(),
            restart_background,
        },
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        save_runner_config_contents(&autostart, &state, &contents, restart_background)
    })
}

#[tauri::command]
fn save_runner_config_model<R: tauri::Runtime>(
    autostart: State<'_, tauri_plugin_autostart::AutoLaunchManager>,
    confirmation_id: String,
    config: RunnerConfig,
    guard: State<'_, SensitiveOperationGuard>,
    restart_background: bool,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::SaveRunnerConfigModel {
            config: config.clone(),
            restart_background,
        },
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        save_runner_config_model_contents(&autostart, &state, config, restart_background)
    })
}

#[tauri::command]
fn reset_runner_config<R: tauri::Runtime>(
    autostart: State<'_, tauri_plugin_autostart::AutoLaunchManager>,
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    restart_background: bool,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::ResetRunnerConfig { restart_background },
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        save_valid_runner_config(
            &autostart,
            &state,
            RunnerConfig::template_toml(),
            restart_background,
            ConfigWriteOperation::Reset,
        )
    })
}

pub(super) fn start_background_runner_message(state: &DesktopUiState) -> Result<String> {
    let (core, options) = current_runtime(state)?;
    validate_serve_start(&core, &state.store, &options)?;
    state
        .background_runner
        .start(core, state.store.clone(), options)
}

pub(super) fn reload_background_runner_message(state: &DesktopUiState) -> Result<String> {
    if !state.background_runner.snapshot()?.running {
        return Ok("Desktop background runner is not running.".to_owned());
    }
    request_running_service_reload(&state.store)?;
    Ok("Requested desktop background runner reload.".to_owned())
}

pub(super) fn stop_background_runner_message(state: &DesktopUiState) -> Result<String> {
    state.background_runner.stop()
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

fn build_dashboard_payload(state: &DesktopUiState) -> Result<DashboardPayload> {
    sync_runtime_config_from_disk(state)?;
    let runner = current_core(state)?.status(&state.store)?;
    let core = current_core(state)?;
    let secret_statuses = runner
        .scripts
        .iter()
        .filter_map(|script| {
            core.list_installed_secrets(&state.store, &script.installed.id)
                .ok()
                .map(|secrets| (script.installed.id.clone(), secrets))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let trigger_auth_statuses = runner
        .scripts
        .iter()
        .map(|script| {
            core.list_trigger_auth(&state.store, &script.installed.id)
                .map(|statuses| (script.installed.id.clone(), statuses))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;
    let recent_runs = state.store.list_run_records(None, Some(50))?;
    let run_statistics = state.store.run_statistics()?;
    let script_updates =
        script_update_payloads(&runner.scripts, state.store.list_script_update_states()?);
    let active_runs = state.active_runs.snapshot();
    let desktop_background = state.background_runner.snapshot()?;
    let runner_config = current_runner_config(state)?;
    let desktop_background_start_blocker = if desktop_background.running {
        None
    } else {
        let start_options = state
            .background_options
            .lock()
            .map_err(|_| anyhow!("desktop background options lock is poisoned"))?
            .clone();
        validate_serve_start(&core, &state.store, &start_options)
            .err()
            .map(|error| error.to_string())
    };
    let serial_devices = serial_device_payloads(&runner_config);
    let service_status = state.store.read_service_status()?;
    let service_health = service_health_document(service_status.as_ref());
    let mut public_service_status = service_status;
    if let Some(status) = public_service_status.as_mut() {
        crate::service::redact_service_control(status);
    }
    let native_doctor_checks = desktop_doctor_checks();
    Ok(DashboardPayload {
        active_runs: active_runs.runs,
        active_runs_revision: active_runs.revision,
        desktop_background,
        desktop_background_start_blocker,
        desktop_platform: desktop_platform(),
        automatic_update_checks: runner_config.updates.automatic_checks,
        launch_at_login_desired: runner_config.desktop.launch_at_login,
        launch_at_login_registered: *state
            .login_startup_registered
            .lock()
            .map_err(|_| anyhow!("login startup state lock is poisoned"))?,
        native_doctor_checks,
        recent_runs,
        run_statistics,
        runner,
        secret_vault: state.secret_vault.snapshot(),
        secret_statuses,
        script_updates,
        serial_devices,
        service_health,
        service_status: public_service_status,
        config_path: state.config_path.display().to_string(),
        storage_root: state.store.root().display().to_string(),
        time_format: runner_config.display.time_format,
        trigger_auth_statuses,
    })
}

fn request_running_service_reload(store: &SqliteRunnerStore) -> Result<()> {
    let status = store
        .read_service_status()?
        .ok_or_else(|| anyhow!("runner service is not running"))?;
    if status.get("state").and_then(Value::as_str) != Some("running") {
        return Err(anyhow!("runner service is not running"));
    }
    crate::service::request_service_control(&status, crate::service::ServiceControlCommand::Reload)
}

#[derive(Serialize)]
struct DashboardPayload {
    active_runs: Vec<ActiveRunSnapshot>,
    active_runs_revision: u64,
    automatic_update_checks: bool,
    config_path: String,
    desktop_background: DesktopRunnerSnapshot,
    desktop_background_start_blocker: Option<String>,
    desktop_platform: &'static str,
    launch_at_login_desired: bool,
    launch_at_login_registered: Option<bool>,
    native_doctor_checks: Vec<DoctorCheck>,
    recent_runs: Vec<StoredRunRecord>,
    run_statistics: baudbound_storage::RunStatistics,
    runner: baudbound_core::RunnerStatus,
    secret_vault: secret_vault::SecretVaultSnapshot,
    secret_statuses: std::collections::BTreeMap<String, Vec<baudbound_core::InstalledSecretStatus>>,
    script_updates: std::collections::BTreeMap<String, ScriptUpdatePayload>,
    serial_devices: Vec<SerialDevicePayload>,
    service_health: Value,
    service_status: Option<Value>,
    storage_root: String,
    time_format: TimeFormat,
    trigger_auth_statuses: std::collections::BTreeMap<String, Vec<TriggerAuthStatus>>,
}

#[derive(Serialize)]
struct ScriptUpdatePayload {
    #[serde(flatten)]
    state: ScriptUpdateState,
    status: &'static str,
}

fn script_update_payloads(
    scripts: &[baudbound_core::ScriptStatus],
    states: Vec<ScriptUpdateState>,
) -> std::collections::BTreeMap<String, ScriptUpdatePayload> {
    let mut states = states
        .into_iter()
        .map(|state| (state.script_id.clone(), state))
        .collect::<std::collections::BTreeMap<_, _>>();
    scripts
        .iter()
        .map(|script| {
            let id = script.installed.id.clone();
            let state = states
                .remove(&id)
                .unwrap_or_else(|| ScriptUpdateState::empty(id.clone()));
            let status = script_update_status(script, &state);
            (id, ScriptUpdatePayload { state, status })
        })
        .collect()
}

fn script_update_status(
    script: &baudbound_core::ScriptStatus,
    state: &ScriptUpdateState,
) -> &'static str {
    let Some(metadata) = script.metadata.as_ref() else {
        return "unavailable";
    };
    let repository_url = metadata.repository_url.trim();
    if repository_url.is_empty() {
        return "unconfigured";
    }
    if state.checked_repository_url.as_deref() != Some(repository_url) {
        return "not_checked";
    }
    if state.last_error.is_some() {
        return "failed";
    }
    let (Ok(current), Some(Ok(latest))) = (
        semver::Version::parse(&metadata.version),
        state.latest_version.as_deref().map(semver::Version::parse),
    ) else {
        return "unavailable";
    };
    if latest > current {
        "available"
    } else {
        "up_to_date"
    }
}

fn desktop_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unsupported"
    }
}

#[derive(Serialize)]
struct SerialDevicePayload {
    auto_reconnect: bool,
    auto_rebind_port: bool,
    baud_rate: u32,
    data_bits: u8,
    device_id: String,
    dtr_on_open: String,
    flow_control: String,
    manufacturer: Option<String>,
    max_message_bytes: usize,
    message_gap_ms: u64,
    open_stabilization_ms: u64,
    parity: String,
    port: String,
    product_id: Option<String>,
    product: Option<String>,
    read_mode: String,
    serial_number: Option<String>,
    stop_bits: String,
    validate_usb_identity: bool,
    vendor_id: Option<String>,
}

#[derive(Serialize)]
struct ActionPayload {
    dashboard: DashboardPayload,
    message: String,
}

#[derive(Serialize)]
struct PackageActionPayload {
    dashboard: DashboardPayload,
    generated_trigger_tokens: Vec<GeneratedTriggerToken>,
    message: String,
}

#[derive(Serialize)]
struct GeneratedTriggerTokenPayload {
    dashboard: DashboardPayload,
    message: String,
    status: TriggerAuthStatus,
    token: String,
}

fn trigger_type_label(trigger_type: &NetworkTriggerType) -> &'static str {
    match trigger_type {
        NetworkTriggerType::Webhook => "webhook",
        NetworkTriggerType::Websocket => "WebSocket",
    }
}

#[derive(Serialize)]
struct RunnerConfigPayload {
    config: RunnerConfig,
    contents: String,
    launch_at_login_registered: bool,
    path: String,
}

fn read_runner_config_payload(
    autostart: &tauri_plugin_autostart::AutoLaunchManager,
    state: &DesktopUiState,
) -> Result<RunnerConfigPayload> {
    let contents = fs::read_to_string(&state.config_path)?;
    let config = RunnerConfig::from_toml(&contents, &state.config_path)?;
    Ok(RunnerConfigPayload {
        config,
        contents,
        launch_at_login_registered: desktop_config::autostart_registration(autostart)?,
        path: state.config_path.display().to_string(),
    })
}

fn save_runner_config_contents(
    autostart: &tauri_plugin_autostart::AutoLaunchManager,
    state: &DesktopUiState,
    contents: &str,
    restart_background: bool,
) -> Result<String> {
    save_valid_runner_config(
        autostart,
        state,
        contents,
        restart_background,
        ConfigWriteOperation::Save,
    )
}

fn save_runner_config_model_contents(
    autostart: &tauri_plugin_autostart::AutoLaunchManager,
    state: &DesktopUiState,
    config: RunnerConfig,
    restart_background: bool,
) -> Result<String> {
    let contents = config.to_pretty_toml()?;
    save_valid_runner_config(
        autostart,
        state,
        &contents,
        restart_background,
        ConfigWriteOperation::Save,
    )
}

#[derive(Clone, Copy)]
enum ConfigWriteOperation {
    Save,
    Reset,
}

impl ConfigWriteOperation {
    fn success_message(self, restarted: bool, restart_required: bool) -> &'static str {
        match (self, restarted, restart_required) {
            (Self::Save, true, _) => {
                "Saved runner config and restarted the desktop background runner."
            }
            (Self::Save, false, true) => {
                "Saved runner config. Restart the desktop background runner to apply listener changes."
            }
            (Self::Save, false, false) => "Saved runner config.",
            (Self::Reset, true, _) => {
                "Reset runner config to defaults and restarted the desktop background runner."
            }
            (Self::Reset, false, true) => {
                "Reset runner config to defaults. Restart the desktop background runner to apply listener changes."
            }
            (Self::Reset, false, false) => "Reset runner config to defaults.",
        }
    }
}

fn save_valid_runner_config(
    autostart: &tauri_plugin_autostart::AutoLaunchManager,
    state: &DesktopUiState,
    contents: &str,
    restart_background: bool,
    operation: ConfigWriteOperation,
) -> Result<String> {
    const BACKGROUND_RESTART_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

    let next_config = RunnerConfig::from_toml(contents, &state.config_path)?;
    let previous_contents = fs::read_to_string(&state.config_path)?;
    let previous_config = current_runner_config(state)?;
    let runtime_changed = runner_runtime_config_changed(&previous_config, &next_config);
    let previous_registration = desktop_config::autostart_registration(autostart)?;
    let was_running = state.background_runner.snapshot()?.running;
    let restart_runtime = restart_background && runtime_changed && was_running;

    if restart_runtime {
        state
            .background_runner
            .stop_and_wait(BACKGROUND_RESTART_TIMEOUT)?;
    }

    let apply_result = (|| -> Result<()> {
        desktop_config::set_autostart_registration(autostart, next_config.desktop.launch_at_login)?;
        desktop_config::remember_autostart_registration(state, autostart);
        RunnerConfig::write_atomic(&state.config_path, contents)?;
        replace_runtime_config(state, next_config)
    })();
    if let Err(error) = apply_result {
        let config_rollback = RunnerConfig::write_atomic(&state.config_path, &previous_contents);
        let runtime_rollback = replace_runtime_config(state, previous_config.clone());
        let autostart_rollback =
            desktop_config::set_autostart_registration(autostart, previous_registration);
        desktop_config::remember_autostart_registration(state, autostart);
        let runner_rollback = if restart_runtime {
            start_background_runner_message(state).map(|_| ())
        } else {
            Ok(())
        };
        return Err(anyhow!(
            "failed to apply saved config: {error}; file rollback: {}; runtime rollback: {}; login startup rollback: {}; background runner rollback: {}",
            rollback_result(config_rollback),
            rollback_result(runtime_rollback),
            rollback_result(autostart_rollback),
            rollback_result(runner_rollback)
        ));
    }

    state.repository_refresh_worker.wake();
    state.script_update_worker.wake();

    if restart_runtime {
        if let Err(error) = start_background_runner_message(state) {
            let config_rollback =
                RunnerConfig::write_atomic(&state.config_path, &previous_contents);
            let runtime_rollback = replace_runtime_config(state, previous_config);
            let autostart_rollback =
                desktop_config::set_autostart_registration(autostart, previous_registration);
            desktop_config::remember_autostart_registration(state, autostart);
            let runner_rollback = start_background_runner_message(state).map(|_| ());
            return Err(anyhow!(
                "failed to restart the desktop background runner with the saved config: {error}; file rollback: {}; runtime rollback: {}; login startup rollback: {}; background runner rollback: {}",
                rollback_result(config_rollback),
                rollback_result(runtime_rollback),
                rollback_result(autostart_rollback),
                rollback_result(runner_rollback)
            ));
        }
        return Ok(operation.success_message(true, false).to_owned());
    }

    if runtime_changed && was_running {
        return Ok(operation.success_message(false, true).to_owned());
    }

    Ok(operation.success_message(false, false).to_owned())
}

fn runner_runtime_config_changed(previous: &RunnerConfig, next: &RunnerConfig) -> bool {
    previous.runner != next.runner
        || previous.serial != next.serial
        || previous.triggers != next.triggers
        || previous.webhooks != next.webhooks
        || previous.websockets != next.websockets
}

fn rollback_result<T, E: std::fmt::Display>(result: std::result::Result<T, E>) -> String {
    match result {
        Ok(_) => "succeeded".to_owned(),
        Err(error) => format!("failed ({error})"),
    }
}

fn replace_runtime_config(state: &DesktopUiState, runner_config: RunnerConfig) -> Result<()> {
    state
        .store
        .set_run_retention_policy(baudbound_storage::RunRetentionPolicy::new(
            runner_config.runner.run_history_max_records,
            runner_config.runner.run_history_max_age_days,
        ))?;
    let existing_core = current_core(state)?;
    let next_core = build_runner_core(
        &runner_config,
        Arc::clone(&state.websocket_registry),
        Arc::clone(&state.active_runs),
    )
    .with_execution_queue_from(&existing_core);
    let serial_connections = next_core.serial_connections();
    let next_background_options = desktop_background_options(
        &runner_config,
        Arc::clone(&state.websocket_registry),
        state.config_path.clone(),
        serial_connections,
        state.trigger_monitor.clone(),
    );

    *state
        .runner_config
        .lock()
        .map_err(|_| anyhow!("runner config lock is poisoned"))? = runner_config;
    *state
        .core
        .lock()
        .map_err(|_| anyhow!("runner core lock is poisoned"))? = next_core;
    *state
        .background_options
        .lock()
        .map_err(|_| anyhow!("desktop background options lock is poisoned"))? =
        next_background_options;
    Ok(())
}

fn sync_runtime_config_from_disk(state: &DesktopUiState) -> Result<()> {
    let contents = fs::read_to_string(&state.config_path)?;
    let runner_config = RunnerConfig::from_toml(&contents, &state.config_path)?;
    replace_runtime_config(state, runner_config)
}

fn current_runner_config(state: &DesktopUiState) -> Result<RunnerConfig> {
    state
        .runner_config
        .lock()
        .map_err(|_| anyhow!("runner config lock is poisoned"))
        .map(|config| config.clone())
}

fn serial_device_payloads(config: &RunnerConfig) -> Vec<SerialDevicePayload> {
    config
        .serial
        .devices
        .iter()
        .map(|(device_id, settings)| serial_device_payload(device_id, settings))
        .collect()
}

fn serial_device_payload(device_id: &str, settings: &SerialDeviceSettings) -> SerialDevicePayload {
    SerialDevicePayload {
        auto_reconnect: settings.auto_reconnect,
        auto_rebind_port: settings.auto_rebind_port,
        baud_rate: settings.baud_rate,
        data_bits: settings.data_bits,
        device_id: device_id.to_owned(),
        dtr_on_open: settings.dtr_on_open.clone(),
        flow_control: settings.flow_control.clone(),
        manufacturer: settings.manufacturer.clone(),
        max_message_bytes: settings.max_message_bytes,
        message_gap_ms: settings.message_gap_ms,
        open_stabilization_ms: settings.open_stabilization_ms,
        parity: settings.parity.clone(),
        port: settings.port.clone(),
        product_id: settings.product_id.clone(),
        product: settings.product.clone(),
        read_mode: settings.read_mode.clone(),
        serial_number: settings.serial_number.clone(),
        stop_bits: settings.stop_bits.clone(),
        validate_usb_identity: settings.validate_usb_identity,
        vendor_id: settings.vendor_id.clone(),
    }
}

fn current_core(state: &DesktopUiState) -> Result<RunnerCore> {
    state
        .core
        .lock()
        .map_err(|_| anyhow!("runner core lock is poisoned"))
        .map(|core| core.clone())
}

fn current_runtime(state: &DesktopUiState) -> Result<(RunnerCore, ServeOptions)> {
    sync_runtime_config_from_disk(state)?;
    let core = current_core(state)?;
    let options = state
        .background_options
        .lock()
        .map_err(|_| anyhow!("desktop background options lock is poisoned"))?
        .clone();
    Ok((core, options))
}

fn build_runner_core(
    runner_config: &RunnerConfig,
    websocket_registry: Arc<WebSocketConnectionRegistry>,
    active_runs: Arc<ActiveRunRegistry>,
) -> RunnerCore {
    let core = RunnerCore::from_config(runner_config)
        .with_execution_mode(baudbound_core::RunnerExecutionMode::Desktop)
        .with_websocket_sink(websocket_registry)
        .with_run_observer(active_runs);
    let action_handler = Arc::new(DesktopActionHandler::new(
        core.headless_action_handler(),
        SystemDesktopActionAdapter::default(),
    ));
    core.with_action_handler(action_handler)
}

fn desktop_background_options(
    runner_config: &RunnerConfig,
    websocket_registry: Arc<WebSocketConnectionRegistry>,
    config_path: PathBuf,
    serial_connections: Arc<baudbound_actions::SerialConnectionRegistry>,
    trigger_monitor: TriggerMonitor,
) -> ServeOptions {
    ServeOptions::from_config(
        runner_config,
        ServeOverrides {
            hotkey_stdin: false,
            max_webhook_body_bytes: None,
            max_websocket_connections: None,
            max_websocket_message_bytes: None,
            reload_interval_seconds: None,
            webhook_bind: None,
            webhook_port: None,
            webhooks: false,
            websocket_bind: None,
            websocket_port: None,
            websockets: false,
        },
        false,
        false,
        websocket_registry,
    )
    .with_serial_connections(serial_connections)
    .with_trigger_monitor(trigger_monitor)
    .with_serial_port_rebind_sink(Arc::new(
        crate::service::RunnerConfigSerialPortRebindSink::new(config_path),
    ) as Arc<dyn SerialPortRebindSink>)
}

#[cfg(test)]
mod tests;
