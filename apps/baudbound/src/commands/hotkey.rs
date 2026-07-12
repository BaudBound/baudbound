use std::{
    io::{self, BufRead},
    time::SystemTime,
};

use anyhow::{Context, Result};
use baudbound_core::{RunReport, RunnerCore};
use baudbound_runtime::RuntimeCancellationToken;
use baudbound_storage::SqliteRunnerStore;
use baudbound_triggers::HotkeyService;

use crate::{cli::HotkeyCommand, output::print_run_report};

pub fn handle_hotkey_command(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    command: HotkeyCommand,
) -> Result<()> {
    let service = desktop_hotkey_service(core, store)?;
    match command {
        HotkeyCommand::List { json } => {
            let hotkeys = service.registered_hotkeys();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "count": service.len(),
                        "hotkeys": hotkeys,
                    }))?
                );
            } else if hotkeys.is_empty() {
                println!("No enabled hotkey triggers found.");
            } else {
                println!("Enabled hotkey triggers:");
                for hotkey in hotkeys {
                    println!("  - {hotkey}");
                }
            }
        }
        HotkeyCommand::Dispatch { key, json } => {
            let reports = dispatch_hotkey_key(core, store, &service, &key, SystemTime::now())?;

            if json {
                println!("{}", serde_json::to_string_pretty(&reports)?);
            } else if reports.is_empty() {
                println!("No enabled hotkey triggers matched {key:?}.");
            } else {
                println!(
                    "Dispatched {} hotkey trigger{} for {key:?}.",
                    reports.len(),
                    if reports.len() == 1 { "" } else { "s" }
                );
                for report in reports {
                    print_run_report(report);
                }
            }
        }
        HotkeyCommand::Listen { stdin, json } => {
            if !stdin {
                anyhow::bail!(
                    "hotkey listen requires --stdin for injected test events. On Windows Desktop, the background runner registers configured hotkeys natively."
                );
            }

            listen_for_stdin_hotkeys(core, store, &service, json)?;
        }
    }

    Ok(())
}

fn listen_for_stdin_hotkeys(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    service: &HotkeyService,
    json: bool,
) -> Result<()> {
    if !json {
        println!("Listening for newline-delimited hotkey expressions on stdin.");
        println!("Press Ctrl+C or close stdin to stop.");
    }

    for line in io::stdin().lock().lines() {
        let key = line.context("failed to read hotkey event from stdin")?;
        let key = key.trim();
        if key.is_empty() {
            continue;
        }

        let reports = dispatch_hotkey_key(core, store, service, key, SystemTime::now())?;
        if json {
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "key": key,
                    "matched": reports.len(),
                    "reports": reports,
                }))?
            );
        } else if reports.is_empty() {
            println!("No enabled hotkey triggers matched {key:?}.");
        } else {
            println!(
                "Dispatched {} hotkey trigger{} for {key:?}.",
                reports.len(),
                if reports.len() == 1 { "" } else { "s" }
            );
            for report in reports {
                print_run_report(report);
            }
        }
    }

    Ok(())
}

pub fn dispatch_hotkey_key(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    service: &HotkeyService,
    key: &str,
    timestamp: SystemTime,
) -> Result<Vec<RunReport>> {
    dispatch_hotkey_key_with_cancellation(
        core,
        store,
        service,
        key,
        timestamp,
        &RuntimeCancellationToken::new(),
    )
}

pub fn dispatch_hotkey_key_with_cancellation(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    service: &HotkeyService,
    key: &str,
    timestamp: SystemTime,
    cancellation: &RuntimeCancellationToken,
) -> Result<Vec<RunReport>> {
    let events = service
        .events_for_key(key, timestamp)
        .with_context(|| format!("failed to build hotkey event for {key:?}"))?;
    events
        .into_iter()
        .map(|event| {
            let script_id = event.script_id.clone();
            let node_id = event.node_id.clone();
            core.dispatch_trigger_event_with_cancellation(store, event, cancellation.clone())
                .with_context(|| {
                    format!("failed to dispatch hotkey trigger {node_id} for {script_id}")
                })
        })
        .collect::<Result<Vec<_>>>()
}

fn desktop_hotkey_service(core: &RunnerCore, store: &SqliteRunnerStore) -> Result<HotkeyService> {
    let registrations = core
        .list_trigger_registrations(store, None)
        .context("failed to load enabled trigger registrations")?;
    HotkeyService::from_registrations(registrations).context("failed to register hotkey triggers")
}
