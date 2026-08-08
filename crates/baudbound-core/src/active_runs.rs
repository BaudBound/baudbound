//! Which runs of a script are in flight, and how to cancel them.
//!
//! A trigger decides what to do about an already running script before it asks
//! the execution queue for a permit, so it needs to answer two questions
//! without waiting: does this script have a run in flight, and what cancels it.
//!
//! An activation that is waiting for a permit counts as in flight. Without
//! that, a rapid stop would see nothing to cancel and a queued start would run
//! immediately afterwards, which is the opposite of what the author asked for.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use baudbound_runtime::{
    RunIdentity, RuntimeCancellationToken, RuntimeLogEntry, RuntimeRunObserver,
};

#[derive(Debug, Default)]
pub struct ActiveRunTracker {
    state: Mutex<TrackerState>,
}

#[derive(Debug, Default)]
struct TrackerState {
    /// Runs that have started, keyed by run id.
    started: HashMap<String, TrackedRun>,
    /// Activations holding a cancellation token while they wait for a permit,
    /// keyed by a token issued when the wait began.
    waiting: HashMap<u64, TrackedRun>,
    next_waiting_id: u64,
}

#[derive(Debug, Clone)]
struct TrackedRun {
    cancellation: RuntimeCancellationToken,
    script_id: String,
}

/// Cancels the wait when dropped, so an activation that never starts does not
/// leave the script looking busy forever.
#[derive(Debug)]
pub struct WaitingActivation {
    id: u64,
    tracker: Arc<ActiveRunTracker>,
}

impl Drop for WaitingActivation {
    fn drop(&mut self) {
        self.tracker.forget_waiting(self.id);
    }
}

impl ActiveRunTracker {
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Records an activation that is about to wait for a permit.
    pub fn register_waiting(
        self: &Arc<Self>,
        script_id: &str,
        cancellation: RuntimeCancellationToken,
    ) -> WaitingActivation {
        let mut state = self.lock();
        let id = state.next_waiting_id;
        state.next_waiting_id = state.next_waiting_id.wrapping_add(1);
        state.waiting.insert(
            id,
            TrackedRun {
                cancellation,
                script_id: script_id.to_owned(),
            },
        );
        WaitingActivation {
            id,
            tracker: Arc::clone(self),
        }
    }

    #[must_use]
    pub fn is_active(&self, script_id: &str) -> bool {
        let state = self.lock();
        state
            .started
            .values()
            .chain(state.waiting.values())
            .any(|run| run.script_id == script_id)
    }

    /// Cancels every run and waiting activation of one script.
    ///
    /// Returns how many were cancelled, so a caller can tell a real stop from
    /// one that arrived after the run had already finished.
    pub fn cancel_script(&self, script_id: &str) -> usize {
        let tokens = {
            let state = self.lock();
            state
                .started
                .values()
                .chain(state.waiting.values())
                .filter(|run| run.script_id == script_id)
                .map(|run| run.cancellation.clone())
                .collect::<Vec<_>>()
        };
        for token in &tokens {
            token.cancel();
        }
        tokens.len()
    }

    fn forget_waiting(&self, id: u64) {
        self.lock().waiting.remove(&id);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, TrackerState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl RuntimeRunObserver for ActiveRunTracker {
    fn run_started(&self, identity: &RunIdentity, cancellation: RuntimeCancellationToken) {
        self.lock().started.insert(
            identity.run_id.clone(),
            TrackedRun {
                cancellation,
                script_id: identity.script_id.clone(),
            },
        );
    }

    fn log_emitted(&self, _identity: &RunIdentity, _entry: &RuntimeLogEntry) {}

    fn run_finished(&self, identity: &RunIdentity) {
        self.lock().started.remove(&identity.run_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(run_id: &str, script_id: &str) -> RunIdentity {
        RunIdentity {
            run_id: run_id.to_owned(),
            script_id: script_id.to_owned(),
            trigger_node_id: "n-trigger".to_owned(),
        }
    }

    #[test]
    fn a_started_run_is_active_until_it_finishes() {
        let tracker = ActiveRunTracker::new();
        assert!(!tracker.is_active("script-1"));

        tracker.run_started(
            &identity("run-1", "script-1"),
            RuntimeCancellationToken::new(),
        );
        assert!(tracker.is_active("script-1"));
        assert!(!tracker.is_active("script-2"));

        tracker.run_finished(&identity("run-1", "script-1"));
        assert!(!tracker.is_active("script-1"));
    }

    #[test]
    fn an_activation_waiting_for_a_permit_counts_as_active() {
        // Otherwise a stop arriving while a start is queued would find nothing
        // to cancel, and the queued start would then run.
        let tracker = ActiveRunTracker::new();
        let waiting = tracker.register_waiting("script-1", RuntimeCancellationToken::new());
        assert!(tracker.is_active("script-1"));

        drop(waiting);
        assert!(!tracker.is_active("script-1"));
    }

    #[test]
    fn cancelling_a_script_cancels_its_runs_and_reports_how_many() {
        let tracker = ActiveRunTracker::new();
        let first = RuntimeCancellationToken::new();
        let second = RuntimeCancellationToken::new();
        let other = RuntimeCancellationToken::new();
        tracker.run_started(&identity("run-1", "script-1"), first.clone());
        let _waiting = tracker.register_waiting("script-1", second.clone());
        tracker.run_started(&identity("run-2", "script-2"), other.clone());

        assert_eq!(tracker.cancel_script("script-1"), 2);
        assert!(first.is_cancelled());
        assert!(second.is_cancelled());
        assert!(!other.is_cancelled(), "another script must be untouched");

        // Nothing left to cancel once they are gone.
        tracker.run_finished(&identity("run-1", "script-1"));
        assert_eq!(tracker.cancel_script("script-2"), 1);
    }
}

#[cfg(test)]
mod overlap_decision_tests {
    use std::{sync::Arc, thread, time::Duration};

    use super::*;

    fn identity(run_id: &str, script_id: &str) -> RunIdentity {
        RunIdentity {
            run_id: run_id.to_owned(),
            script_id: script_id.to_owned(),
            trigger_node_id: "n-trigger".to_owned(),
        }
    }

    #[test]
    fn stopping_a_script_does_not_wait_for_the_run_it_ends() {
        // The whole point of deciding before asking for a permit. With one run
        // allowed per script, a stop that queued would wait for the run it was
        // sent to end, which is a deadlock rather than a toggle.
        let tracker = ActiveRunTracker::new();
        let running = RuntimeCancellationToken::new();
        tracker.run_started(&identity("run-1", "script-1"), running.clone());

        let started = std::time::Instant::now();
        assert!(tracker.is_active("script-1"));
        let cancelled = tracker.cancel_script("script-1");

        assert_eq!(cancelled, 1);
        assert!(running.is_cancelled(), "the running run must be cancelled");
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "the decision must not block on the run it cancels"
        );
    }

    #[test]
    fn a_stop_arriving_after_the_run_finished_cancels_nothing() {
        // Benign: off was already true, and the next activation starts again.
        let tracker = ActiveRunTracker::new();
        tracker.run_started(
            &identity("run-1", "script-1"),
            RuntimeCancellationToken::new(),
        );
        tracker.run_finished(&identity("run-1", "script-1"));

        assert!(!tracker.is_active("script-1"));
        assert_eq!(tracker.cancel_script("script-1"), 0);
    }

    #[test]
    fn the_tracker_is_shared_across_threads() {
        // A listener thread decides while the run thread is still going.
        let tracker = ActiveRunTracker::new();
        let running = RuntimeCancellationToken::new();
        tracker.run_started(&identity("run-1", "script-1"), running.clone());

        let watcher = Arc::clone(&tracker);
        let handle = thread::spawn(move || watcher.cancel_script("script-1"));
        let cancelled = handle.join().expect("the deciding thread should finish");

        assert_eq!(cancelled, 1);
        assert!(running.is_cancelled());
    }
}
