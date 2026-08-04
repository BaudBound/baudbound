use std::{
    collections::{HashMap, VecDeque},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        mpsc::{Receiver, Sender, channel},
    },
    thread::{self, JoinHandle},
};

use baudbound_core::{QueueOverflowStrategy, RunReport, RunnerCore, TriggerEvent};
use baudbound_runtime::{ResourceLimit, RuntimeCancellationToken};
use baudbound_storage::SqliteRunnerStore;

use crate::trigger_monitor::{TriggerMonitor, TriggerMonitorStatus};

pub(super) type TriggerRunner =
    dyn Fn(TriggerEvent) -> Result<RunReport, String> + Send + Sync + 'static;

pub(super) struct TriggerExecutor {
    accepting: bool,
    active_by_script: HashMap<String, usize>,
    active_total: usize,
    completion_receiver: Receiver<TriggerCompletion>,
    completion_sender: Sender<TriggerCompletion>,
    local_completions: VecDeque<TriggerCompletion>,
    next_job_id: u64,
    pending_jobs: usize,
    policy: TriggerExecutionPolicy,
    queued_by_script: HashMap<String, usize>,
    queued_jobs: VecDeque<TriggerJob>,
    runner: Arc<TriggerRunner>,
    cancellation: RuntimeCancellationToken,
    trigger_monitor: Option<TriggerMonitor>,
    worker_label: String,
    workers: HashMap<u64, JoinHandle<()>>,
}

pub(super) struct TriggerCompletion {
    pub(super) event: TriggerEvent,
    pub(super) job_id: u64,
    pub(super) result: Result<RunReport, String>,
    pub(super) source: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TriggerSubmitError {
    Full,
    Stopped,
}

struct TriggerJob {
    event: TriggerEvent,
    job_id: u64,
    source: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct TriggerExecutionPolicy {
    max_active_global: ResourceLimit,
    max_active_per_script: ResourceLimit,
    max_queued_per_script: ResourceLimit,
    overflow_strategy: QueueOverflowStrategy,
}

impl TriggerExecutor {
    pub(super) fn new(
        core: &RunnerCore,
        store: &SqliteRunnerStore,
        worker_label: &str,
        parent_cancellation: &RuntimeCancellationToken,
        trigger_monitor: Option<TriggerMonitor>,
    ) -> Result<Self, String> {
        let cancellation = parent_cancellation.child_token();
        let core = core.clone();
        let policy = core.execution_admission_policy();
        let store = store.clone();
        let run_cancellation = cancellation.clone();
        let runner = Arc::new(move |event: TriggerEvent| {
            core.dispatch_trigger_event_with_cancellation(
                &store,
                event,
                run_cancellation.child_token(),
            )
            .map_err(|error| error.to_string())
        });
        Self::with_policy_and_cancellation(
            TriggerExecutionPolicy {
                max_active_global: policy.max_active_runs_global,
                max_active_per_script: policy.max_active_runs_per_script,
                max_queued_per_script: policy.max_queued_activations_per_script,
                overflow_strategy: policy.queue_overflow_strategy,
            },
            worker_label,
            runner,
            cancellation,
            trigger_monitor,
        )
    }

    pub(super) fn submit_from(
        &mut self,
        event: TriggerEvent,
        source: &'static str,
    ) -> Result<u64, TriggerSubmitError> {
        self.collect_available_completions();
        let monitored_event = event.clone();
        let result = self.try_submit_from(event, source);
        if let Some(monitor) = &self.trigger_monitor {
            let (status, error) = match result {
                Ok(_) => (TriggerMonitorStatus::Queued, None),
                Err(TriggerSubmitError::Full) => (
                    TriggerMonitorStatus::Rejected,
                    Some("trigger execution queue is at capacity"),
                ),
                Err(TriggerSubmitError::Stopped) => (
                    TriggerMonitorStatus::Rejected,
                    Some("trigger execution workers are unavailable"),
                ),
            };
            monitor.observe_submission(&monitored_event, source, status, error);
        }
        result
    }

    fn try_submit_from(
        &mut self,
        event: TriggerEvent,
        source: &'static str,
    ) -> Result<u64, TriggerSubmitError> {
        if !self.accepting {
            return Err(TriggerSubmitError::Stopped);
        }

        let job_id = self.next_job_id;
        let next_job_id = self
            .next_job_id
            .checked_add(1)
            .ok_or(TriggerSubmitError::Stopped)?;
        let next_pending_jobs = self
            .pending_jobs
            .checked_add(1)
            .ok_or(TriggerSubmitError::Stopped)?;
        let job = TriggerJob {
            event,
            job_id,
            source,
        };

        if self.has_active_capacity(&job.event.script_id) {
            self.start_job(&job)
                .map_err(|_| TriggerSubmitError::Stopped)?;
        } else {
            self.enqueue_job(job)?;
        }
        self.next_job_id = next_job_id;
        self.pending_jobs = next_pending_jobs;
        Ok(job_id)
    }

    fn enqueue_job(&mut self, job: TriggerJob) -> Result<(), TriggerSubmitError> {
        let script_id = job.event.script_id.clone();
        let queued = self
            .queued_by_script
            .get(&script_id)
            .copied()
            .unwrap_or_default();
        if !limit_permits_next(self.policy.max_queued_per_script, queued) {
            match self.policy.overflow_strategy {
                QueueOverflowStrategy::RejectNewest => return Err(TriggerSubmitError::Full),
                QueueOverflowStrategy::DropOldest => {
                    let Some(index) = self
                        .queued_jobs
                        .iter()
                        .position(|candidate| candidate.event.script_id == script_id)
                    else {
                        return Err(TriggerSubmitError::Full);
                    };
                    let superseded = self
                        .queued_jobs
                        .remove(index)
                        .expect("located queued activation must remain present");
                    decrement_count(&mut self.queued_by_script, &script_id);
                    self.local_completions.push_back(TriggerCompletion {
                        event: superseded.event,
                        job_id: superseded.job_id,
                        result: Err(
                            "a newer activation replaced this queued run under the configured drop_oldest policy"
                                .to_owned(),
                        ),
                        source: superseded.source,
                    });
                }
            }
        }
        self.queued_jobs.push_back(job);
        *self.queued_by_script.entry(script_id).or_default() += 1;
        Ok(())
    }

    fn start_job(&mut self, job: &TriggerJob) -> Result<(), String> {
        let event = job.event.clone();
        let runner_event = event.clone();
        let script_id = event.script_id.clone();
        let next_active_total = self
            .active_total
            .checked_add(1)
            .ok_or_else(|| "active trigger execution count was exhausted".to_owned())?;
        let next_active_for_script = self
            .active_by_script
            .get(&script_id)
            .copied()
            .unwrap_or_default()
            .checked_add(1)
            .ok_or_else(|| {
                format!("active trigger execution count for script {script_id:?} was exhausted")
            })?;
        let job_id = job.job_id;
        let source = job.source;
        let runner = Arc::clone(&self.runner);
        let completion_sender = self.completion_sender.clone();
        let worker = thread::Builder::new()
            .name(format!("baudbound-{}-{job_id}", self.worker_label))
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| runner(runner_event)))
                    .unwrap_or_else(|_| Err("trigger execution worker panicked".to_owned()));
                let _ = completion_sender.send(TriggerCompletion {
                    event,
                    job_id,
                    result,
                    source,
                });
            })
            .map_err(|source| format!("failed to spawn trigger execution worker: {source}"))?;
        self.active_total = next_active_total;
        self.active_by_script
            .insert(script_id, next_active_for_script);
        debug_assert!(self.workers.insert(job_id, worker).is_none());
        Ok(())
    }

    pub(super) fn try_completion(&mut self) -> Option<TriggerCompletion> {
        self.collect_available_completions();
        let completion = self.local_completions.pop_front()?;
        self.pending_jobs = self.pending_jobs.saturating_sub(1);
        Some(completion)
    }

    fn collect_available_completions(&mut self) {
        while let Ok(completion) = self.completion_receiver.try_recv() {
            self.finish_worker(&completion);
            self.local_completions.push_back(completion);
        }
        self.schedule_queued_jobs();
    }

    fn finish_worker(&mut self, completion: &TriggerCompletion) {
        if let Some(worker) = self.workers.remove(&completion.job_id) {
            let _ = worker.join();
        }
        self.active_total = self.active_total.saturating_sub(1);
        decrement_count(&mut self.active_by_script, &completion.event.script_id);
    }

    fn schedule_queued_jobs(&mut self) {
        loop {
            if !self
                .policy
                .max_active_global
                .permits(u64::try_from(self.active_total.saturating_add(1)).unwrap_or(u64::MAX))
            {
                return;
            }
            let Some(index) = self
                .queued_jobs
                .iter()
                .position(|job| self.has_active_capacity(&job.event.script_id))
            else {
                return;
            };
            let job = self
                .queued_jobs
                .remove(index)
                .expect("located queued activation must remain present");
            decrement_count(&mut self.queued_by_script, &job.event.script_id);
            if let Err(error) = self.start_job(&job) {
                self.accepting = false;
                tracing::error!(%error, "trigger execution worker could not be started");
                self.local_completions.push_back(TriggerCompletion {
                    event: job.event,
                    job_id: job.job_id,
                    result: Err(error.clone()),
                    source: job.source,
                });
                self.fail_all_queued_jobs(error);
                return;
            }
        }
    }

    fn fail_all_queued_jobs(&mut self, error: String) {
        for job in self.queued_jobs.drain(..) {
            self.local_completions.push_back(TriggerCompletion {
                event: job.event,
                job_id: job.job_id,
                result: Err(error.clone()),
                source: job.source,
            });
        }
        self.queued_by_script.clear();
    }

    fn has_active_capacity(&self, script_id: &str) -> bool {
        limit_permits_next(self.policy.max_active_global, self.active_total)
            && limit_permits_next(
                self.policy.max_active_per_script,
                self.active_by_script
                    .get(script_id)
                    .copied()
                    .unwrap_or_default(),
            )
    }

    pub(super) fn has_pending(&self) -> bool {
        self.pending_jobs > 0
    }

    #[cfg(test)]
    pub(super) fn with_runner(
        worker_count: usize,
        queue_capacity: usize,
        worker_label: &str,
        runner: Arc<TriggerRunner>,
    ) -> Result<Self, String> {
        Self::with_runner_and_cancellation(
            worker_count,
            queue_capacity,
            worker_label,
            runner,
            RuntimeCancellationToken::new(),
            None,
        )
    }

    #[cfg(test)]
    fn with_runner_and_cancellation(
        worker_count: usize,
        queue_capacity: usize,
        worker_label: &str,
        runner: Arc<TriggerRunner>,
        cancellation: RuntimeCancellationToken,
        trigger_monitor: Option<TriggerMonitor>,
    ) -> Result<Self, String> {
        Self::with_policy_and_cancellation(
            TriggerExecutionPolicy {
                max_active_global: ResourceLimit::limited(
                    u64::try_from(worker_count.max(1)).unwrap_or(u64::MAX),
                ),
                max_active_per_script: ResourceLimit::limited(1),
                max_queued_per_script: ResourceLimit::limited(
                    u64::try_from(queue_capacity.max(1)).unwrap_or(u64::MAX),
                ),
                overflow_strategy: QueueOverflowStrategy::RejectNewest,
            },
            worker_label,
            runner,
            cancellation,
            trigger_monitor,
        )
    }

    fn with_policy_and_cancellation(
        policy: TriggerExecutionPolicy,
        worker_label: &str,
        runner: Arc<TriggerRunner>,
        cancellation: RuntimeCancellationToken,
        trigger_monitor: Option<TriggerMonitor>,
    ) -> Result<Self, String> {
        if worker_label.trim().is_empty() {
            return Err("trigger execution worker label cannot be empty".to_owned());
        }
        let (completion_sender, completion_receiver) = channel();
        Ok(Self {
            accepting: true,
            active_by_script: HashMap::new(),
            active_total: 0,
            completion_receiver,
            completion_sender,
            local_completions: VecDeque::new(),
            next_job_id: 1,
            pending_jobs: 0,
            policy,
            queued_by_script: HashMap::new(),
            queued_jobs: VecDeque::new(),
            runner,
            cancellation,
            trigger_monitor,
            worker_label: worker_label.to_owned(),
            workers: HashMap::new(),
        })
    }

    pub(super) fn shutdown(&mut self) -> Result<(), String> {
        self.accepting = false;
        self.cancellation.cancel();
        self.queued_jobs.clear();
        self.queued_by_script.clear();

        let mut panicked_workers = 0_usize;
        for (_, worker) in self.workers.drain() {
            if worker.join().is_err() {
                panicked_workers = panicked_workers.saturating_add(1);
            }
        }
        self.active_by_script.clear();
        self.active_total = 0;
        self.pending_jobs = 0;
        if panicked_workers > 0 {
            return Err(format!(
                "{panicked_workers} trigger execution worker(s) panicked during shutdown"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "executor_tests.rs"]
mod tests;

impl Drop for TriggerExecutor {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            tracing::error!(%error, "trigger executor shutdown failed");
        }
    }
}

fn limit_permits_next(limit: ResourceLimit, current: usize) -> bool {
    let Some(next) = current.checked_add(1) else {
        return false;
    };
    limit.permits(u64::try_from(next).unwrap_or(u64::MAX))
}

fn decrement_count(counts: &mut HashMap<String, usize>, key: &str) {
    let should_remove = if let Some(count) = counts.get_mut(key) {
        *count = count.saturating_sub(1);
        *count == 0
    } else {
        false
    };
    if should_remove {
        counts.remove(key);
    }
}
