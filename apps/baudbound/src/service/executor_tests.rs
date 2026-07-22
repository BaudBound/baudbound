use std::{
    collections::BTreeMap,
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use baudbound_runtime::RunIdentity;
use serde_json::Value;

use super::*;
use crate::trigger_monitor::{
    TriggerMonitor, TriggerMonitorEvent, TriggerMonitorEventSink, TriggerMonitorStatus,
};

#[derive(Default)]
struct CollectingMonitorSink(std::sync::Mutex<Vec<TriggerMonitorEvent>>);

impl TriggerMonitorEventSink for CollectingMonitorSink {
    fn publish(&self, event: TriggerMonitorEvent) {
        self.0.lock().unwrap().push(event);
    }
}

#[test]
fn executes_different_scripts_concurrently() {
    let barrier = Arc::new(Barrier::new(3));
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let runner = {
        let barrier = Arc::clone(&barrier);
        let active = Arc::clone(&active);
        let peak = Arc::clone(&peak);
        Arc::new(move |event: TriggerEvent| {
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(current, Ordering::SeqCst);
            barrier.wait();
            active.fetch_sub(1, Ordering::SeqCst);
            Ok(report_for_event(&event))
        }) as Arc<TriggerRunner>
    };
    let mut executor = TriggerExecutor::with_runner(2, 2, "test", runner)
        .expect("test trigger executor should start");

    executor
        .submit_from(event_for_script("script-1", "one"), "test")
        .expect("first job should queue");
    executor
        .submit_from(event_for_script("script-2", "two"), "test")
        .expect("second job should queue");
    barrier.wait();

    let completions = wait_for_completions(&mut executor, 2);
    assert_eq!(completions.len(), 2);
    assert_eq!(peak.load(Ordering::SeqCst), 2);
}

#[test]
fn serializes_runs_for_the_same_script() {
    let (started_sender, started_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let runner = {
        let release_receiver = std::sync::Mutex::new(release_receiver);
        Arc::new(move |event: TriggerEvent| {
            started_sender
                .send(event.node_id.clone())
                .expect("worker-start signal should send");
            release_receiver
                .lock()
                .expect("release receiver lock should not be poisoned")
                .recv()
                .expect("worker release signal should send");
            Ok(report_for_event(&event))
        }) as Arc<TriggerRunner>
    };
    let mut executor = TriggerExecutor::with_runner(2, 2, "test", runner)
        .expect("test trigger executor should start");

    executor
        .submit_from(event("first"), "test")
        .expect("first job should queue");
    assert_eq!(
        started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("first job should start"),
        "first"
    );
    executor
        .submit_from(event("second"), "test")
        .expect("second job should queue");
    assert!(
        started_receiver
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "second run for the same script started before the first completed"
    );

    release_sender
        .send(())
        .expect("first worker release signal should send");
    assert_eq!(wait_for_completions(&mut executor, 1).len(), 1);
    assert_eq!(
        started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("second job should start after first completion"),
        "second"
    );
    release_sender
        .send(())
        .expect("second worker release signal should send");
    assert_eq!(wait_for_completions(&mut executor, 1).len(), 1);
}

#[test]
fn rejects_work_when_the_bounded_queue_is_full() {
    let (started_sender, started_receiver) = mpsc::channel();
    let release = Arc::new(Barrier::new(2));
    let runner = {
        let release = Arc::clone(&release);
        Arc::new(move |event: TriggerEvent| {
            started_sender
                .send(())
                .expect("worker-start signal should send");
            release.wait();
            Ok(report_for_event(&event))
        }) as Arc<TriggerRunner>
    };
    let monitor = TriggerMonitor::default();
    let monitor_sink = Arc::new(CollectingMonitorSink::default());
    monitor.connect_sink(monitor_sink.clone()).unwrap();
    monitor.start();
    let mut executor = TriggerExecutor::with_runner_and_cancellation(
        1,
        1,
        "test",
        runner,
        baudbound_runtime::RuntimeCancellationToken::new(),
        Some(monitor),
    )
    .expect("test trigger executor should start");

    executor
        .submit_from(event("running"), "test")
        .expect("running job should queue");
    started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("worker should start first job");
    executor
        .submit_from(event("queued"), "test")
        .expect("second job should fill queue");
    assert_eq!(
        executor.submit_from(event("rejected"), "test"),
        Err(TriggerSubmitError::Full)
    );
    let monitored = wait_for_monitor_events(&monitor_sink, 3);
    assert_eq!(monitored[0].status, TriggerMonitorStatus::Queued);
    assert_eq!(monitored[1].status, TriggerMonitorStatus::Queued);
    assert_eq!(monitored[2].status, TriggerMonitorStatus::Rejected);
    assert_eq!(
        monitored[2].error.as_deref(),
        Some("trigger execution queue is at capacity")
    );

    release.wait();
    assert_eq!(wait_for_completions(&mut executor, 1).len(), 1);
    started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("worker should start queued job");
    release.wait();
    assert_eq!(wait_for_completions(&mut executor, 1).len(), 1);
}

#[test]
fn preserves_dispatch_failures_in_completions() {
    let runner =
        Arc::new(|_event: TriggerEvent| Err("dispatch denied".to_owned())) as Arc<TriggerRunner>;
    let mut executor = TriggerExecutor::with_runner(1, 1, "test", runner)
        .expect("test trigger executor should start");
    executor
        .submit_from(event("failed"), "test")
        .expect("job should queue");

    let completion = wait_for_completions(&mut executor, 1)
        .pop()
        .expect("completion should exist");
    assert_eq!(completion.result.unwrap_err(), "dispatch denied");
    assert_eq!(completion.event.node_id, "failed");
}

#[test]
fn dropping_executor_cancels_its_shared_runtime_token() {
    let cancellation = baudbound_runtime::RuntimeCancellationToken::new();
    let runner = Arc::new(|event: TriggerEvent| Ok(report_for_event(&event))) as Arc<TriggerRunner>;
    let executor = TriggerExecutor::with_runner_and_cancellation(
        1,
        1,
        "test",
        runner,
        cancellation.clone(),
        None,
    )
    .expect("test trigger executor should start");

    drop(executor);

    assert!(cancellation.is_cancelled());
}

#[test]
fn shutdown_waits_until_execution_workers_have_exited() {
    let (started_sender, started_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let finished = Arc::new(AtomicUsize::new(0));
    let runner = {
        let finished = Arc::clone(&finished);
        let release_receiver = std::sync::Mutex::new(release_receiver);
        Arc::new(move |event: TriggerEvent| {
            started_sender
                .send(())
                .expect("worker-start signal should send");
            release_receiver
                .lock()
                .expect("release receiver lock should not be poisoned")
                .recv()
                .expect("worker release signal should send");
            finished.fetch_add(1, Ordering::SeqCst);
            Ok(report_for_event(&event))
        }) as Arc<TriggerRunner>
    };
    let mut executor = TriggerExecutor::with_runner(1, 1, "test", runner)
        .expect("test trigger executor should start");
    executor
        .submit_from(event("running"), "test")
        .expect("job should queue");
    started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("worker should start");

    release_sender
        .send(())
        .expect("worker release signal should send");
    executor.shutdown().expect("executor should stop cleanly");

    assert_eq!(finished.load(Ordering::SeqCst), 1);
    assert!(executor.workers.is_empty());
    assert!(executor.sender.is_none());
}

fn wait_for_completions(executor: &mut TriggerExecutor, count: usize) -> Vec<TriggerCompletion> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut completions = Vec::new();
    while completions.len() < count && Instant::now() < deadline {
        if let Some(completion) = executor.try_completion() {
            completions.push(completion);
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    completions
}

fn wait_for_monitor_events(sink: &CollectingMonitorSink, count: usize) -> Vec<TriggerMonitorEvent> {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let events = sink.0.lock().unwrap().clone();
        if events.len() >= count || Instant::now() >= deadline {
            return events;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn event(node_id: &str) -> TriggerEvent {
    event_for_script("script-1", node_id)
}

fn event_for_script(script_id: &str, node_id: &str) -> TriggerEvent {
    TriggerEvent {
        action_type: "trigger.manual".to_owned(),
        node_id: node_id.to_owned(),
        payload: Value::Null,
        script_id: script_id.to_owned(),
    }
}

fn report_for_event(event: &TriggerEvent) -> RunReport {
    RunReport {
        identity: RunIdentity {
            run_id: format!("run-{}", event.node_id),
            script_id: event.script_id.clone(),
            trigger_node_id: event.node_id.clone(),
        },
        logs: Vec::new(),
        variable_scopes: BTreeMap::new(),
        variables: BTreeMap::new(),
    }
}
