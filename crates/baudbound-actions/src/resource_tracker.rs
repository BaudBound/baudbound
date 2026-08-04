use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use baudbound_runtime::ResourceLimit;

#[derive(Default)]
pub(crate) struct ActionResourceTracker {
    state: Mutex<ActionResourceState>,
}

#[derive(Default)]
struct ActionResourceState {
    active_processes: HashMap<String, usize>,
    file_write_bytes: HashMap<String, u64>,
    launch_history: HashMap<String, VecDeque<LaunchRecord>>,
    next_launch_id: u64,
}

#[derive(Clone, Copy)]
struct LaunchRecord {
    id: u64,
    timestamp: Instant,
}

pub(crate) struct ProcessLaunchPermit {
    launch_id: Option<u64>,
    script_id: String,
    spawned: bool,
    tracker: Arc<ActionResourceTracker>,
}

pub(crate) struct FileWriteBudget {
    committed: bool,
    limit: ResourceLimit,
    reserved: u64,
    run_id: String,
    tracker: Arc<ActionResourceTracker>,
}

impl ActionResourceTracker {
    pub(crate) fn reserve_process_launch(
        self: &Arc<Self>,
        script_id: &str,
        max_active: ResourceLimit,
        max_launches_per_minute: ResourceLimit,
    ) -> Result<ProcessLaunchPermit, String> {
        let mut state = self.lock();
        prune_launch_history(&mut state, script_id);
        let active = state
            .active_processes
            .get(script_id)
            .copied()
            .unwrap_or_default();
        if !limit_permits_next(max_active, active) {
            return Err(format!(
                "script {script_id:?} reached the configured {max_active} active process limit"
            ));
        }
        let launches = state.launch_history.get(script_id).map_or(0, VecDeque::len);
        if !limit_permits_next(max_launches_per_minute, launches) {
            return Err(format!(
                "script {script_id:?} reached the configured {max_launches_per_minute} process launches per minute limit"
            ));
        }
        let launch_id = if max_launches_per_minute.value().is_some() {
            state.next_launch_id = state
                .next_launch_id
                .checked_add(1)
                .ok_or_else(|| "process launch sequence was exhausted".to_owned())?;
            let launch_id = state.next_launch_id;
            state
                .launch_history
                .entry(script_id.to_owned())
                .or_default()
                .push_back(LaunchRecord {
                    id: launch_id,
                    timestamp: Instant::now(),
                });
            Some(launch_id)
        } else {
            None
        };
        let next_active = active
            .checked_add(1)
            .ok_or_else(|| "active process accounting overflowed".to_owned())?;
        state
            .active_processes
            .insert(script_id.to_owned(), next_active);
        drop(state);
        Ok(ProcessLaunchPermit {
            launch_id,
            script_id: script_id.to_owned(),
            spawned: false,
            tracker: Arc::clone(self),
        })
    }

    pub(crate) fn file_write_budget(
        self: &Arc<Self>,
        run_id: &str,
        limit: ResourceLimit,
    ) -> FileWriteBudget {
        FileWriteBudget {
            committed: false,
            limit,
            reserved: 0,
            run_id: run_id.to_owned(),
            tracker: Arc::clone(self),
        }
    }

    pub(crate) fn finish_run(&self, run_id: &str) {
        self.lock().file_write_bytes.remove(run_id);
    }

    fn lock(&self) -> MutexGuard<'_, ActionResourceState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl ProcessLaunchPermit {
    pub(crate) fn mark_spawned(&mut self) {
        self.spawned = true;
    }
}

impl Drop for ProcessLaunchPermit {
    fn drop(&mut self) {
        let mut state = self.tracker.lock();
        decrement_count(&mut state.active_processes, &self.script_id);
        if !self.spawned
            && let Some(launch_id) = self.launch_id
        {
            let remove_history =
                if let Some(history) = state.launch_history.get_mut(&self.script_id) {
                    if let Some(index) = history.iter().position(|record| record.id == launch_id) {
                        history.remove(index);
                    }
                    history.is_empty()
                } else {
                    false
                };
            if remove_history {
                state.launch_history.remove(&self.script_id);
            }
        }
    }
}

impl FileWriteBudget {
    pub(crate) fn account(&mut self, bytes: u64) -> Result<(), String> {
        if self.limit == ResourceLimit::Unlimited {
            return Ok(());
        }
        let mut state = self.tracker.lock();
        let current = state
            .file_write_bytes
            .get(&self.run_id)
            .copied()
            .unwrap_or_default();
        let next = current
            .checked_add(bytes)
            .ok_or_else(|| "per-run file write accounting overflowed".to_owned())?;
        if self.limit.is_exceeded_by(next) {
            return Err(format!(
                "run {:?} would exceed the configured {} byte file-write limit",
                self.run_id, self.limit
            ));
        }
        state.file_write_bytes.insert(self.run_id.clone(), next);
        self.reserved = self
            .reserved
            .checked_add(bytes)
            .ok_or_else(|| "file write reservation accounting overflowed".to_owned())?;
        Ok(())
    }

    pub(crate) fn commit(mut self) {
        self.committed = true;
    }

    pub(crate) fn release(&mut self, bytes: u64) {
        let released = bytes.min(self.reserved);
        if released == 0 {
            return;
        }
        let mut state = self.tracker.lock();
        let should_remove = if let Some(total) = state.file_write_bytes.get_mut(&self.run_id) {
            *total = total.saturating_sub(released);
            *total == 0
        } else {
            false
        };
        if should_remove {
            state.file_write_bytes.remove(&self.run_id);
        }
        self.reserved = self.reserved.saturating_sub(released);
    }
}

impl Drop for FileWriteBudget {
    fn drop(&mut self) {
        if self.committed || self.reserved == 0 {
            return;
        }
        let mut state = self.tracker.lock();
        let should_remove = if let Some(total) = state.file_write_bytes.get_mut(&self.run_id) {
            *total = total.saturating_sub(self.reserved);
            *total == 0
        } else {
            false
        };
        if should_remove {
            state.file_write_bytes.remove(&self.run_id);
        }
    }
}

fn prune_launch_history(state: &mut ActionResourceState, script_id: &str) {
    let cutoff = Instant::now()
        .checked_sub(Duration::from_secs(60))
        .unwrap_or_else(Instant::now);
    let remove = if let Some(history) = state.launch_history.get_mut(script_id) {
        while history
            .front()
            .is_some_and(|record| record.timestamp <= cutoff)
        {
            history.pop_front();
        }
        history.is_empty()
    } else {
        false
    };
    if remove {
        state.launch_history.remove(script_id);
    }
}

fn limit_permits_next(limit: ResourceLimit, current: usize) -> bool {
    current
        .checked_add(1)
        .is_some_and(|next| limit.permits(u64::try_from(next).unwrap_or(u64::MAX)))
}

fn decrement_count(counts: &mut HashMap<String, usize>, key: &str) {
    let remove = if let Some(count) = counts.get_mut(key) {
        *count = count.saturating_sub(1);
        *count == 0
    } else {
        false
    };
    if remove {
        counts.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlimited_process_launch_rate_keeps_no_history() {
        let tracker = Arc::new(ActionResourceTracker::default());
        let mut permit = tracker
            .reserve_process_launch("script", ResourceLimit::Unlimited, ResourceLimit::Unlimited)
            .expect("unlimited process launch should be admitted");
        permit.mark_spawned();
        drop(permit);

        let state = tracker.lock();
        assert!(state.launch_history.is_empty());
        assert!(state.active_processes.is_empty());
    }

    #[test]
    fn unlimited_file_write_budget_keeps_no_accounting_state() {
        let tracker = Arc::new(ActionResourceTracker::default());
        let mut budget = tracker.file_write_budget("run", ResourceLimit::Unlimited);
        budget
            .account(u64::MAX)
            .expect("unlimited file-write accounting should not overflow");
        budget.commit();

        assert!(tracker.lock().file_write_bytes.is_empty());
    }

    #[test]
    fn finite_process_concurrency_and_launch_rate_are_enforced_independently() {
        let tracker = Arc::new(ActionResourceTracker::default());
        let active = tracker
            .reserve_process_launch(
                "script",
                ResourceLimit::limited(1),
                ResourceLimit::Unlimited,
            )
            .expect("first active process should be admitted");
        let active_error = tracker
            .reserve_process_launch(
                "script",
                ResourceLimit::limited(1),
                ResourceLimit::Unlimited,
            )
            .err()
            .expect("second active process should be rejected");
        assert!(active_error.contains("active process limit"));
        drop(active);

        let mut launched = tracker
            .reserve_process_launch(
                "script",
                ResourceLimit::Unlimited,
                ResourceLimit::limited(1),
            )
            .expect("first launch in the rate window should be admitted");
        launched.mark_spawned();
        drop(launched);
        let rate_error = tracker
            .reserve_process_launch(
                "script",
                ResourceLimit::Unlimited,
                ResourceLimit::limited(1),
            )
            .err()
            .expect("second launch in the rate window should be rejected");
        assert!(rate_error.contains("process launches per minute limit"));

        tracker
            .reserve_process_launch(
                "other-script",
                ResourceLimit::Unlimited,
                ResourceLimit::limited(1),
            )
            .expect("launch-rate accounting should be isolated per script");
    }

    #[test]
    fn finite_file_write_budget_is_cumulative_and_released_on_failed_actions() {
        let tracker = Arc::new(ActionResourceTracker::default());
        let mut first = tracker.file_write_budget("run", ResourceLimit::limited(4));
        first.account(3).expect("first write should fit");
        first.commit();

        let mut oversized = tracker.file_write_budget("run", ResourceLimit::limited(4));
        assert!(oversized.account(2).is_err());
        drop(oversized);

        let mut final_byte = tracker.file_write_budget("run", ResourceLimit::limited(4));
        final_byte.account(1).expect("remaining byte should fit");
        final_byte.commit();
        assert_eq!(tracker.lock().file_write_bytes.get("run"), Some(&4));

        tracker.finish_run("run");
        let mut next_run = tracker.file_write_budget("run", ResourceLimit::limited(4));
        next_run
            .account(4)
            .expect("finishing a run should release its accounting");

        let mut rolled_back = tracker.file_write_budget("rollback", ResourceLimit::limited(4));
        rolled_back.account(4).expect("reservation should fit");
        drop(rolled_back);
        let mut replacement = tracker.file_write_budget("rollback", ResourceLimit::limited(4));
        replacement
            .account(4)
            .expect("an uncommitted reservation should be released");
    }

    #[test]
    fn repeated_resource_accounting_stays_bounded_and_releases_active_state() {
        const ITERATIONS: u64 = 10_000;

        let tracker = Arc::new(ActionResourceTracker::default());
        for _ in 0..ITERATIONS {
            let mut permit = tracker
                .reserve_process_launch(
                    "script",
                    ResourceLimit::limited(1),
                    ResourceLimit::Unlimited,
                )
                .expect("released process permits must remain reusable");
            permit.mark_spawned();
        }
        {
            let state = tracker.lock();
            assert!(state.active_processes.is_empty());
            assert!(state.launch_history.is_empty());
        }

        for _ in 0..ITERATIONS {
            let mut budget = tracker.file_write_budget("run", ResourceLimit::limited(ITERATIONS));
            budget.account(1).expect("cumulative write should fit");
            budget.commit();
        }
        assert_eq!(
            tracker.lock().file_write_bytes.get("run"),
            Some(&ITERATIONS)
        );
        tracker.finish_run("run");
        assert!(tracker.lock().file_write_bytes.is_empty());
    }
}
