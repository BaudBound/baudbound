use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Result, anyhow};
use baudbound_actions::DesktopActionHandler;
use baudbound_core::{RunnerConfig, RunnerCore, SerialDeviceSettings};
use baudbound_storage::{DesktopSettings, ScriptStore, SqliteRunnerStore, StoredRunRecord};
use baudbound_triggers::{SerialPortRebindSink, WebSocketConnectionRegistry};
use serde::Serialize;
use serde_json::Value;
use serialport::SerialPortType;
use tauri::{Manager, State};

use crate::commands::{
    doctor::{DoctorCheck, desktop_doctor_checks},
    service_health::service_health_document,
};
use crate::desktop_actions::SystemDesktopActionAdapter;
use crate::service::{ServeOptions, ServeOverrides};

mod background;
mod lifecycle;
mod settings;

use background::{DesktopRunnerSnapshot, DesktopRunnerSupervisor};
use settings::{DesktopSettingsPayload, SettingsActionPayload};

macro_rules! desktop_command_handler {
    () => {
        tauri::generate_handler![
            approve_script,
            dashboard_state,
            read_desktop_settings,
            import_script_package,
            prepare_for_update,
            remove_script,
            revoke_script_approval,
            reload_background_runner,
            read_runner_config,
            run_script,
            save_runner_config,
            save_runner_config_model,
            save_desktop_settings,
            scan_serial_ports,
            set_script_secret,
            remove_script_secret,
            select_package_file,
            set_script_enabled,
            start_background_runner,
            stop_background_runner,
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
    let background_options = desktop_background_options(
        &runner_config,
        Arc::clone(&websocket_registry),
        config_path.clone(),
    );
    let background_runner = DesktopRunnerSupervisor::default();
    let desktop_settings = store
        .read_desktop_settings()
        .map_err(|error| anyhow!("failed to load desktop settings: {error}"))?;
    tauri::Builder::default()
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("BaudBound")
                .args(["ui", "--autostart"])
                .build(),
        )
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(DesktopUiState {
            background_options: Mutex::new(background_options),
            background_runner: background_runner.clone(),
            desktop_settings: Mutex::new(desktop_settings),
            config_path,
            runner_config: Mutex::new(runner_config),
            core: Mutex::new(core),
            store,
            websocket_registry,
            operation_lock: Mutex::new(()),
        })
        .setup(move |app| {
            settings::reconcile_autostart_registration(app.handle());
            lifecycle::configure_desktop_lifecycle(app, launched_from_autostart)?;
            settings::start_configured_background_runner(app.handle());
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
    background_options: Mutex<ServeOptions>,
    background_runner: DesktopRunnerSupervisor,
    desktop_settings: Mutex<DesktopSettings>,
    config_path: PathBuf,
    runner_config: Mutex<RunnerConfig>,
    core: Mutex<RunnerCore>,
    store: SqliteRunnerStore,
    websocket_registry: Arc<WebSocketConnectionRegistry>,
    operation_lock: Mutex<()>,
}

#[tauri::command]
fn read_desktop_settings(
    autostart: State<'_, tauri_plugin_autostart::AutoLaunchManager>,
    state: State<'_, DesktopUiState>,
) -> Result<DesktopSettingsPayload, String> {
    settings::read_settings_payload(&autostart, &state).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_desktop_settings(
    autostart: State<'_, tauri_plugin_autostart::AutoLaunchManager>,
    settings: DesktopSettings,
    state: State<'_, DesktopUiState>,
) -> Result<SettingsActionPayload, String> {
    settings::save_settings(&autostart, &state, settings).map_err(|error| error.to_string())
}

#[tauri::command]
fn dashboard_state(state: State<'_, DesktopUiState>) -> Result<DashboardPayload, String> {
    build_dashboard_payload(&state).map_err(|error| error.to_string())
}

#[tauri::command]
fn select_package_file() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("BaudBound package", &["bbs"])
        .pick_file()
        .map(|path| path.display().to_string())
}

#[tauri::command]
fn scan_serial_ports() -> Result<Vec<SerialPortScanPayload>, String> {
    serialport::available_ports()
        .map_err(|source| format!("failed to scan serial ports: {source}"))
        .map(|ports| {
            ports
                .into_iter()
                .map(serial_port_scan_payload)
                .collect::<Vec<_>>()
        })
}

#[tauri::command]
fn approve_script(
    reference: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    run_locked_action(&state, || {
        current_core(&state)?.approve_installed(&state.store, &reference)?;
        Ok(format!("Approved {reference}."))
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
fn import_script_package(
    package_path: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    let path = PathBuf::from(package_path);
    run_locked_action(&state, || {
        let script = current_core(&state)?.import_package(&state.store, &path)?;
        Ok(format!(
            "Imported {} ({}) as {}.",
            script.name, script.id, script.package_file_name
        ))
    })
}

#[tauri::command]
fn update_script_package(
    package_path: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    let path = PathBuf::from(package_path);
    run_locked_action(&state, || {
        let script = current_core(&state)?.update_package(&state.store, &path)?;
        Ok(format!(
            "Updated {} ({}) as {}.",
            script.name, script.id, script.package_file_name
        ))
    })
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
fn run_script(
    reference: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    run_locked_action(&state, || {
        let report = current_core(&state)?.run_installed(&state.store, &reference)?;
        Ok(format!(
            "Run {} completed for {reference}.",
            report.identity.run_id
        ))
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
fn set_script_secret(
    reference: String,
    name: String,
    value: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
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
fn remove_script_secret(
    reference: String,
    name: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
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
fn read_runner_config(state: State<'_, DesktopUiState>) -> Result<RunnerConfigPayload, String> {
    read_runner_config_payload(&state).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_runner_config(
    contents: String,
    restart_background: bool,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    run_locked_action(&state, || {
        save_runner_config_contents(&state, &contents, restart_background)
    })
}

#[tauri::command]
fn save_runner_config_model(
    config: RunnerConfig,
    restart_background: bool,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    run_locked_action(&state, || {
        save_runner_config_model_contents(&state, config, restart_background)
    })
}

pub(super) fn start_background_runner_message(state: &DesktopUiState) -> Result<String> {
    let (core, options) = current_runtime(state)?;
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
    let recent_runs = state.store.list_run_records(None, Some(50))?;
    let desktop_background = state.background_runner.snapshot()?;
    let serial_devices = serial_device_payloads(&current_runner_config(state)?);
    let service_status = state.store.read_service_status()?;
    let service_health = service_health_document(service_status.as_ref());
    let mut public_service_status = service_status;
    if let Some(status) = public_service_status.as_mut() {
        crate::service::redact_service_control(status);
    }
    let native_doctor_checks = desktop_doctor_checks();
    Ok(DashboardPayload {
        desktop_background,
        native_doctor_checks,
        recent_runs,
        runner,
        secret_statuses,
        serial_devices,
        service_health,
        service_status: public_service_status,
        config_path: state.config_path.display().to_string(),
        storage_root: state.store.root().display().to_string(),
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
    config_path: String,
    desktop_background: DesktopRunnerSnapshot,
    native_doctor_checks: Vec<DoctorCheck>,
    recent_runs: Vec<StoredRunRecord>,
    runner: baudbound_core::RunnerStatus,
    secret_statuses: std::collections::BTreeMap<String, Vec<baudbound_core::InstalledSecretStatus>>,
    serial_devices: Vec<SerialDevicePayload>,
    service_health: Value,
    service_status: Option<Value>,
    storage_root: String,
}

#[derive(Serialize)]
struct SerialDevicePayload {
    auto_reconnect: bool,
    auto_rebind_port: bool,
    baud_rate: u32,
    data_bits: u8,
    device_id: String,
    flow_control: String,
    manufacturer: Option<String>,
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
struct SerialPortScanPayload {
    manufacturer: Option<String>,
    port: String,
    port_type: String,
    product: Option<String>,
    product_id: Option<String>,
    serial_number: Option<String>,
    vendor_id: Option<String>,
}

#[derive(Serialize)]
struct ActionPayload {
    dashboard: DashboardPayload,
    message: String,
}

#[derive(Serialize)]
struct RunnerConfigPayload {
    config: RunnerConfig,
    contents: String,
    path: String,
}

fn read_runner_config_payload(state: &DesktopUiState) -> Result<RunnerConfigPayload> {
    let contents = fs::read_to_string(&state.config_path)?;
    let config = RunnerConfig::from_toml(&contents, &state.config_path)?;
    Ok(RunnerConfigPayload {
        config,
        contents,
        path: state.config_path.display().to_string(),
    })
}

fn save_runner_config_contents(
    state: &DesktopUiState,
    contents: &str,
    restart_background: bool,
) -> Result<String> {
    save_valid_runner_config(state, contents, restart_background)
}

fn save_runner_config_model_contents(
    state: &DesktopUiState,
    config: RunnerConfig,
    restart_background: bool,
) -> Result<String> {
    let contents = toml::to_string_pretty(&SerializableRunnerConfig::from(config))
        .map_err(|source| anyhow!("failed to serialize runner config: {source}"))?;
    save_valid_runner_config(state, &contents, restart_background)
}

fn save_valid_runner_config(
    state: &DesktopUiState,
    contents: &str,
    restart_background: bool,
) -> Result<String> {
    let next_config = RunnerConfig::from_toml(contents, &state.config_path)?;
    write_runner_config_file(&state.config_path, contents)?;
    replace_runtime_config(state, next_config)?;

    let was_running = state.background_runner.snapshot()?.running;
    if restart_background && was_running {
        state
            .background_runner
            .stop_and_wait(std::time::Duration::from_secs(2))?;
        let (core, options) = current_runtime(state)?;
        state
            .background_runner
            .start(core, state.store.clone(), options)?;
        return Ok("Saved runner config and restarted the desktop background runner.".to_owned());
    }

    if was_running {
        return Ok(
            "Saved runner config. Restart the desktop background runner to apply listener changes."
                .to_owned(),
        );
    }

    Ok("Saved runner config.".to_owned())
}

fn write_runner_config_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

#[derive(Serialize)]
struct SerializableRunnerConfig {
    runner: baudbound_core::RunnerSettings,
    serial: baudbound_core::SerialSettings,
    triggers: baudbound_core::TriggerSettings,
    webhooks: baudbound_core::WebhookSettings,
    websockets: baudbound_core::WebSocketSettings,
}

impl From<RunnerConfig> for SerializableRunnerConfig {
    fn from(config: RunnerConfig) -> Self {
        Self {
            runner: config.runner,
            serial: config.serial,
            triggers: config.triggers,
            webhooks: config.webhooks,
            websockets: config.websockets,
        }
    }
}

fn replace_runtime_config(state: &DesktopUiState, runner_config: RunnerConfig) -> Result<()> {
    state
        .store
        .set_run_retention_policy(baudbound_storage::RunRetentionPolicy::new(
            runner_config.runner.run_history_max_records,
            runner_config.runner.run_history_max_age_days,
        ))?;
    let next_core = build_runner_core(&runner_config, Arc::clone(&state.websocket_registry));
    let next_background_options = desktop_background_options(
        &runner_config,
        Arc::clone(&state.websocket_registry),
        state.config_path.clone(),
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
        flow_control: settings.flow_control.clone(),
        manufacturer: settings.manufacturer.clone(),
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

fn serial_port_scan_payload(port: serialport::SerialPortInfo) -> SerialPortScanPayload {
    match port.port_type {
        SerialPortType::UsbPort(info) => SerialPortScanPayload {
            manufacturer: info.manufacturer,
            port: port.port_name,
            port_type: "usb".to_owned(),
            product: info.product,
            product_id: Some(format!("{:04X}", info.pid)),
            serial_number: info.serial_number,
            vendor_id: Some(format!("{:04X}", info.vid)),
        },
        SerialPortType::BluetoothPort => SerialPortScanPayload {
            manufacturer: None,
            port: port.port_name,
            port_type: "bluetooth".to_owned(),
            product: None,
            product_id: None,
            serial_number: None,
            vendor_id: None,
        },
        SerialPortType::PciPort => SerialPortScanPayload {
            manufacturer: None,
            port: port.port_name,
            port_type: "pci".to_owned(),
            product: None,
            product_id: None,
            serial_number: None,
            vendor_id: None,
        },
        SerialPortType::Unknown => SerialPortScanPayload {
            manufacturer: None,
            port: port.port_name,
            port_type: "unknown".to_owned(),
            product: None,
            product_id: None,
            serial_number: None,
            vendor_id: None,
        },
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
) -> RunnerCore {
    let core = RunnerCore::from_config(runner_config).with_websocket_sink(websocket_registry);
    let action_handler = Arc::new(DesktopActionHandler::new(
        core.headless_action_handler(),
        SystemDesktopActionAdapter,
    ));
    core.with_action_handler(action_handler)
}

fn desktop_background_options(
    runner_config: &RunnerConfig,
    websocket_registry: Arc<WebSocketConnectionRegistry>,
    config_path: PathBuf,
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
    .with_serial_port_rebind_sink(Arc::new(
        crate::service::RunnerConfigSerialPortRebindSink::new(config_path),
    ) as Arc<dyn SerialPortRebindSink>)
}

#[cfg(test)]
mod tests;
