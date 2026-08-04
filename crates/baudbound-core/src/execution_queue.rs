use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use baudbound_runtime::{ResourceLimit, RuntimeCancellationToken};

use crate::config::QueueOverflowStrategy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionAdmissionPolicy {
    pub max_active_runs_global: ResourceLimit,
    pub max_active_runs_per_script: ResourceLimit,
    pub max_queued_activations_per_script: ResourceLimit,
    pub queue_overflow_strategy: QueueOverflowStrategy,
}

impl Default for ExecutionAdmissionPolicy {
    fn default() -> Self {
        Self {
            max_active_runs_global: ResourceLimit::limited(16),
            max_active_runs_per_script: ResourceLimit::limited(1),
            max_queued_activations_per_script: ResourceLimit::limited(64),
            queue_overflow_strategy: QueueOverflowStrategy::RejectNewest,
        }
    }
}

pub(crate) struct ScriptExecutionQueue {
    changed: Condvar,
    state: Mutex<QueueState>,
}

struct QueueState {
    active_runs: HashMap<String, usize>,
    active_total: usize,
    dependencies: HashMap<String, HashMap<String, usize>>,
    policy: ExecutionAdmissionPolicy,
    waiting: HashMap<String, VecDeque<Arc<Waiter>>>,
}

struct Waiter {
    superseded: AtomicBool,
}

pub(crate) struct ScriptExecutionPermit<'a> {
    owner_script_id: Option<String>,
    queue: &'a ScriptExecutionQueue,
    script_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AcquireError {
    Cancelled,
    Busy,
    Full,
    Rejected,
    Superseded,
}

impl ScriptExecutionQueue {
    pub(crate) fn new(policy: ExecutionAdmissionPolicy) -> Self {
        Self {
            changed: Condvar::new(),
            state: Mutex::new(QueueState {
                active_runs: HashMap::new(),
                active_total: 0,
                dependencies: HashMap::new(),
                policy,
                waiting: HashMap::new(),
            }),
        }
    }

    pub(crate) fn update_policy(&self, policy: ExecutionAdmissionPolicy) {
        self.lock_state().policy = policy;
        self.changed.notify_all();
    }

    pub(crate) fn acquire(
        &self,
        script_id: &str,
        cancellation: &RuntimeCancellationToken,
        is_rejected: impl Fn() -> bool,
    ) -> Result<ScriptExecutionPermit<'_>, AcquireError> {
        self.acquire_internal(script_id, None, cancellation, is_rejected)
    }

    pub(crate) fn acquire_nested(
        &self,
        owner_script_id: &str,
        script_id: &str,
        cancellation: &RuntimeCancellationToken,
        is_rejected: impl Fn() -> bool,
    ) -> Result<ScriptExecutionPermit<'_>, AcquireError> {
        self.acquire_internal(script_id, Some(owner_script_id), cancellation, is_rejected)
    }

    #[cfg(test)]
    pub(crate) fn waiting_count(&self, script_id: &str) -> usize {
        self.lock_state()
            .waiting
            .get(script_id)
            .map_or(0, VecDeque::len)
    }

    fn acquire_internal(
        &self,
        script_id: &str,
        owner_script_id: Option<&str>,
        cancellation: &RuntimeCancellationToken,
        is_rejected: impl Fn() -> bool,
    ) -> Result<ScriptExecutionPermit<'_>, AcquireError> {
        const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(50);

        let waiter = Arc::new(Waiter {
            superseded: AtomicBool::new(false),
        });
        let mut state = self.lock_state();
        if let Some(owner_script_id) = owner_script_id {
            if creates_dependency_cycle(&state.dependencies, owner_script_id, script_id) {
                return Err(AcquireError::Busy);
            }
            add_dependency(&mut state, owner_script_id, script_id);
        }
        let queue_is_full = state.waiting.get(script_id).is_some_and(|waiting| {
            !limit_permits_next(
                state.policy.max_queued_activations_per_script,
                waiting.len(),
            )
        });
        if queue_is_full {
            match state.policy.queue_overflow_strategy {
                QueueOverflowStrategy::RejectNewest => {
                    remove_dependency(&mut state, owner_script_id, script_id);
                    return Err(AcquireError::Full);
                }
                QueueOverflowStrategy::DropOldest => {
                    if let Some(superseded) = state
                        .waiting
                        .get_mut(script_id)
                        .and_then(VecDeque::pop_front)
                    {
                        superseded.superseded.store(true, Ordering::Release);
                    }
                }
            }
        }
        state
            .waiting
            .entry(script_id.to_owned())
            .or_default()
            .push_back(Arc::clone(&waiter));

        loop {
            if waiter.superseded.load(Ordering::Acquire) {
                remove_waiter(&mut state, script_id, &waiter);
                remove_dependency(&mut state, owner_script_id, script_id);
                self.changed.notify_all();
                return Err(AcquireError::Superseded);
            }
            if is_rejected() {
                remove_waiter(&mut state, script_id, &waiter);
                remove_dependency(&mut state, owner_script_id, script_id);
                self.changed.notify_all();
                return Err(AcquireError::Rejected);
            }
            if cancellation.is_cancelled() {
                remove_waiter(&mut state, script_id, &waiter);
                remove_dependency(&mut state, owner_script_id, script_id);
                self.changed.notify_all();
                return Err(AcquireError::Cancelled);
            }

            let is_first = state
                .waiting
                .get(script_id)
                .and_then(VecDeque::front)
                .is_some_and(|candidate| Arc::ptr_eq(candidate, &waiter));
            if is_first && has_active_capacity(&state, script_id) {
                remove_waiter(&mut state, script_id, &waiter);
                state.active_total = state
                    .active_total
                    .checked_add(1)
                    .expect("active run count was checked before increment");
                *state.active_runs.entry(script_id.to_owned()).or_default() += 1;
                return Ok(ScriptExecutionPermit {
                    owner_script_id: owner_script_id.map(ToOwned::to_owned),
                    queue: self,
                    script_id: script_id.to_owned(),
                });
            }

            state = self
                .changed
                .wait_timeout(state, CANCELLATION_POLL_INTERVAL)
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, QueueState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Default for ScriptExecutionQueue {
    fn default() -> Self {
        Self::new(ExecutionAdmissionPolicy::default())
    }
}

impl Drop for ScriptExecutionPermit<'_> {
    fn drop(&mut self) {
        let mut state = self.queue.lock_state();
        state.active_total = state.active_total.saturating_sub(1);
        if let Some(active) = state.active_runs.get_mut(&self.script_id) {
            *active = active.saturating_sub(1);
            if *active == 0 {
                state.active_runs.remove(&self.script_id);
            }
        }
        remove_dependency(&mut state, self.owner_script_id.as_deref(), &self.script_id);
        drop(state);
        self.queue.changed.notify_all();
    }
}

fn creates_dependency_cycle(
    dependencies: &HashMap<String, HashMap<String, usize>>,
    owner_script_id: &str,
    target_script_id: &str,
) -> bool {
    let mut visited = HashSet::new();
    let mut pending = vec![target_script_id];
    while let Some(current) = pending.pop() {
        if current == owner_script_id {
            return true;
        }
        if !visited.insert(current) {
            continue;
        }
        if let Some(targets) = dependencies.get(current) {
            pending.extend(targets.keys().map(String::as_str));
        }
    }
    false
}

fn add_dependency(state: &mut QueueState, owner_script_id: &str, script_id: &str) {
    let count = state
        .dependencies
        .entry(owner_script_id.to_owned())
        .or_default()
        .entry(script_id.to_owned())
        .or_default();
    *count = count.saturating_add(1);
}

fn remove_dependency(state: &mut QueueState, owner_script_id: Option<&str>, script_id: &str) {
    let Some(owner_script_id) = owner_script_id else {
        return;
    };
    let should_remove_owner = if let Some(targets) = state.dependencies.get_mut(owner_script_id) {
        if let Some(count) = targets.get_mut(script_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                targets.remove(script_id);
            }
        }
        targets.is_empty()
    } else {
        false
    };
    if should_remove_owner {
        state.dependencies.remove(owner_script_id);
    }
}

fn has_active_capacity(state: &QueueState, script_id: &str) -> bool {
    limit_permits_next(state.policy.max_active_runs_global, state.active_total)
        && limit_permits_next(
            state.policy.max_active_runs_per_script,
            state
                .active_runs
                .get(script_id)
                .copied()
                .unwrap_or_default(),
        )
}

fn limit_permits_next(limit: ResourceLimit, current: usize) -> bool {
    let Some(next) = current.checked_add(1) else {
        return false;
    };
    limit.permits(u64::try_from(next).unwrap_or(u64::MAX))
}

fn remove_waiter(state: &mut QueueState, script_id: &str, waiter: &Arc<Waiter>) {
    let should_remove_queue = if let Some(waiting) = state.waiting.get_mut(script_id) {
        if let Some(index) = waiting
            .iter()
            .position(|candidate| Arc::ptr_eq(candidate, waiter))
        {
            waiting.remove(index);
        }
        waiting.is_empty()
    } else {
        false
    };
    if should_remove_queue {
        state.waiting.remove(script_id);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, mpsc},
        thread,
        time::Duration,
    };

    use super::*;

    #[test]
    fn queues_the_same_script_until_the_active_run_finishes() {
        let queue = Arc::new(ScriptExecutionQueue::default());
        let first = queue
            .acquire("script-1", &RuntimeCancellationToken::new(), || false)
            .expect("first run should acquire its script");
        let (acquired_sender, acquired_receiver) = mpsc::channel();
        let thread_queue = Arc::clone(&queue);
        let waiter = thread::spawn(move || {
            let _permit = thread_queue
                .acquire("script-1", &RuntimeCancellationToken::new(), || false)
                .expect("queued run should eventually acquire its script");
            acquired_sender
                .send(())
                .expect("acquired signal should send");
        });

        assert!(
            acquired_receiver
                .recv_timeout(Duration::from_millis(100))
                .is_err()
        );
        drop(first);
        acquired_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("queued run should start after the first finishes");
        waiter.join().expect("waiter thread should finish");
    }

    #[test]
    fn permits_different_scripts_at_the_same_time() {
        let queue = ScriptExecutionQueue::default();
        let _first = queue
            .acquire("script-1", &RuntimeCancellationToken::new(), || false)
            .expect("first script should acquire");
        let _second = queue
            .acquire("script-2", &RuntimeCancellationToken::new(), || false)
            .expect("different script should acquire concurrently");
    }

    #[test]
    fn global_and_per_script_active_limits_are_enforced_independently() {
        let global_queue = Arc::new(ScriptExecutionQueue::new(ExecutionAdmissionPolicy {
            max_active_runs_global: ResourceLimit::limited(1),
            max_active_runs_per_script: ResourceLimit::Unlimited,
            max_queued_activations_per_script: ResourceLimit::Unlimited,
            queue_overflow_strategy: QueueOverflowStrategy::RejectNewest,
        }));
        let global_first = global_queue
            .acquire("script-1", &RuntimeCancellationToken::new(), || false)
            .expect("first global run should acquire");
        let (global_sender, global_receiver) = mpsc::channel();
        let waiting_global_queue = Arc::clone(&global_queue);
        let global_waiter = thread::spawn(move || {
            let result = waiting_global_queue
                .acquire("script-2", &RuntimeCancellationToken::new(), || false)
                .map(|_permit| ());
            global_sender
                .send(result)
                .expect("global result should send");
        });
        assert!(
            global_receiver
                .recv_timeout(Duration::from_millis(100))
                .is_err(),
            "the global active limit should block another script"
        );
        drop(global_first);
        assert_eq!(
            global_receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("global waiter should finish"),
            Ok(())
        );
        global_waiter.join().expect("global waiter should join");

        let script_queue = Arc::new(ScriptExecutionQueue::new(ExecutionAdmissionPolicy {
            max_active_runs_global: ResourceLimit::Unlimited,
            max_active_runs_per_script: ResourceLimit::limited(1),
            max_queued_activations_per_script: ResourceLimit::Unlimited,
            queue_overflow_strategy: QueueOverflowStrategy::RejectNewest,
        }));
        let script_first = script_queue
            .acquire("script-1", &RuntimeCancellationToken::new(), || false)
            .expect("first script run should acquire");
        let other_script = script_queue
            .acquire("script-2", &RuntimeCancellationToken::new(), || false)
            .expect("another script should not consume the per-script limit");
        let (script_sender, script_receiver) = mpsc::channel();
        let waiting_script_queue = Arc::clone(&script_queue);
        let script_waiter = thread::spawn(move || {
            let result = waiting_script_queue
                .acquire("script-1", &RuntimeCancellationToken::new(), || false)
                .map(|_permit| ());
            script_sender
                .send(result)
                .expect("script result should send");
        });
        assert!(
            script_receiver
                .recv_timeout(Duration::from_millis(100))
                .is_err(),
            "the per-script active limit should block the same script"
        );
        drop(script_first);
        assert_eq!(
            script_receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("script waiter should finish"),
            Ok(())
        );
        drop(other_script);
        script_waiter.join().expect("script waiter should join");
    }

    #[test]
    fn bounds_the_waiting_queue_for_one_script() {
        let queue = ScriptExecutionQueue::default();
        queue
            .state
            .lock()
            .expect("queue state should lock")
            .waiting
            .insert(
                "script-1".to_owned(),
                (0..64)
                    .map(|_| {
                        Arc::new(Waiter {
                            superseded: AtomicBool::new(false),
                        })
                    })
                    .collect(),
            );

        assert!(matches!(
            queue.acquire("script-1", &RuntimeCancellationToken::new(), || false),
            Err(AcquireError::Full)
        ));
    }

    #[test]
    fn drop_oldest_queue_policy_supersedes_only_the_oldest_waiter() {
        let queue = Arc::new(ScriptExecutionQueue::new(ExecutionAdmissionPolicy {
            max_active_runs_global: ResourceLimit::limited(1),
            max_active_runs_per_script: ResourceLimit::limited(1),
            max_queued_activations_per_script: ResourceLimit::limited(1),
            queue_overflow_strategy: QueueOverflowStrategy::DropOldest,
        }));
        let active = queue
            .acquire("script", &RuntimeCancellationToken::new(), || false)
            .expect("active run should acquire");

        let (old_sender, old_receiver) = mpsc::channel();
        let old_queue = Arc::clone(&queue);
        let old_waiter = thread::spawn(move || {
            let result = old_queue
                .acquire("script", &RuntimeCancellationToken::new(), || false)
                .map(|_permit| ());
            old_sender.send(result).expect("old result should send");
        });
        wait_for_waiter_count(&queue, "script", 1);

        let (new_sender, new_receiver) = mpsc::channel();
        let new_queue = Arc::clone(&queue);
        let new_waiter = thread::spawn(move || {
            let result = new_queue
                .acquire("script", &RuntimeCancellationToken::new(), || false)
                .map(|_permit| ());
            new_sender.send(result).expect("new result should send");
        });

        assert_eq!(
            old_receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("old waiter should be superseded"),
            Err(AcquireError::Superseded)
        );
        assert!(
            new_receiver
                .recv_timeout(Duration::from_millis(100))
                .is_err(),
            "replacement waiter should remain queued until capacity is available"
        );
        drop(active);
        assert_eq!(
            new_receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("replacement waiter should acquire"),
            Ok(())
        );
        old_waiter.join().expect("old waiter should join");
        new_waiter.join().expect("new waiter should join");
    }

    #[test]
    fn cancellation_removes_a_queued_run() {
        let queue = Arc::new(ScriptExecutionQueue::default());
        let _first = queue
            .acquire("script-1", &RuntimeCancellationToken::new(), || false)
            .expect("first run should acquire its script");
        let cancellation = RuntimeCancellationToken::new();
        let thread_cancellation = cancellation.clone();
        let thread_queue = Arc::clone(&queue);
        let waiter = thread::spawn(move || {
            thread_queue
                .acquire("script-1", &thread_cancellation, || false)
                .map(|_permit| ())
        });

        cancellation.cancel();
        assert!(matches!(waiter.join(), Ok(Err(AcquireError::Cancelled))));
    }

    #[test]
    fn policy_rejection_removes_a_queued_run() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let queue = Arc::new(ScriptExecutionQueue::default());
        let _first = queue
            .acquire("script-1", &RuntimeCancellationToken::new(), || false)
            .expect("first run should acquire its script");
        let rejected = Arc::new(AtomicBool::new(false));
        let thread_rejected = Arc::clone(&rejected);
        let thread_queue = Arc::clone(&queue);
        let waiter = thread::spawn(move || {
            thread_queue
                .acquire("script-1", &RuntimeCancellationToken::new(), || {
                    thread_rejected.load(Ordering::Relaxed)
                })
                .map(|_permit| ())
        });

        rejected.store(true, Ordering::Relaxed);
        assert!(matches!(waiter.join(), Ok(Err(AcquireError::Rejected))));
    }

    #[test]
    fn rejects_a_nested_wait_that_would_create_a_deadlock() {
        let queue = Arc::new(ScriptExecutionQueue::default());
        let _script_a = queue
            .acquire("script-a", &RuntimeCancellationToken::new(), || false)
            .expect("script A should acquire");
        let script_b = queue
            .acquire("script-b", &RuntimeCancellationToken::new(), || false)
            .expect("script B should acquire");
        let cancellation = RuntimeCancellationToken::new();
        let thread_cancellation = cancellation.clone();
        let thread_queue = Arc::clone(&queue);
        let waiter = thread::spawn(move || {
            thread_queue
                .acquire_nested("script-a", "script-b", &thread_cancellation, || false)
                .map(|_permit| ())
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while std::time::Instant::now() < deadline {
            if queue
                .lock_state()
                .dependencies
                .get("script-a")
                .is_some_and(|targets| targets.contains_key("script-b"))
            {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(matches!(
            queue.acquire_nested(
                "script-b",
                "script-a",
                &RuntimeCancellationToken::new(),
                || false,
            ),
            Err(AcquireError::Busy)
        ));

        cancellation.cancel();
        drop(script_b);
        assert!(matches!(waiter.join(), Ok(Err(AcquireError::Cancelled))));
    }

    fn wait_for_waiter_count(queue: &ScriptExecutionQueue, script_id: &str, expected: usize) {
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while queue.waiting_count(script_id) != expected {
            assert!(
                std::time::Instant::now() < deadline,
                "queue did not reach {expected} waiter(s)"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }
}
