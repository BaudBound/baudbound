use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};

use baudbound_runtime::{
    RunIdentity, RuntimeCancellationToken, RuntimeLogEntry, RuntimeRunObserver,
    unix_timestamp_millis_now,
};
use serde::Serialize;
use tauri::{AppHandle, Runtime};

mod events;

use events::{ActiveRunEvent, ActiveRunEventSink, TauriActiveRunEventSink};

const MAX_LIVE_LOG_ENTRIES: usize = 500;

#[derive(Clone, Serialize)]
pub(super) struct ActiveRunSnapshot {
    pub(super) cancellation_requested: bool,
    pub(super) discarded_log_count: usize,
    pub(super) logs: Vec<RuntimeLogEntry>,
    pub(super) run_id: String,
    pub(super) script_id: String,
    pub(super) started_at_unix_ms: u64,
    pub(super) trigger_node_id: String,
}

struct ActiveRunEntry {
    cancellation: RuntimeCancellationToken,
    cancellation_requested: bool,
    discarded_log_count: usize,
    logs: VecDeque<RuntimeLogEntry>,
    run_id: String,
    script_id: String,
    started_at_unix_ms: u64,
    trigger_node_id: String,
}

impl ActiveRunEntry {
    fn snapshot(&self) -> ActiveRunSnapshot {
        ActiveRunSnapshot {
            cancellation_requested: self.cancellation_requested,
            discarded_log_count: self.discarded_log_count,
            logs: self.logs.iter().cloned().collect(),
            run_id: self.run_id.clone(),
            script_id: self.script_id.clone(),
            started_at_unix_ms: self.started_at_unix_ms,
            trigger_node_id: self.trigger_node_id.clone(),
        }
    }
}

pub(super) struct ActiveRunRegistry {
    event_sink: Mutex<Option<Arc<dyn ActiveRunEventSink>>>,
    state: Mutex<ActiveRunRegistryState>,
}

#[derive(Default)]
struct ActiveRunRegistryState {
    revision: u64,
    runs: BTreeMap<String, ActiveRunEntry>,
}

impl ActiveRunRegistryState {
    fn next_revision(&mut self) -> u64 {
        self.revision = self.revision.saturating_add(1);
        self.revision
    }
}

#[derive(Default)]
pub(super) struct ActiveRunsSnapshot {
    pub(super) revision: u64,
    pub(super) runs: Vec<ActiveRunSnapshot>,
}

impl Default for ActiveRunRegistry {
    fn default() -> Self {
        Self {
            event_sink: Mutex::new(None),
            state: Mutex::new(ActiveRunRegistryState::default()),
        }
    }
}

impl ActiveRunRegistry {
    pub(super) fn connect_event_sink<R: Runtime>(&self, app: AppHandle<R>) {
        self.set_event_sink(Arc::new(TauriActiveRunEventSink::new(app)));
    }

    pub(super) fn snapshot(&self) -> ActiveRunsSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut runs = state
            .runs
            .values()
            .map(ActiveRunEntry::snapshot)
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| {
            left.started_at_unix_ms
                .cmp(&right.started_at_unix_ms)
                .then_with(|| left.run_id.cmp(&right.run_id))
        });
        ActiveRunsSnapshot {
            revision: state.revision,
            runs,
        }
    }

    pub(super) fn stop_run(&self, run_id: &str) -> bool {
        let cancellation = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(entry) = state.runs.get_mut(run_id) else {
                return false;
            };
            if entry.cancellation_requested {
                return true;
            }
            entry.cancellation_requested = true;
            let cancellation = entry.cancellation.clone();
            let revision = state.next_revision();
            self.publish(ActiveRunEvent::CancellationRequested {
                revision,
                run_id: run_id.to_owned(),
            });
            cancellation
        };
        cancellation.cancel();
        true
    }

    pub(super) fn stop_script_runs(&self, script_id: &str) -> usize {
        let cancellations = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let run_ids = state
                .runs
                .iter()
                .filter(|(_, entry)| entry.script_id == script_id && !entry.cancellation_requested)
                .map(|(run_id, _)| run_id.clone())
                .collect::<Vec<_>>();
            let mut cancellations = Vec::with_capacity(run_ids.len());
            for run_id in run_ids {
                let cancellation = {
                    let entry = state
                        .runs
                        .get_mut(&run_id)
                        .expect("collected active run should remain present");
                    entry.cancellation_requested = true;
                    entry.cancellation.clone()
                };
                let revision = state.next_revision();
                self.publish(ActiveRunEvent::CancellationRequested {
                    revision,
                    run_id: run_id.clone(),
                });
                cancellations.push((run_id, cancellation));
            }
            cancellations
        };
        for (_, cancellation) in &cancellations {
            cancellation.cancel();
        }
        cancellations.len()
    }

    fn publish(&self, event: ActiveRunEvent) {
        if let Some(sink) = self
            .event_sink
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
        {
            sink.publish(event);
        }
    }

    fn set_event_sink(&self, sink: Arc<dyn ActiveRunEventSink>) {
        *self
            .event_sink
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(sink);
    }
}

impl RuntimeRunObserver for ActiveRunRegistry {
    fn run_started(&self, identity: &RunIdentity, cancellation: RuntimeCancellationToken) {
        let entry = ActiveRunEntry {
            cancellation_requested: cancellation.is_cancelled(),
            cancellation,
            discarded_log_count: 0,
            logs: VecDeque::with_capacity(MAX_LIVE_LOG_ENTRIES),
            run_id: identity.run_id.clone(),
            script_id: identity.script_id.clone(),
            started_at_unix_ms: unix_timestamp_millis_now(),
            trigger_node_id: identity.trigger_node_id.clone(),
        };
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.runs.insert(identity.run_id.clone(), entry);
            let revision = state.next_revision();
            let run = state
                .runs
                .get(&identity.run_id)
                .expect("started run should be present")
                .snapshot();
            self.publish(ActiveRunEvent::Started { revision, run });
        }
    }

    fn log_emitted(&self, identity: &RunIdentity, entry: &RuntimeLogEntry) {
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(run) = state.runs.get_mut(&identity.run_id) else {
                return;
            };
            if run.logs.len() == MAX_LIVE_LOG_ENTRIES {
                run.logs.pop_front();
                run.discarded_log_count = run.discarded_log_count.saturating_add(1);
            }
            run.logs.push_back(entry.clone());
            let discarded_log_count = run.discarded_log_count;
            let revision = state.next_revision();
            self.publish(ActiveRunEvent::LogEmitted {
                discarded_log_count,
                log: entry.clone(),
                revision,
                run_id: identity.run_id.clone(),
            });
        }
    }

    fn run_finished(&self, identity: &RunIdentity) {
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if state.runs.remove(&identity.run_id).is_none() {
                return;
            }
            let revision = state.next_revision();
            self.publish(ActiveRunEvent::Finished {
                revision,
                run_id: identity.run_id.clone(),
            });
        }
    }

    fn run_recorded(&self) {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.publish(ActiveRunEvent::RunRecorded {
            revision: state.revision,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Barrier, thread, time::Duration};

    use super::*;

    #[derive(Default)]
    struct RecordingEventSink {
        events: Mutex<Vec<ActiveRunEvent>>,
    }

    impl ActiveRunEventSink for RecordingEventSink {
        fn publish(&self, event: ActiveRunEvent) {
            self.events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(event);
        }
    }

    struct OrderingEventSink {
        events: Mutex<Vec<u64>>,
        first_publish_started: Barrier,
    }

    impl ActiveRunEventSink for OrderingEventSink {
        fn publish(&self, event: ActiveRunEvent) {
            let revision = event_revision(&event);
            if revision == 1 {
                self.first_publish_started.wait();
                thread::sleep(Duration::from_millis(25));
            }
            self.events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(revision);
        }
    }

    fn event_revision(event: &ActiveRunEvent) -> u64 {
        match event {
            ActiveRunEvent::Started { revision, .. }
            | ActiveRunEvent::LogEmitted { revision, .. }
            | ActiveRunEvent::CancellationRequested { revision, .. }
            | ActiveRunEvent::Finished { revision, .. }
            | ActiveRunEvent::RunRecorded { revision } => *revision,
        }
    }

    fn identity(run_id: &str, script_id: &str) -> RunIdentity {
        RunIdentity {
            run_id: run_id.to_owned(),
            script_id: script_id.to_owned(),
            trigger_node_id: "n-trigger".to_owned(),
        }
    }

    #[test]
    fn tracks_logs_and_removes_finished_runs() {
        let registry = ActiveRunRegistry::default();
        let identity = identity("run-1", "script-1");
        registry.run_started(&identity, RuntimeCancellationToken::new());
        registry.log_emitted(
            &identity,
            &RuntimeLogEntry {
                action_type: Some("action.log".to_owned()),
                level: "info".to_owned(),
                message: "running".to_owned(),
                node_id: Some("n-log".to_owned()),
                timestamp_unix_ms: 1,
            },
        );

        let snapshots = registry.snapshot().runs;
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].logs[0].message, "running");

        registry.run_finished(&identity);
        assert!(registry.snapshot().runs.is_empty());
    }

    #[test]
    fn stops_only_runs_owned_by_the_selected_script() {
        let registry = ActiveRunRegistry::default();
        let first_token = RuntimeCancellationToken::new();
        let second_token = RuntimeCancellationToken::new();
        registry.run_started(&identity("run-1", "script-1"), first_token.clone());
        registry.run_started(&identity("run-2", "script-2"), second_token.clone());

        assert_eq!(registry.stop_script_runs("script-1"), 1);
        assert!(first_token.is_cancelled());
        assert!(!second_token.is_cancelled());
        assert!(registry.snapshot().runs[0].cancellation_requested);
    }

    #[test]
    fn bounds_live_logs_and_reports_omitted_entries() {
        let registry = ActiveRunRegistry::default();
        let identity = identity("run-1", "script-1");
        registry.run_started(&identity, RuntimeCancellationToken::new());

        for index in 0..=MAX_LIVE_LOG_ENTRIES {
            registry.log_emitted(
                &identity,
                &RuntimeLogEntry {
                    action_type: None,
                    level: "info".to_owned(),
                    message: index.to_string(),
                    node_id: None,
                    timestamp_unix_ms: index as u64,
                },
            );
        }

        let snapshots = registry.snapshot().runs;
        assert_eq!(snapshots[0].logs.len(), MAX_LIVE_LOG_ENTRIES);
        assert_eq!(snapshots[0].discarded_log_count, 1);
        assert_eq!(snapshots[0].logs[0].message, "1");
    }

    #[test]
    fn publishes_ordered_revisioned_lifecycle_events() {
        let registry = ActiveRunRegistry::default();
        let sink = Arc::new(RecordingEventSink::default());
        registry.set_event_sink(sink.clone());
        let identity = identity("run-1", "script-1");

        registry.run_started(&identity, RuntimeCancellationToken::new());
        registry.log_emitted(
            &identity,
            &RuntimeLogEntry {
                action_type: None,
                level: "info".to_owned(),
                message: "running".to_owned(),
                node_id: None,
                timestamp_unix_ms: 1,
            },
        );
        assert!(registry.stop_run("run-1"));
        registry.run_finished(&identity);
        registry.run_recorded();

        let events = sink
            .events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(matches!(
            &events[0],
            ActiveRunEvent::Started { revision: 1, run } if run.run_id == "run-1"
        ));
        assert!(matches!(
            &events[1],
            ActiveRunEvent::LogEmitted { revision: 2, run_id, .. } if run_id == "run-1"
        ));
        assert!(matches!(
            &events[2],
            ActiveRunEvent::CancellationRequested { revision: 3, run_id } if run_id == "run-1"
        ));
        assert!(matches!(
            &events[3],
            ActiveRunEvent::Finished { revision: 4, run_id } if run_id == "run-1"
        ));
        assert!(matches!(
            events[4],
            ActiveRunEvent::RunRecorded { revision: 4 }
        ));
        let serialized = serde_json::to_value(&events[0])
            .expect("active run event should serialize for the Tauri bridge");
        assert_eq!(serialized["kind"], "started");
        assert_eq!(serialized["revision"], 1);
        assert_eq!(serialized["run"]["run_id"], "run-1");
        assert_eq!(registry.snapshot().revision, 4);
    }

    #[test]
    fn concurrent_mutations_publish_in_revision_order() {
        let registry = Arc::new(ActiveRunRegistry::default());
        let sink = Arc::new(OrderingEventSink {
            events: Mutex::new(Vec::new()),
            first_publish_started: Barrier::new(2),
        });
        registry.set_event_sink(sink.clone());

        let first_registry = Arc::clone(&registry);
        let first = thread::spawn(move || {
            first_registry.run_started(
                &identity("run-1", "script-1"),
                RuntimeCancellationToken::new(),
            );
        });
        sink.first_publish_started.wait();
        let second_registry = Arc::clone(&registry);
        let second = thread::spawn(move || {
            second_registry.run_started(
                &identity("run-2", "script-2"),
                RuntimeCancellationToken::new(),
            );
        });

        first.join().expect("first publisher should exit");
        second.join().expect("second publisher should exit");
        assert_eq!(
            sink.events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_slice(),
            &[1, 2]
        );
    }
}
