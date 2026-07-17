use std::path::Path;

use baudbound_core::{
    ApprovalStatus, PackageHashStatus, RunReport, RunnerStatus, TriggerRegistration,
};
use baudbound_storage::{InstalledScript, ScriptApproval, StoredRunRecord};

use crate::time_format::CliTimeFormatter;

pub fn print_installed_script(script: &InstalledScript) {
    println!("Script: {}", script.name);
    println!("ID: {}", script.id);
    println!("Enabled: {}", script.enabled);
    println!("Risk: {}", script.risk_level);
    println!("Target runtime: {}", script.target_runtime);
    println!("Package hash: {}", script.package_hash);
    println!("Package file: {}", script.package_file_name);
    println!("Package path: {}", script.package_path.display());
    println!("Assets: {}", script.asset_count);
    println!("Package version: {}", script.package_format_version);
    println!("Runtime version: {}", script.script_language_version);
}

pub fn print_approval_permissions(approval: &ScriptApproval) {
    if approval.approved_permissions.is_empty() {
        println!("Approved permissions: none");
        return;
    }

    println!("Approved permissions:");
    for permission in &approval.approved_permissions {
        println!("  - {permission}");
    }
}

pub fn print_service_status(
    service_status: Option<&serde_json::Value>,
    service_health: Option<&serde_json::Value>,
    time: CliTimeFormatter,
) -> anyhow::Result<()> {
    println!();
    println!("Service:");

    let Some(service_status) = service_status else {
        println!("  No serve status has been written yet.");
        return Ok(());
    };

    println!(
        "  State: {}",
        service_status
            .get("state")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
    );
    if let Some(pid) = service_status
        .get("pid")
        .and_then(serde_json::Value::as_u64)
    {
        println!("  PID: {pid}");
    }
    if let Some(last_heartbeat) = service_status
        .get("last_heartbeat_unix")
        .and_then(serde_json::Value::as_u64)
    {
        println!(
            "  Last heartbeat: {}",
            time.format_unix_seconds(last_heartbeat)?
        );
    }
    if let Some(reload_interval) = service_status
        .get("reload_interval_seconds")
        .and_then(serde_json::Value::as_u64)
    {
        println!("  Reload interval: {reload_interval}s");
    }

    let active_service_count = service_status
        .get("active_service_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    println!("  Active listener services: {active_service_count}");
    print_listener_service_diagnostics(service_status);
    print_dispatch_activity(service_status);

    if let Some(service_health) = service_health {
        let health = service_health
            .get("health")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        println!("  Health: {health}");
        if let Some(age) = service_health
            .get("heartbeat_age_seconds")
            .and_then(serde_json::Value::as_u64)
        {
            println!("  Heartbeat age: {age}s");
        }
        if service_health
            .get("stale")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            println!("  Warning: service heartbeat is stale.");
        }
    }
    Ok(())
}

fn print_listener_service_diagnostics(service_status: &serde_json::Value) {
    let Some(services) = service_status
        .get("services")
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };

    println!("  Listener services:");
    for service in services {
        let name = service
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        let enabled = service
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let registrations = service
            .get("registrations")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default();
        let state = service
            .get("diagnostics")
            .and_then(|diagnostics| diagnostics.get("state"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(if enabled { "waiting" } else { "disabled" });
        let summary = service
            .get("diagnostics")
            .and_then(|diagnostics| diagnostics.get("summary"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");

        println!("    - {name}: {state}, {registrations} registration(s)");
        if !summary.is_empty() {
            println!("      {summary}");
        }
    }
}

fn print_dispatch_activity(service_status: &serde_json::Value) {
    let Some(activity) = service_status.get("activity") else {
        return;
    };

    let total_dispatch_count = activity
        .get("total_dispatch_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    let failed_dispatch_count = activity
        .get("failed_dispatch_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();

    println!("  Trigger dispatches: {total_dispatch_count}");
    println!("  Failed dispatches: {failed_dispatch_count}");
    if let Some(trigger_count) = activity
        .get("triggers")
        .and_then(serde_json::Value::as_object)
        .map(serde_json::Map::len)
    {
        println!("  Trigger nodes with activity: {trigger_count}");
    }

    let Some(last_dispatch) = activity.get("last_dispatch") else {
        return;
    };
    if last_dispatch.is_null() {
        return;
    }

    let status = last_dispatch
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let source = last_dispatch
        .get("source")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let script_id = last_dispatch
        .get("script_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let node_id = last_dispatch
        .get("node_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");

    println!("  Last dispatch: {status} via {source}");
    println!("    Script: {script_id}");
    println!("    Trigger: {node_id}");
    if let Some(run_id) = last_dispatch
        .get("run_id")
        .and_then(serde_json::Value::as_str)
    {
        println!("    Run: {run_id}");
    }
    if let Some(error) = last_dispatch
        .get("error")
        .and_then(serde_json::Value::as_str)
    {
        println!("    Error: {error}");
    }
}

pub fn print_runner_status(status: &RunnerStatus, storage_root: &Path) {
    println!("Runner: {}", status.runner_name);
    println!("Storage: {}", storage_root.display());
    println!(
        "Supported target runtimes: {}",
        status.supported_target_runtimes.join(", ")
    );
    println!(
        "Scripts: {} total, {} enabled, {} disabled",
        status.total_script_count, status.enabled_script_count, status.disabled_script_count
    );
    println!("Active triggers: {}", status.trigger_count);
    println!("Scripts with problems: {}", status.problem_count);

    if status.scripts.is_empty() {
        println!("No scripts installed.");
        return;
    }

    println!();
    println!(
        "{:<24}  {:<8}  {:<10}  {:<18}  {:<9}  Name",
        "Script ID", "State", "Hash", "Approval", "Triggers"
    );
    for script in &status.scripts {
        println!(
            "{:<24}  {:<8}  {:<10}  {:<18}  {:<9}  {}",
            truncate_for_table(&script.installed.id, 24),
            if script.installed.enabled {
                "enabled"
            } else {
                "disabled"
            },
            package_hash_status_label(&script.package_hash_status),
            approval_status_label(&script.approval_status),
            script.triggers.len(),
            script.installed.name
        );

        if let Some(error) = &script.package_error {
            println!("  package: {error}");
        }
        if let PackageHashStatus::Mismatch { expected, actual } = &script.package_hash_status {
            println!("  hash: expected {expected}, got {actual}");
        }
        if let PackageHashStatus::Error { message: error } = &script.package_hash_status {
            println!("  hash: {error}");
        }
        match &script.approval_status {
            ApprovalStatus::Error { message: error } => println!("  approval: {error}"),
            ApprovalStatus::StalePackageHash {
                approved_package_hash,
                installed_package_hash,
            } => println!(
                "  approval: approved {approved_package_hash}, installed {installed_package_hash}"
            ),
            _ => {}
        }
    }
}

fn package_hash_status_label(status: &PackageHashStatus) -> &'static str {
    match status {
        PackageHashStatus::Valid => "valid",
        PackageHashStatus::Mismatch { .. } => "mismatch",
        PackageHashStatus::Error { .. } => "error",
    }
}

fn approval_status_label(status: &ApprovalStatus) -> &'static str {
    match status {
        ApprovalStatus::Current => "current",
        ApprovalStatus::Error { .. } => "error",
        ApprovalStatus::Missing => "missing",
        ApprovalStatus::PackageUnavailable => "package unavailable",
        ApprovalStatus::PermissionMismatch => "permission changed",
        ApprovalStatus::StalePackageHash { .. } => "stale hash",
        ApprovalStatus::Unknown => "unknown",
    }
}

pub fn print_trigger_registrations(registrations: &[TriggerRegistration]) {
    println!(
        "{:<28}  {:<24}  {:<28}  {:<24}  Type",
        "Script", "Script ID", "Trigger Node", "Action"
    );
    for registration in registrations {
        println!(
            "{:<28}  {:<24}  {:<28}  {:<24}  {}",
            truncate_for_table(&registration.script_name, 28),
            truncate_for_table(&registration.script_id, 24),
            truncate_for_table(&registration.node_id, 28),
            truncate_for_table(&registration.action_type, 24),
            registration.runner_type
        );
    }
}

fn truncate_for_table(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_owned();
    }

    value
        .chars()
        .take(width.saturating_sub(3))
        .collect::<String>()
        + "..."
}

pub fn print_run_report(report: RunReport) {
    println!("Run: {}", report.identity.run_id);
    println!("Trigger: {}", report.identity.trigger_node_id);
    for log in report.logs {
        match log.node_id {
            Some(node_id) => println!("[{}] [{}] {}", log.level, node_id, log.message),
            None => println!("[{}] {}", log.level, log.message),
        }
    }
    if !report.variables.is_empty() {
        println!("Variables:");
        for (name, value) in report.variables {
            println!("  {name}: {value}");
        }
    }
}

pub fn print_run_record(record: &StoredRunRecord, time: CliTimeFormatter) -> anyhow::Result<()> {
    println!(
        "Run: {}  script={}  status={}  completed_at={}",
        record.run_id,
        record.script_id,
        record.status,
        time.format_unix_seconds(record.completed_at_unix)?
    );
    for log in &record.logs {
        let timestamp = time.format_unix_milliseconds(log.timestamp_unix_ms)?;
        match &log.node_id {
            Some(node_id) => println!(
                "  [{timestamp}] [{}] [{}] {}",
                log.level, node_id, log.message
            ),
            None => println!("  [{timestamp}] [{}] {}", log.level, log.message),
        }
    }
    if !record.variables.is_empty() {
        println!("  Variables:");
        for (name, value) in &record.variables {
            println!("    {name}: {value}");
        }
    }
    Ok(())
}
