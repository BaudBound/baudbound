use std::{
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, SyncSender, TrySendError, channel, sync_channel},
    },
    thread::{self, JoinHandle},
};

use baudbound_core::{RunReport, RunnerCore, TriggerEvent};
use baudbound_runtime::RuntimeCancellationToken;
use baudbound_storage::SqliteRunnerStore;

const MAX_WORKERS: usize = 16;
const MIN_WORKERS: usize = 2;
const QUEUE_CAPACITY_PER_WORKER: usize = 8;

pub(super) type TriggerRunner =
    dyn Fn(TriggerEvent) -> Result<RunReport, String> + Send + Sync + 'static;

pub(super) struct TriggerExecutor {
    completion_receiver: Receiver<TriggerCompletion>,
    next_job_id: u64,
    pending_jobs: usize,
    sender: Option<SyncSender<TriggerJob>>,
    cancellation: RuntimeCancellationToken,
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
    ) -> Result<Self, String> {
        let core = core.clone();
        let store = store.clone();
        let run_cancellation = cancellation.clone();
        let runner = Arc::new(move |event: TriggerEvent| {
            core.dispatch_trigger_event_with_cancellation(&store, event, run_cancellation.clone())
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
        )
    }

    pub(super) fn submit_from(
        &mut self,
        event: TriggerEvent,
        source: &'static str,
    ) -> Result<u64, TriggerSubmitError> {
        let job_id = self.next_job_id;
        self.next_job_id = self
            .next_job_id
            .checked_add(1)
            .ok_or(TriggerSubmitError::Stopped)?;
        let Some(sender) = &self.sender else {
            return Err(TriggerSubmitError::Stopped);
        };
        match sender.try_send(TriggerJob {
            event,
            job_id,
            source,
        }) {
            Ok(()) => {
                self.pending_jobs = self.pending_jobs.saturating_add(1);
                Ok(job_id)
            }
            Err(TrySendError::Full(_)) => Err(TriggerSubmitError::Full),
            Err(TrySendError::Disconnected(_)) => Err(TriggerSubmitError::Stopped),
        }
    }

    pub(super) fn try_completion(&mut self) -> Option<TriggerCompletion> {
        let completion = self.completion_receiver.try_recv().ok()?;
        self.pending_jobs = self.pending_jobs.saturating_sub(1);
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
        )
    }

    fn with_runner_and_cancellation(
        worker_count: usize,
        queue_capacity: usize,
        worker_label: &str,
        runner: Arc<TriggerRunner>,
        cancellation: RuntimeCancellationToken,
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
            completion_receiver,
            next_job_id: 1,
            pending_jobs: 0,
            sender: Some(sender),
            cancellation,
            workers,
        })
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
