use anyhow::{Context, Result};
use baudbound_core::RunnerCore;
use baudbound_storage::{ScriptStore, SqliteRunnerStore};

use crate::{output::print_approval_permissions, time_format::CliTimeFormatter};

pub(super) fn print_approval(store: &SqliteRunnerStore, script: String, json: bool) -> Result<()> {
    let approval = store
        .find_script_approval(&script)
        .with_context(|| format!("failed to inspect approval for {script:?}"))?;
    match (approval, json) {
        (Some(approval), true) => {
            println!("{}", serde_json::to_string_pretty(&approval)?);
        }
        (Some(approval), false) => {
            println!("Approved script: {}", approval.script_id);
            println!("Package hash: {}", approval.package_hash);
            println!(
                "Approved at: {}",
                CliTimeFormatter::from_store(store)?
                    .format_unix_seconds(approval.approved_at_unix)?
            );
            print_approval_permissions(&approval);
        }
        (None, true) => println!("null"),
        (None, false) => println!("No approval stored for {script:?}."),
    }
    Ok(())
}

pub(super) fn approve_script(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    script: String,
) -> Result<()> {
    let approval = core
        .approve_installed(store, &script)
        .with_context(|| format!("failed to approve installed script {script:?}"))?;
    println!(
        "Approved {} for package {}",
        approval.script_id, approval.package_hash
    );
    print_approval_permissions(&approval);
    Ok(())
}

pub(super) fn revoke_approval(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    script: String,
) -> Result<()> {
    let revoked = core
        .revoke_approval(store, &script)
        .with_context(|| format!("failed to revoke approval for {script:?}"))?;
    if let Some(approval) = revoked {
        println!("Revoked approval for {}", approval.script_id);
    } else {
        println!("No approval was stored for {script:?}.");
    }
    Ok(())
}
