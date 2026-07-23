use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, SyncSender, TrySendError, channel, sync_channel},
    },
    thread::{self, JoinHandle},
};

use baudbound_core::{RunReport, RunnerCore, TriggerEvent};
use baudbound_runtime::RuntimeCancellationToken;
use baudbound_storage::SqliteRunnerStore;

use crate::trigger_monitor::{TriggerMonitor, TriggerMonitorStatus};

const MAX_WORKERS: usize = 16;
const MIN_WORKERS: usize = 2;
const QUEUE_CAPACITY_PER_WORKER: usize = 8;

pub(super) type TriggerRunner =
    dyn Fn(TriggerEvent) -> Result<RunReport, String> + Send + Sync + 'static;

pub(super) struct TriggerExecutor {
    active_scripts: HashSet<String>,
    completion_receiver: Receiver<TriggerCompletion>,
    deferred_jobs: HashMap<String, VecDeque<TriggerJob>>,
    local_completions: VecDeque<TriggerCompletion>,
    max_pending_jobs: usize,
    next_job_id: u64,
    pending_jobs: usize,
    sender: Option<SyncSender<TriggerJob>>,
    cancellation: RuntimeCancellationToken,
    trigger_monitor: Option<TriggerMonitor>,
    workers: Vec<JoinHandle<()>>,
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

impl TriggerExecutor {
    pub(super) fn new(
        core: &RunnerCore,
        store: &SqliteRunnerStore,
        worker_label: &str,
        cancellation: RuntimeCancellationToken,
        trigger_monitor: Option<TriggerMonitor>,
    ) -> Result<Self, String> {
        let core = core.clone();
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
        let worker_count = thread::available_parallelism()
            .map_or(MIN_WORKERS, usize::from)
            .clamp(MIN_WORKERS, MAX_WORKERS);
        Self::with_runner_and_cancellation(
            worker_count,
            worker_count.saturating_mul(QUEUE_CAPACITY_PER_WORKER),
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
        if self.sender.is_none() {
            return Err(TriggerSubmitError::Stopped);
        }
        if self.pending_jobs >= self.max_pending_jobs {
            return Err(TriggerSubmitError::Full);
        }

        let job_id = self.next_job_id;
        self.next_job_id = self
            .next_job_id
            .checked_add(1)
            .ok_or(TriggerSubmitError::Stopped)?;
        let job = TriggerJob {
            event,
            job_id,
            source,
        };
        let script_id = job.event.script_id.clone();
        if self.active_scripts.contains(&script_id) {
            self.deferred_jobs
                .entry(script_id)
                .or_default()
                .push_back(job);
            self.pending_jobs = self.pending_jobs.saturating_add(1);
            return Ok(job_id);
        }

        let sender = self.sender.as_ref().ok_or(TriggerSubmitError::Stopped)?;
        match sender.try_send(job) {
            Ok(()) => {
                self.active_scripts.insert(script_id);
                self.pending_jobs = self.pending_jobs.saturating_add(1);
                Ok(job_id)
            }
            Err(TrySendError::Full(_)) => Err(TriggerSubmitError::Full),
            Err(TrySendError::Disconnected(_)) => Err(TriggerSubmitError::Stopped),
        }
    }

    pub(super) fn try_completion(&mut self) -> Option<TriggerCompletion> {
        let completion = self
            .local_completions
            .pop_front()
            .or_else(|| self.completion_receiver.try_recv().ok())?;
        self.pending_jobs = self.pending_jobs.saturating_sub(1);
        self.schedule_next_for_script(&completion.event.script_id);
        Some(completion)
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

    fn with_runner_and_cancellation(
        worker_count: usize,
        queue_capacity: usize,
        worker_label: &str,
        runner: Arc<TriggerRunner>,
        cancellation: RuntimeCancellationToken,
        trigger_monitor: Option<TriggerMonitor>,
    ) -> Result<Self, String> {
        let (sender, receiver) = sync_channel::<TriggerJob>(queue_capacity.max(1));
        let receiver = Arc::new(Mutex::new(receiver));
        let (completion_sender, completion_receiver) = channel();
        let mut workers: Vec<JoinHandle<()>> = Vec::with_capacity(worker_count.max(1));

        for worker_index in 0..worker_count.max(1) {
            let receiver = Arc::clone(&receiver);
            let completion_sender = completion_sender.clone();
            let runner = Arc::clone(&runner);
            let worker = match thread::Builder::new()
                .name(format!("baudbound-{worker_label}-{worker_index}"))
                .spawn(move || worker_loop(&receiver, &completion_sender, &runner))
            {
                Ok(worker) => worker,
                Err(source) => {
                    cancellation.cancel();
                    drop(sender);
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(format!(
                        "failed to spawn {worker_label} execution worker {worker_index}: {source}"
                    ));
                }
            };
            workers.push(worker);
        }

        Ok(Self {
            active_scripts: HashSet::new(),
            completion_receiver,
            deferred_jobs: HashMap::new(),
            local_completions: VecDeque::new(),
            max_pending_jobs: worker_count.max(1).saturating_add(queue_capacity.max(1)),
            next_job_id: 1,
            pending_jobs: 0,
            sender: Some(sender),
            cancellation,
            trigger_monitor,
            workers,
        })
    }

    fn schedule_next_for_script(&mut self, script_id: &str) {
        let next_job = self
            .deferred_jobs
            .get_mut(script_id)
            .and_then(VecDeque::pop_front);
        if self
            .deferred_jobs
            .get(script_id)
            .is_some_and(VecDeque::is_empty)
        {
            self.deferred_jobs.remove(script_id);
        }

        let Some(job) = next_job else {
            self.active_scripts.remove(script_id);
            return;
        };
        let Some(sender) = &self.sender else {
            self.queue_stopped_completion(job);
            return;
        };
        if let Err(error) = sender.send(job) {
            self.queue_stopped_completion(error.0);
        }
    }

    fn queue_stopped_completion(&mut self, job: TriggerJob) {
        self.local_completions.push_back(TriggerCompletion {
            event: job.event,
            job_id: job.job_id,
            result: Err("trigger execution workers are unavailable".to_owned()),
            source: job.source,
        });
    }

    pub(super) fn shutdown(&mut self) -> Result<(), String> {
        self.cancellation.cancel();
        self.sender.take();

        let mut panicked_workers = 0_usize;
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                panicked_workers = panicked_workers.saturating_add(1);
            }
        }
        if panicked_workers > 0 {
            return Err(format!(
                "{panicked_workers} trigger execution worker(s) panicked during shutdown"
            ));
        }
        Ok(())
    }
}

impl Drop for TriggerExecutor {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            tracing::error!(%error, "trigger executor shutdown failed");
        }
    }
}

fn worker_loop(
    receiver: &Mutex<Receiver<TriggerJob>>,
    completion_sender: &Sender<TriggerCompletion>,
    runner: &Arc<TriggerRunner>,
) {
    loop {
        let job = {
            let receiver = receiver
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            receiver.recv()
        };
        let Ok(job) = job else {
            return;
        };
        let event = job.event.clone();
        let result = runner(job.event);
        if completion_sender
            .send(TriggerCompletion {
                event,
                job_id: job.job_id,
                result,
                source: job.source,
            })
            .is_err()
        {
            return;
        }
    }
}

#[cfg(test)]
#[path = "executor_tests.rs"]
mod tests;
