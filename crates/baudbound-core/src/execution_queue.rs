use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use baudbound_runtime::RuntimeCancellationToken;

#[derive(Default)]
pub(crate) struct ScriptExecutionQueue {
    changed: Condvar,
    state: Mutex<QueueState>,
}

#[derive(Default)]
struct QueueState {
    active_scripts: HashSet<String>,
    dependencies: HashMap<String, String>,
    waiting: HashMap<String, VecDeque<Arc<Waiter>>>,
}

struct Waiter;

pub(crate) struct ScriptExecutionPermit<'a> {
    owner_script_id: Option<String>,
    queue: &'a ScriptExecutionQueue,
    script_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AcquireError {
    Cancelled,
    Busy,
}

impl ScriptExecutionQueue {
    pub(crate) fn acquire(
        &self,
        script_id: &str,
        cancellation: &RuntimeCancellationToken,
    ) -> Result<ScriptExecutionPermit<'_>, AcquireError> {
        self.acquire_internal(script_id, None, cancellation)
    }

    pub(crate) fn acquire_nested(
        &self,
        owner_script_id: &str,
        script_id: &str,
        cancellation: &RuntimeCancellationToken,
    ) -> Result<ScriptExecutionPermit<'_>, AcquireError> {
        self.acquire_internal(script_id, Some(owner_script_id), cancellation)
    }

    fn acquire_internal(
        &self,
        script_id: &str,
        owner_script_id: Option<&str>,
        cancellation: &RuntimeCancellationToken,
    ) -> Result<ScriptExecutionPermit<'_>, AcquireError> {
        const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(50);

        let waiter = Arc::new(Waiter);
        let mut state = self.lock_state();
        if let Some(owner_script_id) = owner_script_id {
            if state.dependencies.contains_key(owner_script_id)
                || creates_dependency_cycle(&state.dependencies, owner_script_id, script_id)
            {
                return Err(AcquireError::Busy);
            }
            state
                .dependencies
                .insert(owner_script_id.to_owned(), script_id.to_owned());
        }
        state
            .waiting
            .entry(script_id.to_owned())
            .or_default()
            .push_back(Arc::clone(&waiter));

        loop {
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
            if is_first && !state.active_scripts.contains(script_id) {
                remove_waiter(&mut state, script_id, &waiter);
                state.active_scripts.insert(script_id.to_owned());
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

impl Drop for ScriptExecutionPermit<'_> {
    fn drop(&mut self) {
        let mut state = self.queue.lock_state();
        state.active_scripts.remove(&self.script_id);
        remove_dependency(&mut state, self.owner_script_id.as_deref(), &self.script_id);
        drop(state);
        self.queue.changed.notify_all();
    }
}

fn creates_dependency_cycle(
    dependencies: &HashMap<String, String>,
    owner_script_id: &str,
    target_script_id: &str,
) -> bool {
    let mut current = target_script_id;
    let mut visited = HashSet::new();
    while visited.insert(current) {
        if current == owner_script_id {
            return true;
        }
        let Some(next) = dependencies.get(current) else {
            return false;
        };
        current = next;
    }
    true
}

fn remove_dependency(state: &mut QueueState, owner_script_id: Option<&str>, script_id: &str) {
    if let Some(owner_script_id) = owner_script_id
        && state
            .dependencies
            .get(owner_script_id)
            .is_some_and(|target| target == script_id)
    {
        state.dependencies.remove(owner_script_id);
    }
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
            .acquire("script-1", &RuntimeCancellationToken::new())
            .expect("first run should acquire its script");
        let (acquired_sender, acquired_receiver) = mpsc::channel();
        let thread_queue = Arc::clone(&queue);
        let waiter = thread::spawn(move || {
            let _permit = thread_queue
                .acquire("script-1", &RuntimeCancellationToken::new())
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
            .acquire("script-1", &RuntimeCancellationToken::new())
            .expect("first script should acquire");
        let _second = queue
            .acquire("script-2", &RuntimeCancellationToken::new())
            .expect("different script should acquire concurrently");
    }

    #[test]
    fn cancellation_removes_a_queued_run() {
        let queue = Arc::new(ScriptExecutionQueue::default());
        let _first = queue
            .acquire("script-1", &RuntimeCancellationToken::new())
            .expect("first run should acquire its script");
        let cancellation = RuntimeCancellationToken::new();
        let thread_cancellation = cancellation.clone();
        let thread_queue = Arc::clone(&queue);
        let waiter = thread::spawn(move || {
            thread_queue
                .acquire("script-1", &thread_cancellation)
                .map(|_permit| ())
        });

        cancellation.cancel();
        assert!(matches!(waiter.join(), Ok(Err(AcquireError::Cancelled))));
    }

    #[test]
    fn rejects_a_nested_wait_that_would_create_a_deadlock() {
        let queue = Arc::new(ScriptExecutionQueue::default());
        let _script_a = queue
            .acquire("script-a", &RuntimeCancellationToken::new())
            .expect("script A should acquire");
        let script_b = queue
            .acquire("script-b", &RuntimeCancellationToken::new())
            .expect("script B should acquire");
        let cancellation = RuntimeCancellationToken::new();
        let thread_cancellation = cancellation.clone();
        let thread_queue = Arc::clone(&queue);
        let waiter = thread::spawn(move || {
            thread_queue
                .acquire_nested("script-a", "script-b", &thread_cancellation)
                .map(|_permit| ())
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while std::time::Instant::now() < deadline {
            if queue
                .lock_state()
                .dependencies
                .get("script-a")
                .is_some_and(|target| target == "script-b")
            {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(matches!(
            queue.acquire_nested("script-b", "script-a", &RuntimeCancellationToken::new()),
            Err(AcquireError::Busy)
        ));

        cancellation.cancel();
        drop(script_b);
        assert!(matches!(waiter.join(), Ok(Err(AcquireError::Cancelled))));
    }
}
