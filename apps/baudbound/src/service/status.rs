use anyhow::{Context, Result};
use baudbound_storage::FilesystemScriptStore;
use baudbound_triggers::TriggerServiceDiagnostics;

use super::{activity::ServiceActivity, options::ServeOptions, triggers::TriggerServices};
use crate::paths::current_unix_timestamp;

pub(super) fn write_serve_status(
    store: &FilesystemScriptStore,
    options: &ServeOptions,
    services: &TriggerServices,
    state: &str,
    started_at_unix: u64,
    last_reload_at_unix: u64,
    activity: &ServiceActivity,
) -> Result<()> {
    let service_rows = serve_status_services(options, services);
    let active_service_count = service_rows.iter().filter(|row| row.active).count();
    let document = serde_json::json!({
        "active_service_count": active_service_count,
        "activity": activity,
        "configured_serial_device_count": options.serial_devices.len(),
        "idle": active_service_count == 0,
        "last_heartbeat_unix": current_unix_timestamp(),
        "last_reload_unix": last_reload_at_unix,
        "pid": std::process::id(),
        "reload_interval_seconds": options.reload_check_interval.as_secs(),
        "runner_name": options.runner_name.clone(),
        "services": service_rows
            .into_iter()
            .map(|row| {
                serde_json::json!({
                    "active": row.active,
                    "diagnostics": row.diagnostics,
                    "enabled": row.enabled,
                    "name": row.name,
                    "registrations": row.registrations,
                    "target": row.target,
                })
            })
            .collect::<Vec<_>>(),
        "started_at_unix": started_at_unix,
        "state": state,
        "storage_root": store.root(),
    });
    store
        .write_service_status(&document)
        .context("failed to write runner service status")
}

fn serve_status_services(
    options: &ServeOptions,
    services: &TriggerServices,
) -> Vec<ServeStatusServiceRow> {
    vec![
        serve_status_service(
            "startup",
            options.startup_enabled,
            services.startup.len(),
            "runner startup".to_owned(),
            services.startup.diagnostics(),
        ),
        serve_status_service(
            "schedule",
            options.schedules_enabled,
            services.schedules.len(),
            "internal timer".to_owned(),
            services.schedules.diagnostics(),
        ),
        serve_status_service(
            "file_watch",
            options.file_watch_enabled,
            services.file_watch_service.len(),
            "filesystem watcher".to_owned(),
            services.file_watch_service.diagnostics(),
        ),
        serve_status_service(
            "process_started",
            options.process_watch_enabled,
            services.process_started_service.len(),
            "process poller".to_owned(),
            services.process_started_service.diagnostics(),
        ),
        serve_status_service(
            "serial_input",
            options.serial_enabled,
            services.serial_input_service.len(),
            format!("{} configured device(s)", options.serial_devices.len()),
            services.serial_input_service.diagnostics(),
        ),
        serve_status_service(
            "hotkey_stdin",
            options.hotkey_stdin_enabled,
            services.hotkey_service.len(),
            "stdin hotkey events".to_owned(),
            services.hotkey_service.diagnostics(),
        ),
        serve_status_service(
            "webhook",
            options.webhooks_enabled,
            services
                .webhook_host
                .as_ref()
                .map_or(0, |host| host.service.len()),
            format!("{}:{}", options.webhook_bind, options.webhook_port),
            services
                .webhook_host
                .as_ref()
                .map(|host| host.service.diagnostics())
                .unwrap_or_else(|| inactive_diagnostics("webhook route")),
        ),
        serve_status_service(
            "websocket",
            options.websockets_enabled,
            services.websocket_service.len(),
            format!("{}:{}", options.websocket_bind, options.websocket_port),
            services.websocket_service.diagnostics(),
        ),
    ]
}

fn serve_status_service(
    name: &'static str,
    enabled: bool,
    registrations: usize,
    target: String,
    diagnostics: TriggerServiceDiagnostics,
) -> ServeStatusServiceRow {
    ServeStatusServiceRow {
        active: enabled && registrations > 0,
        diagnostics,
        enabled,
        name,
        registrations,
        target,
    }
}

struct ServeStatusServiceRow {
    active: bool,
    diagnostics: TriggerServiceDiagnostics,
    enabled: bool,
    name: &'static str,
    registrations: usize,
    target: String,
}

fn inactive_diagnostics(label: &str) -> TriggerServiceDiagnostics {
    TriggerServiceDiagnostics {
        running: false,
        state: "idle",
        summary: format!("0 {label}s registered"),
    }
}
