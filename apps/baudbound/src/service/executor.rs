use std::{
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
    },
    thread,
};

use baudbound_core::{RunReport, RunnerCore, TriggerEvent};
use baudbound_storage::SqliteRunnerStore;

const MAX_WORKERS: usize = 16;
const MIN_WORKERS: usize = 2;
const QUEUE_CAPACITY_PER_WORKER: usize = 8;

pub(super) type TriggerRunner =
    dyn Fn(TriggerEvent) -> Result<RunReport, String> + Send + Sync + 'static;

pub(super) struct TriggerExecutor {
    completion_receiver: Receiver<TriggerCompletion>,
    next_job_id: u64,
    sender: Option<SyncSender<TriggerJob>>,
}

pub(super) struct TriggerCompletion {
    pub(super) event: TriggerEvent,
    pub(super) job_id: u64,
    pub(super) result: Result<RunReport, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TriggerSubmitError {
    Full,
    Stopped,
}

struct TriggerJob {
    event: TriggerEvent,
    job_id: u64,
}

impl TriggerExecutor {
    pub(super) fn new(
        core: &RunnerCore,
        store: &SqliteRunnerStore,
        worker_label: &str,
    ) -> Result<Self, String> {
        let core = core.clone();
        let store = store.clone();
        let runner = Arc::new(move |event: TriggerEvent| {
            core.dispatch_trigger_event(&store, event)
                .map_err(|error| error.to_string())
        });
        let worker_count = thread::available_parallelism()
            .map_or(MIN_WORKERS, usize::from)
            .clamp(MIN_WORKERS, MAX_WORKERS);
        Self::with_runner(
            worker_count,
            worker_count.saturating_mul(QUEUE_CAPACITY_PER_WORKER),
            worker_label,
            runner,
        )
    }

    pub(super) fn submit(&mut self, event: TriggerEvent) -> Result<u64, TriggerSubmitError> {
        let job_id = self.next_job_id;
        self.next_job_id = self
            .next_job_id
            .checked_add(1)
            .ok_or(TriggerSubmitError::Stopped)?;
        let Some(sender) = &self.sender else {
            return Err(TriggerSubmitError::Stopped);
        };
        match sender.try_send(TriggerJob { event, job_id }) {
            Ok(()) => Ok(job_id),
            Err(TrySendError::Full(_)) => Err(TriggerSubmitError::Full),
            Err(TrySendError::Disconnected(_)) => Err(TriggerSubmitError::Stopped),
        }
    }

    pub(super) fn try_completion(&self) -> Option<TriggerCompletion> {
        self.completion_receiver.try_recv().ok()
    }

    pub(super) fn with_runner(
        worker_count: usize,
        queue_capacity: usize,
        worker_label: &str,
        runner: Arc<TriggerRunner>,
    ) -> Result<Self, String> {
        let (sender, receiver) = sync_channel::<TriggerJob>(queue_capacity.max(1));
        let receiver = Arc::new(Mutex::new(receiver));
        let (completion_sender, completion_receiver) = sync_channel(queue_capacity.max(1));

        for worker_index in 0..worker_count.max(1) {
            let receiver = Arc::clone(&receiver);
            let completion_sender = completion_sender.clone();
            let runner = Arc::clone(&runner);
            thread::Builder::new()
                .name(format!("baudbound-{worker_label}-{worker_index}"))
                .spawn(move || worker_loop(&receiver, &completion_sender, &runner))
                .map_err(|source| {
                    format!(
                        "failed to spawn {worker_label} execution worker {worker_index}: {source}"
                    )
                })?;
        }

        Ok(Self {
            completion_receiver,
            next_job_id: 1,
            sender: Some(sender),
        })
    }
}

impl Drop for TriggerExecutor {
    fn drop(&mut self) {
        self.sender.take();
    }
}

fn worker_loop(
    receiver: &Mutex<Receiver<TriggerJob>>,
    completion_sender: &SyncSender<TriggerCompletion>,
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
