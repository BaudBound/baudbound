use std::path::PathBuf;

use anyhow::{Context, Result};
use baudbound_core::RunnerCore;
use baudbound_storage::{GeneratedTriggerToken, NetworkTriggerType, SqliteRunnerStore};

use crate::output::print_installed_script;

pub(super) fn import_script(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    package: PathBuf,
) -> Result<()> {
    let script = core
        .import_package(store, &package)
        .with_context(|| format!("failed to import {}", package.display()))?;
    println!(
        "Imported {} ({}) as {} into {}",
        script.name,
        script.id,
        script.package_file_name,
        store.root().display()
    );
    Ok(())
}

pub(super) fn update_script(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    package: PathBuf,
) -> Result<()> {
    let script = core
        .update_package(store, &package)
        .with_context(|| format!("failed to update from {}", package.display()))?;
    println!(
        "Updated {} ({}) as {}",
        script.name, script.id, script.package_file_name
    );
    Ok(())
}

pub(super) fn print_generated_trigger_tokens(tokens: &[GeneratedTriggerToken]) {
    if tokens.is_empty() {
        return;
    }
    println!();
    println!("New network trigger tokens:");
    println!("Save these tokens now. They cannot be shown again.");
    for generated in tokens {
        println!(
            "{}  {}  {}",
            trigger_type_name(generated.status.trigger_type),
            generated.status.node_id,
            generated.token
        );
    }
}

fn trigger_type_name(trigger_type: NetworkTriggerType) -> &'static str {
    match trigger_type {
        NetworkTriggerType::Webhook => "webhook",
        NetworkTriggerType::Websocket => "websocket",
    }
}

pub(super) fn list_scripts(core: &RunnerCore, store: &SqliteRunnerStore, json: bool) -> Result<()> {
    let scripts = core
        .list_installed(store)
        .context("failed to list installed scripts")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&scripts)?);
    } else if scripts.is_empty() {
        println!("No scripts installed.");
    } else {
        for script in scripts {
            println!(
                "{}  {}  {}  {}",
                script.id,
                if script.enabled {
                    "enabled "
                } else {
                    "disabled"
                },
                script.risk_level,
                script.name
            );
        }
    }
    Ok(())
}

pub(super) fn inspect_script(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    script: String,
    json: bool,
) -> Result<()> {
    let script = core
        .inspect_installed(store, &script)
        .with_context(|| format!("failed to inspect installed script {script:?}"))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&script)?);
    } else {
        print_installed_script(&script);
    }
    Ok(())
}

pub(super) fn set_script_enabled(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    script: String,
    enabled: bool,
) -> Result<()> {
    let updated = core
        .set_installed_enabled(store, &script, enabled)
        .with_context(|| {
            format!(
                "failed to {} installed script {script:?}",
                if enabled { "enable" } else { "disable" }
            )
        })?;
    println!(
        "{} {} ({})",
        if enabled { "Enabled" } else { "Disabled" },
        updated.name,
        updated.id
    );
    Ok(())
}

pub(super) fn remove_script(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    script: String,
) -> Result<()> {
    let removed = core
        .remove_installed(store, &script)
        .with_context(|| format!("failed to remove installed script {script:?}"))?;
    println!("Removed {} ({})", removed.name, removed.id);
    Ok(())
}
