use std::{fs, path::PathBuf, sync::Arc};

use anyhow::{Result, anyhow};
use baudbound_actions::DesktopActionHandler;
use baudbound_core::{RunnerConfig, RunnerCore, SerialDeviceSettings};
use baudbound_triggers::{SerialPortRebindSink, WebSocketConnectionRegistry};
use serde::Serialize;
use tauri::{Manager, Runtime, State};

use crate::{
    desktop_actions::SystemDesktopActionAdapter,
    service::{ServeOptions, ServeOverrides, validate_serve_start},
    trigger_monitor::TriggerMonitor,
};

use super::{
    ActionPayload, DesktopUiState, SerialDevicePayload,
    active_runs::ActiveRunRegistry,
    command_guard::{SensitiveOperation, SensitiveOperationGuard},
    consume_sensitive_operation, desktop_config, request_running_service_reload, run_locked_action,
};

#[tauri::command]
pub(super) fn read_runner_config(
    autostart: State<'_, tauri_plugin_autostart::AutoLaunchManager>,
    state: State<'_, DesktopUiState>,
) -> Result<RunnerConfigPayload, String> {
    read_runner_config_payload(&autostart, &state).map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) async fn save_runner_config<R: Runtime>(
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
    let app = window.app_handle().clone();
    run_config_action(app, move |autostart, state| {
        save_runner_config_contents(autostart, state, &contents, restart_background)
    })
    .await
}

#[tauri::command]
pub(super) async fn save_runner_config_model<R: Runtime>(
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
            config: Box::new(config.clone()),
            restart_background,
        },
        &guard,
        &state,
        &window,
    )?;
    let app = window.app_handle().clone();
    run_config_action(app, move |autostart, state| {
        save_runner_config_model_contents(autostart, state, config, restart_background)
    })
    .await
}

#[tauri::command]
pub(super) async fn reset_runner_config<R: Runtime>(
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
    let app = window.app_handle().clone();
    run_config_action(app, move |autostart, state| {
        save_valid_runner_config(
            autostart,
            state,
            RunnerConfig::template_toml(),
            restart_background,
            ConfigWriteOperation::Reset,
        )
    })
    .await
}

async fn run_config_action<R, F>(
    app: tauri::AppHandle<R>,
    action: F,
) -> Result<ActionPayload, String>
where
    R: Runtime,
    F: FnOnce(&tauri_plugin_autostart::AutoLaunchManager, &DesktopUiState) -> Result<String>
        + Send
        + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let autostart = app.state::<tauri_plugin_autostart::AutoLaunchManager>();
        let state = app.state::<DesktopUiState>();
        run_locked_action(&state, || action(&autostart, &state))
    })
    .await
    .map_err(|error| format!("runner config task failed: {error}"))?
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

#[derive(Serialize)]
pub(super) struct RunnerConfigPayload {
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
    // Settings edited from the interface arrive as a whole config, but the file
    // they replace is the one the author has been reading. Write the values
    // over it so its comments and spacing survive the edit.
    let existing = fs::read_to_string(&state.config_path).unwrap_or_default();
    let contents = config.to_toml_preserving_comments(&existing)?;
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
                "Saved runner config. Restart the desktop background runner to apply runtime changes."
            }
            (Self::Save, false, false) => "Saved runner config.",
            (Self::Reset, true, _) => {
                "Reset runner config to defaults and restarted the desktop background runner."
            }
            (Self::Reset, false, true) => {
                "Reset runner config to defaults. Restart the desktop background runner to apply runtime changes."
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

pub(super) fn runner_runtime_config_changed(previous: &RunnerConfig, next: &RunnerConfig) -> bool {
    let RunnerConfig {
        desktop: _,
        display: _,
        limits: previous_limits,
        runner: previous_runner,
        security: previous_security,
        serial: previous_serial,
        triggers: previous_triggers,
        updates: _,
        webhooks: previous_webhooks,
        websockets: previous_websockets,
    } = previous;
    let RunnerConfig {
        desktop: _,
        display: _,
        limits: next_limits,
        runner: next_runner,
        security: next_security,
        serial: next_serial,
        triggers: next_triggers,
        updates: _,
        webhooks: next_webhooks,
        websockets: next_websockets,
    } = next;

    (
        previous_limits,
        previous_runner,
        previous_security,
        previous_serial,
        previous_triggers,
        previous_webhooks,
        previous_websockets,
    ) != (
        next_limits,
        next_runner,
        next_security,
        next_serial,
        next_triggers,
        next_webhooks,
        next_websockets,
    )
}

fn rollback_result<T, E: std::fmt::Display>(result: std::result::Result<T, E>) -> String {
    match result {
        Ok(_) => "succeeded".to_owned(),
        Err(error) => format!("failed ({error})"),
    }
}

fn replace_runtime_config(state: &DesktopUiState, runner_config: RunnerConfig) -> Result<()> {
    state
        .dialog_broker
        .configure(
            super::desktop_dialog::DesktopDialogOptions::from_desktop_settings(
                &runner_config.desktop,
            ),
        )
        .map_err(|error| anyhow!(error))?;
    state
        .store
        .set_run_retention_policy(baudbound_storage::RunRetentionPolicy::new(
            runner_config.runner.run_history_max_records,
            runner_config.runner.run_history_max_age_days,
            runner_config.runner.run_history_max_bytes,
        ))?;
    let existing_core = current_core(state)?;
    let next_core = build_runner_core(
        &runner_config,
        Arc::clone(&state.websocket_registry),
        Arc::clone(&state.active_runs),
        Arc::clone(&state.blacklist),
        Arc::clone(&state.dialog_broker),
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

pub(super) fn sync_runtime_config_from_disk(state: &DesktopUiState) -> Result<()> {
    let contents = fs::read_to_string(&state.config_path)?;
    let runner_config = RunnerConfig::from_toml(&contents, &state.config_path)?;
    replace_runtime_config(state, runner_config)
}

pub(super) fn current_runner_config(state: &DesktopUiState) -> Result<RunnerConfig> {
    state
        .runner_config
        .lock()
        .map_err(|_| anyhow!("runner config lock is poisoned"))
        .map(|config| config.clone())
}

pub(super) fn serial_device_payloads(config: &RunnerConfig) -> Vec<SerialDevicePayload> {
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

pub(super) fn current_core(state: &DesktopUiState) -> Result<RunnerCore> {
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

pub(super) fn build_runner_core(
    runner_config: &RunnerConfig,
    websocket_registry: Arc<WebSocketConnectionRegistry>,
    active_runs: Arc<ActiveRunRegistry>,
    blacklist: Arc<crate::blacklist::BlacklistService>,
    dialog_broker: Arc<super::desktop_dialog::DesktopDialogBroker>,
) -> RunnerCore {
    let core = RunnerCore::from_config(runner_config)
        .with_execution_mode(baudbound_core::RunnerExecutionMode::Desktop)
        .with_blacklist_policy(Arc::clone(&blacklist))
        .with_websocket_sink(websocket_registry)
        .with_run_observer(blacklist)
        .with_run_observer(active_runs);
    let action_handler = Arc::new(DesktopActionHandler::new(
        core.headless_action_handler(),
        SystemDesktopActionAdapter::with_dialog_provider(dialog_broker),
    ));
    core.with_action_handler(action_handler)
}

pub(super) fn desktop_background_options(
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
