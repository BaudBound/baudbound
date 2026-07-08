use std::{
    sync::mpsc::Receiver,
    time::{Instant, SystemTime},
};

use baudbound_core::{RunnerCore, TriggerEvent};
use baudbound_storage::FilesystemScriptStore;
use baudbound_triggers::{HotkeyService, ScheduleService, StartupService};

use super::heartbeat::ServeStatusTracker;
use crate::{commands::hotkey::dispatch_hotkey_key, output::print_run_report};

pub(super) fn dispatch_hotkey_stdin_events(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    service: &HotkeyService,
    receiver: &Receiver<String>,
    status: &mut ServeStatusTracker,
) -> bool {
    let mut dispatched_any_event = false;
    while let Ok(key) = receiver.try_recv() {
        match dispatch_hotkey_key(core, store, service, &key, SystemTime::now()) {
            Ok(reports) if reports.is_empty() => {
                println!("No enabled hotkey triggers matched {key:?}.");
            }
            Ok(reports) => {
                dispatched_any_event = true;
                println!(
                    "Dispatched {} hotkey trigger{} for {key:?}.",
                    reports.len(),
                    if reports.len() == 1 { "" } else { "s" }
                );
                for report in reports {
                    status.record_report("hotkey", &report);
                    print_run_report(report);
                }
            }
            Err(error) => eprintln!("Hotkey dispatch failed for {key:?}: {error}"),
        }
    }
    dispatched_any_event
}

pub(super) fn dispatch_startup_events(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    startup: &mut StartupService,
    status: &mut ServeStatusTracker,
) -> bool {
    let mut dispatched_any_event = false;
    for event in startup.drain_events() {
        dispatched_any_event = true;
        println!(
            "Dispatching startup trigger {} for script {}",
            event.node_id, event.script_id
        );
        match core.dispatch_trigger_event(store, event.clone()) {
            Ok(report) => {
                status.record_report("startup", &report);
                print_run_report(report);
            }
            Err(error) => {
                status.record_event_failure("startup", &event, error.to_string());
                eprintln!("Startup dispatch failed: {error}");
            }
        }
    }
    dispatched_any_event
}

pub(super) fn dispatch_due_schedules(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    schedules: &mut ScheduleService,
    status: &mut ServeStatusTracker,
) -> bool {
    let mut dispatched_any_event = false;
    let events = schedules.due_events(Instant::now(), SystemTime::now());
    for event in events {
        dispatched_any_event = true;
        println!(
            "Dispatching schedule trigger {} for script {}",
            event.node_id, event.script_id
        );
        match core.dispatch_trigger_event(store, event.clone()) {
            Ok(report) => {
                status.record_report("schedule", &report);
                print_run_report(report);
            }
            Err(error) => {
                status.record_event_failure("schedule", &event, error.to_string());
                eprintln!("Schedule dispatch failed: {error}");
            }
        }
    }
    dispatched_any_event
}

pub(super) fn dispatch_trigger_events(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    receiver: &Receiver<TriggerEvent>,
    status: &mut ServeStatusTracker,
) -> bool {
    let mut dispatched_any_event = false;
    for event in receiver.try_iter() {
        dispatched_any_event = true;
        dispatch_trigger_event(core, store, event, status);
    }
    dispatched_any_event
}

pub(super) fn dispatch_trigger_event(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    event: TriggerEvent,
    status: &mut ServeStatusTracker,
) {
    println!(
        "Dispatching trigger {} for script {}",
        event.node_id, event.script_id
    );
    match core.dispatch_trigger_event(store, event.clone()) {
        Ok(report) => {
            status.record_report("listener", &report);
            print_run_report(report);
        }
        Err(error) => {
            status.record_event_failure("listener", &event, error.to_string());
            eprintln!("Trigger dispatch failed: {error}");
        }
    }
}
