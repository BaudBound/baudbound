use anyhow::{Context, Result};
use baudbound_core::{RunnerConfig, RunnerCore, RunnerStatus};
use baudbound_storage::SqliteRunnerStore;

use crate::{
    commands::service_health::service_health_document,
    output::{print_runner_status, print_service_status, print_trigger_registrations},
    service::redact_service_control,
    time_format::CliTimeFormatter,
};

pub fn runner_status(core: &RunnerCore, store: &SqliteRunnerStore) -> Result<RunnerStatus> {
    core.status(store).context("failed to build runner status")
}

pub fn print_app_status(
    config: &RunnerConfig,
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    json: bool,
) -> Result<()> {
    let status = runner_status(core, store)?;
    let service_status = store
        .read_service_status()
        .context("failed to read runner service status")?;
    let service_health = service_health_document(service_status.as_ref());
    let mut public_service_status = service_status.clone();
    if let Some(status) = public_service_status.as_mut() {
        redact_service_control(status);
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "desktop": {
                    "action_adapter": "system",
                    "native_tray": false,
                    "storage_root": store.root(),
                    "supported_target_runtimes": core.supported_target_runtimes(),
                },
                "runner": status,
                "service_health": service_health,
                "service": public_service_status,
            }))?
        );
        return Ok(());
    }

    println!("Storage: {}", store.root().display());
    println!(
        "Supported target runtimes: {}",
        core.supported_target_runtimes().join(", ")
    );
    println!(
        "Desktop action adapter: system native backends (run `baudbound doctor` for availability)."
    );
    println!("Native tray/UI: not started yet");
    print_service_status(
        public_service_status.as_ref(),
        Some(&service_health),
        CliTimeFormatter::from_config(config),
    )?;
    println!();
    print_runner_status(&status, store.root());
    Ok(())
}

pub fn print_script_status(core: &RunnerCore, store: &SqliteRunnerStore, json: bool) -> Result<()> {
    let status = runner_status(core, store)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        print_runner_status(&status, store.root());
    }
    Ok(())
}

pub fn print_trigger_registrations_for_script(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    script: Option<&str>,
    json: bool,
) -> Result<()> {
    let registrations = core
        .list_trigger_registrations(store, script)
        .context("failed to list trigger registrations")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&registrations)?);
    } else if registrations.is_empty() {
        println!("No trigger registrations found.");
    } else {
        print_trigger_registrations(&registrations);
    }
    Ok(())
}
