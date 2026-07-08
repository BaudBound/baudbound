use anyhow::{Context, Result};
use baudbound_core::{RunnerCore, TriggerEvent};
use baudbound_storage::FilesystemScriptStore;

use crate::output::print_run_report;

pub(super) fn dispatch_trigger_command(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    script: String,
    trigger: String,
    payload_json: Option<String>,
) -> Result<()> {
    let payload = parse_payload_json(payload_json)?;
    let installed = core
        .inspect_installed(store, &script)
        .with_context(|| format!("failed to resolve installed script {script:?}"))?;
    let report = core
        .dispatch_trigger_event(
            store,
            TriggerEvent {
                node_id: trigger,
                payload,
                script_id: installed.id,
            },
        )
        .with_context(|| format!("failed to dispatch trigger event for {script:?}"))?;
    print_run_report(report);
    Ok(())
}

pub(super) fn run_script(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    script: String,
    trigger: Option<String>,
    payload_json: Option<String>,
) -> Result<()> {
    let payload = parse_payload_json(payload_json)?;
    let report = core
        .run_installed_with_trigger(store, &script, trigger.as_deref(), payload)
        .with_context(|| format!("failed to run installed script {script:?}"))?;
    print_run_report(report);
    Ok(())
}

fn parse_payload_json(payload_json: Option<String>) -> Result<serde_json::Value> {
    match payload_json {
        Some(payload) => {
            serde_json::from_str(&payload).with_context(|| "failed to parse --payload-json as JSON")
        }
        None => Ok(serde_json::Value::Null),
    }
}
