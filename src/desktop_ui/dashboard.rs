use anyhow::{Result, anyhow};
use baudbound_core::TimeFormat;
use baudbound_storage::{
    GeneratedTriggerToken, NetworkTriggerType, ScriptStore, ScriptUpdateState, SqliteRunnerStore,
    StoredRunRecord, TriggerAuthStatus,
};
use serde::Serialize;
use serde_json::Value;

use crate::commands::{
    doctor::{DoctorCheck, desktop_doctor_checks},
    service_health::service_health_document,
};
use crate::service::validate_serve_start;

use super::{
    DesktopUiState,
    active_runs::ActiveRunSnapshot,
    background::DesktopRunnerSnapshot,
    config::{
        current_core, current_runner_config, serial_device_payloads, sync_runtime_config_from_disk,
    },
    secret_vault,
};

pub(super) fn build_dashboard_payload(state: &DesktopUiState) -> Result<DashboardPayload> {
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
        blacklist: state.blacklist.status(),
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
        stored_secret_value_count: state.store.stored_secret_value_count()?,
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

pub(super) fn request_running_service_reload(store: &SqliteRunnerStore) -> Result<()> {
    let status = store
        .read_service_status()?
        .ok_or_else(|| anyhow!("runner service is not running"))?;
    if status.get("state").and_then(Value::as_str) != Some("running") {
        return Err(anyhow!("runner service is not running"));
    }
    crate::service::request_service_control(&status, crate::service::ServiceControlCommand::Reload)
}

#[derive(Serialize)]
pub(super) struct DashboardPayload {
    active_runs: Vec<ActiveRunSnapshot>,
    active_runs_revision: u64,
    automatic_update_checks: bool,
    blacklist: crate::blacklist::BlacklistStatus,
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
    stored_secret_value_count: usize,
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
pub(super) struct SerialDevicePayload {
    pub(super) auto_reconnect: bool,
    pub(super) auto_rebind_port: bool,
    pub(super) baud_rate: u32,
    pub(super) data_bits: u8,
    pub(super) device_id: String,
    pub(super) dtr_on_open: String,
    pub(super) flow_control: String,
    pub(super) manufacturer: Option<String>,
    pub(super) max_message_bytes: usize,
    pub(super) message_gap_ms: u64,
    pub(super) open_stabilization_ms: u64,
    pub(super) parity: String,
    pub(super) port: String,
    pub(super) product_id: Option<String>,
    pub(super) product: Option<String>,
    pub(super) read_mode: String,
    pub(super) serial_number: Option<String>,
    pub(super) stop_bits: String,
    pub(super) validate_usb_identity: bool,
    pub(super) vendor_id: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ActionPayload {
    pub(super) dashboard: DashboardPayload,
    pub(super) message: String,
}

#[derive(Serialize)]
pub(super) struct PackageActionPayload {
    pub(super) dashboard: DashboardPayload,
    pub(super) generated_trigger_tokens: Vec<GeneratedTriggerToken>,
    pub(super) message: String,
}

#[derive(Serialize)]
pub(super) struct GeneratedTriggerTokenPayload {
    pub(super) dashboard: DashboardPayload,
    pub(super) message: String,
    pub(super) status: TriggerAuthStatus,
    pub(super) token: String,
}

pub(super) fn trigger_type_label(trigger_type: &NetworkTriggerType) -> &'static str {
    match trigger_type {
        NetworkTriggerType::Webhook => "webhook",
        NetworkTriggerType::Websocket => "WebSocket",
    }
}
