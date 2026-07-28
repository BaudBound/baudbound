use anyhow::{Context, Result, bail};
use baudbound_core::{RunnerCore, TriggerRegistration};
use baudbound_storage::SqliteRunnerStore;
use baudbound_triggers::SerialInputService;

use super::options::ServeOptions;

pub fn validate_serve_start(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    options: &ServeOptions,
) -> Result<()> {
    validate_enabled_script_secrets(core, store)?;
    let registrations = core
        .list_trigger_registrations(store, None)
        .context("failed to load trigger registrations")?;
    if options.serial_enabled {
        SerialInputService::validate(registrations, &options.serial_connections)
            .context("serial input configuration is not ready")?;
    }
    Ok(())
}

pub fn validate_enabled_script_secrets(core: &RunnerCore, store: &SqliteRunnerStore) -> Result<()> {
    let scripts = core
        .list_installed(store)
        .context("failed to load installed scripts while checking required secrets")?;
    let mut blockers = Vec::new();

    for script in scripts.into_iter().filter(|script| script.enabled) {
        let mut missing = core
            .list_installed_secrets(store, &script.id)
            .with_context(|| {
                format!(
                    "failed to check required secrets for enabled script {:?} ({})",
                    script.name, script.id
                )
            })?
            .into_iter()
            .filter(|secret| secret.required && !secret.configured)
            .map(|secret| secret.name)
            .collect::<Vec<_>>();
        missing.sort();

        if !missing.is_empty() {
            blockers.push(format!(
                "{:?} ({}): {}",
                script.name,
                script.id,
                missing.join(", ")
            ));
        }
    }

    blockers.sort();
    if !blockers.is_empty() {
        bail!(
            "background runner cannot run because required secret values are missing for enabled scripts. {}. Configure the listed secrets or disable the affected scripts",
            blockers.join(". ")
        );
    }

    Ok(())
}

pub fn print_serve_preflight(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    options: &ServeOptions,
    json: bool,
) -> Result<()> {
    validate_enabled_script_secrets(core, store)?;
    let registrations = core
        .list_trigger_registrations(store, None)
        .context("failed to load trigger registrations")?;
    let service_rows = serve_preflight_rows(options, &registrations);
    let active_service_count = service_rows.iter().filter(|row| row.active).count();

    if json {
        let services = service_rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "active": row.active,
                    "enabled": row.enabled,
                    "name": row.name,
                    "registrations": row.registrations,
                    "target": row.target,
                })
            })
            .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "active_service_count": active_service_count,
                "configured_serial_device_count": options.serial_devices.len(),
                "idle": active_service_count == 0,
                "reload_interval_seconds": options.reload_check_interval.as_secs(),
                "services": services,
                "storage_root": store.root(),
                "trigger_registration_count": registrations.len(),
            }))?
        );
        return Ok(());
    }

    println!("Serve preflight");
    println!("Storage: {}", store.root().display());
    println!("Trigger registrations: {}", registrations.len());
    println!(
        "Trigger reload interval: {} second{}",
        options.reload_check_interval.as_secs(),
        if options.reload_check_interval.as_secs() == 1 {
            ""
        } else {
            "s"
        }
    );
    println!(
        "Configured serial devices: {}",
        options.serial_devices.len()
    );
    println!();
    println!(
        "{:<18}  {:<8}  {:<13}  {:<7}  Target",
        "Service", "Enabled", "Registrations", "Active"
    );
    for row in service_rows {
        println!(
            "{:<18}  {:<8}  {:<13}  {:<7}  {}",
            row.name,
            yes_no(row.enabled),
            row.registrations,
            yes_no(row.active),
            row.target
        );
    }
    if active_service_count == 0 {
        println!();
        println!("No listener services would be active.");
    }

    Ok(())
}

struct ServePreflightRow {
    active: bool,
    enabled: bool,
    name: &'static str,
    registrations: usize,
    target: String,
}

fn serve_preflight_rows(
    options: &ServeOptions,
    registrations: &[TriggerRegistration],
) -> Vec<ServePreflightRow> {
    vec![
        serve_preflight_row(
            "startup",
            options.startup_enabled,
            count_trigger_registrations(registrations, "trigger.startup"),
            "runner startup".to_owned(),
        ),
        serve_preflight_row(
            "schedule",
            options.schedules_enabled,
            count_trigger_registrations(registrations, "trigger.schedule"),
            "internal timer".to_owned(),
        ),
        serve_preflight_row(
            "file_watch",
            options.file_watch_enabled,
            count_trigger_registrations(registrations, "trigger.file_watch"),
            "filesystem watcher".to_owned(),
        ),
        serve_preflight_row(
            "process_started",
            options.process_watch_enabled,
            count_trigger_registrations(registrations, "trigger.process_started"),
            "process poller".to_owned(),
        ),
        serve_preflight_row(
            "serial_input",
            options.serial_enabled,
            count_trigger_registrations(registrations, "trigger.serial_input"),
            format!("{} configured device(s)", options.serial_devices.len()),
        ),
        serve_preflight_row(
            "hotkey",
            cfg!(windows) && options.hotkeys_enabled,
            count_trigger_registrations(registrations, "trigger.hotkey"),
            "native Windows global hotkeys".to_owned(),
        ),
        serve_preflight_row(
            "hotkey_stdin",
            options.hotkey_stdin_enabled,
            count_trigger_registrations(registrations, "trigger.hotkey"),
            "stdin hotkey events".to_owned(),
        ),
        serve_preflight_row(
            "webhook",
            options.webhooks_enabled,
            count_trigger_registrations(registrations, "trigger.webhook"),
            format!("{}:{}", options.webhook_bind, options.webhook_port),
        ),
        serve_preflight_row(
            "websocket",
            options.websockets_enabled,
            count_trigger_registrations(registrations, "trigger.websocket"),
            format!("{}:{}", options.websocket_bind, options.websocket_port),
        ),
    ]
}

fn serve_preflight_row(
    name: &'static str,
    enabled: bool,
    registrations: usize,
    target: String,
) -> ServePreflightRow {
    ServePreflightRow {
        active: enabled && registrations > 0,
        enabled,
        name,
        registrations,
        target,
    }
}

fn count_trigger_registrations(registrations: &[TriggerRegistration], action_type: &str) -> usize {
    registrations
        .iter()
        .filter(|registration| registration.action_type == action_type)
        .count()
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
