use std::{
    sync::mpsc::Receiver,
    time::{Instant, SystemTime},
};

use baudbound_core::{RunnerCore, TriggerEvent};
use baudbound_storage::SqliteRunnerStore;
use baudbound_triggers::{HotkeyService, ScheduleService, StartupService};

use super::{
    executor::{TriggerExecutor, TriggerSubmitError},
    heartbeat::ServeStatusTracker,
};
use crate::{commands::hotkey::dispatch_hotkey_key, output::print_run_report};

pub(super) fn dispatch_hotkey_stdin_events(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
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
    store: &SqliteRunnerStore,
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
    store: &SqliteRunnerStore,
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

pub(super) fn queue_trigger_events(
    receiver: &Receiver<TriggerEvent>,
    executor: &mut TriggerExecutor,
    status: &mut ServeStatusTracker,
) {
    const MAX_EVENTS_PER_POLL: usize = 256;
    for event in receiver.try_iter().take(MAX_EVENTS_PER_POLL) {
        queue_trigger_event(executor, event, status);
    }
}

pub(super) fn queue_trigger_event(
    executor: &mut TriggerExecutor,
    event: TriggerEvent,
    status: &mut ServeStatusTracker,
) {
    println!(
        "Queueing trigger {} for script {}",
        event.node_id, event.script_id
    );
    match executor.submit(event.clone()) {
        Ok(_) => {}
        Err(TriggerSubmitError::Full) => {
            let error = "trigger execution queue is at capacity";
            status.record_event_failure("listener", &event, error.to_owned());
            eprintln!("Trigger dispatch rejected for {}: {error}", event.node_id);
        }
        Err(TriggerSubmitError::Stopped) => {
            let error = "trigger execution workers are unavailable";
            status.record_event_failure("listener", &event, error.to_owned());
            eprintln!("Trigger dispatch rejected for {}: {error}", event.node_id);
        }
    }
}

pub(super) fn record_trigger_completions(
    executor: &TriggerExecutor,
    status: &mut ServeStatusTracker,
) -> bool {
    let mut completed_any = false;
    while let Some(completion) = executor.try_completion() {
        completed_any = true;
        match completion.result {
            Ok(report) => {
                status.record_report("listener", &report);
                print_run_report(report);
            }
            Err(error) => {
                status.record_event_failure("listener", &completion.event, error.clone());
                eprintln!("Trigger dispatch failed: {error}");
            }
        }
    }
    completed_any
}
