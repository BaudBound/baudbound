use std::path::PathBuf;

use anyhow::{Context, Result};
use baudbound_core::RunnerCore;

pub fn validate_package(core: &RunnerCore, package: PathBuf) -> Result<()> {
    let summary = core
        .validate_package(&package)
        .with_context(|| format!("failed to validate {}", package.display()))?;
    println!(
        "Package valid: {} (package v{}, runtime v{}, {}, {} asset{})",
        summary.script_name,
        summary.package_format_version,
        summary.script_language_version,
        summary.target_runtimes.join(", "),
        summary.asset_count,
        if summary.asset_count == 1 { "" } else { "s" }
    );
    Ok(())
}

pub fn inspect_package(core: &RunnerCore, target: String, json: bool) -> Result<()> {
    let package = PathBuf::from(&target);
    let inspection = core
        .inspect_package(&package)
        .with_context(|| format!("failed to inspect {}", package.display()))?;

    if json {
        let output = serde_json::json!({
            "summary": inspection.summary,
            "entries": inspection.entries,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Script: {}", inspection.summary.script_name);
        println!(
            "Target runtimes: {}",
            inspection.summary.target_runtimes.join(", ")
        );
        println!(
            "Package version: {}",
            inspection.summary.package_format_version
        );
        println!(
            "Runtime version: {}",
            inspection.summary.script_language_version
        );
        println!("Files:");
        for entry in inspection.entries {
            println!("  - {entry}");
        }
    }
    Ok(())
}
