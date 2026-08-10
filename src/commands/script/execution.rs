use anyhow::{Context, Result};
use baudbound_core::{RunnerCore, TriggerEvent};
use baudbound_storage::SqliteRunnerStore;

use crate::output::print_run_report;

pub(super) fn dispatch_trigger_command(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    script: String,
    trigger: String,
    payload_json: Option<String>,
) -> Result<()> {
    let payload = parse_payload_json(payload_json)?;
    let installed = core
        .inspect_installed(store, &script)
        .with_context(|| format!("failed to resolve installed script {script:?}"))?;
    let registration = core
        .list_trigger_registrations(store, Some(&installed.id))
        .with_context(|| format!("failed to inspect triggers for {script:?}"))?
        .into_iter()
        .find(|registration| registration.node_id == trigger)
        .with_context(|| format!("script {script:?} has no trigger {trigger:?}"))?;
    let report = core
        .dispatch_trigger_event(
            store,
            TriggerEvent {
                action_type: registration.action_type,
                node_id: trigger,
                payload,
                script_id: installed.id,
            },
        )
        .with_context(|| format!("failed to dispatch trigger event for {script:?}"))?;
    print_activation(report);
    Ok(())
}

pub(super) fn run_script(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    script: String,
    trigger: Option<String>,
    payload_json: Option<String>,
) -> Result<()> {
    let payload = parse_payload_json(payload_json)?;
    let report = core
        .run_installed_with_trigger(store, &script, trigger.as_deref(), payload)
        .with_context(|| format!("failed to run installed script {script:?}"))?;
    print_activation(report);
    Ok(())
}

/// Prints what the activation did. A trigger set to stop or skip an already
/// running script does not produce a run report, and saying so is more useful
/// than printing an empty one.
fn print_activation(activation: baudbound_core::TriggerActivation) {
    match activation {
        baudbound_core::TriggerActivation::Started { report } => print_run_report(*report),
        baudbound_core::TriggerActivation::Stopped { cancelled } => {
            println!("Stopped {cancelled} running instance(s). No new run started.");
        }
        baudbound_core::TriggerActivation::Skipped => {
            println!("Skipped: the script is already running.");
        }
    }
}

fn parse_payload_json(payload_json: Option<String>) -> Result<serde_json::Value> {
    match payload_json {
        Some(payload) => {
            serde_json::from_str(&payload).with_context(|| "failed to parse --payload-json as JSON")
        }
        None => Ok(serde_json::Value::Null),
    }
}
