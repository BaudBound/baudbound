use std::{
    sync::mpsc::Receiver,
    time::{Instant, SystemTime},
};

use baudbound_core::TriggerEvent;
use baudbound_triggers::{HotkeyService, ScheduleService, StartupService};

use super::{
    executor::{TriggerExecutor, TriggerSubmitError},
    heartbeat::ServeStatusTracker,
};
use crate::output::print_run_report;

pub(super) fn dispatch_hotkey_stdin_events(
    service: &HotkeyService,
    receiver: &Receiver<String>,
    executor: &mut TriggerExecutor,
    status: &mut ServeStatusTracker,
) -> bool {
    let mut dispatched_any_event = false;
    while let Ok(key) = receiver.try_recv() {
        match service.events_for_key(&key, SystemTime::now()) {
            Ok(events) if events.is_empty() => {
                println!("No enabled hotkey triggers matched {key:?}.");
            }
            Ok(events) => {
                dispatched_any_event = true;
                println!(
                    "Dispatched {} hotkey trigger{} for {key:?}.",
                    events.len(),
                    if events.len() == 1 { "" } else { "s" }
                );
                for event in events {
                    queue_trigger_event_from(executor, event, status, "hotkey");
                }
            }
            Err(error) => eprintln!("Hotkey event creation failed for {key:?}: {error}"),
        }
    }
    dispatched_any_event
}

pub(super) fn dispatch_startup_events(
    startup: &mut StartupService,
    executor: &mut TriggerExecutor,
    status: &mut ServeStatusTracker,
) -> bool {
    let mut dispatched_any_event = false;
    for event in startup.drain_events() {
        dispatched_any_event = true;
        println!(
            "Dispatching startup trigger {} for script {}",
            event.node_id, event.script_id
        );
        queue_trigger_event_from(executor, event, status, "startup");
    }
    dispatched_any_event
}

pub(super) fn dispatch_due_schedules(
    schedules: &mut ScheduleService,
    executor: &mut TriggerExecutor,
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
        queue_trigger_event_from(executor, event, status, "schedule");
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
    queue_trigger_event_from(executor, event, status, "listener");
}

fn queue_trigger_event_from(
    executor: &mut TriggerExecutor,
    event: TriggerEvent,
    status: &mut ServeStatusTracker,
    source: &'static str,
) {
    println!(
        "Queueing trigger {} for script {}",
        event.node_id, event.script_id
    );
    match executor.submit_from(event.clone(), source) {
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
    executor: &mut TriggerExecutor,
    status: &mut ServeStatusTracker,
) -> bool {
    let mut completed_any = false;
    while let Some(completion) = executor.try_completion() {
        completed_any = true;
        match completion.result {
            Ok(report) => {
                status.record_report(completion.source, &report);
                print_run_report(report);
            }
            Err(error) => {
                status.record_event_failure(completion.source, &completion.event, error.clone());
                eprintln!("Trigger dispatch failed: {error}");
            }
        }
    }
    completed_any
}
