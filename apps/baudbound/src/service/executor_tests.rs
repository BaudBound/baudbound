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

#[test]
fn executes_trigger_jobs_concurrently() {
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
        .submit(event("one"))
        .expect("first job should queue");
    executor
        .submit(event("two"))
        .expect("second job should queue");
    barrier.wait();

    let completions = wait_for_completions(&executor, 2);
    assert_eq!(completions.len(), 2);
    assert_eq!(peak.load(Ordering::SeqCst), 2);
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
    let mut executor = TriggerExecutor::with_runner(1, 1, "test", runner)
        .expect("test trigger executor should start");

    executor
        .submit(event("running"))
        .expect("running job should queue");
    started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("worker should start first job");
    executor
        .submit(event("queued"))
        .expect("second job should fill queue");
    assert_eq!(
        executor.submit(event("rejected")),
        Err(TriggerSubmitError::Full)
    );

    release.wait();
    started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("worker should start queued job");
    release.wait();
    assert_eq!(wait_for_completions(&executor, 2).len(), 2);
}

#[test]
fn preserves_dispatch_failures_in_completions() {
    let runner =
        Arc::new(|_event: TriggerEvent| Err("dispatch denied".to_owned())) as Arc<TriggerRunner>;
    let mut executor = TriggerExecutor::with_runner(1, 1, "test", runner)
        .expect("test trigger executor should start");
    executor.submit(event("failed")).expect("job should queue");

    let completion = wait_for_completions(&executor, 1)
        .pop()
        .expect("completion should exist");
    assert_eq!(completion.result.unwrap_err(), "dispatch denied");
    assert_eq!(completion.event.node_id, "failed");
}

fn wait_for_completions(executor: &TriggerExecutor, count: usize) -> Vec<TriggerCompletion> {
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

fn event(node_id: &str) -> TriggerEvent {
    TriggerEvent {
        node_id: node_id.to_owned(),
        payload: Value::Null,
        script_id: "script-1".to_owned(),
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
        variables: BTreeMap::new(),
    }
}
