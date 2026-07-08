use anyhow::{Context, Result};
use baudbound_core::{RunnerCore, RunnerStatus};
use baudbound_storage::FilesystemScriptStore;

use crate::{
    commands::service_health::service_health_document,
    output::{print_runner_status, print_service_status, print_trigger_registrations},
};

pub fn runner_status(core: &RunnerCore, store: &FilesystemScriptStore) -> Result<RunnerStatus> {
    core.status(store).context("failed to build runner status")
}

pub fn print_app_status(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    json: bool,
) -> Result<()> {
    let status = runner_status(core, store)?;
    let service_status = store
        .read_service_status()
        .context("failed to read runner service status")?;
    let service_health = service_health_document(service_status.as_ref());

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "desktop": {
                    "action_adapter": "system",
                    "native_tray": false,
                    "runner_name": core.name,
                    "storage_root": store.root(),
                    "supported_target_runtimes": core.supported_target_runtimes(),
                },
                "runner": status,
                "service_health": service_health,
                "service": service_status,
            }))?
        );
        return Ok(());
    }

    println!("{} runner", core.name);
    println!("Storage: {}", store.root().display());
    println!(
        "Supported target runtimes: {}",
        core.supported_target_runtimes().join(", ")
    );
    println!(
        "Desktop action adapter: clipboard, notifications, message boxes, audio, keyboard, mouse, and window actions."
    );
    println!("Native tray/UI: not started yet");
    print_service_status(service_status.as_ref(), Some(&service_health));
    println!();
    print_runner_status(&status, store.root());
    Ok(())
}

pub fn print_script_status(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    json: bool,
) -> Result<()> {
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
    store: &FilesystemScriptStore,
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
