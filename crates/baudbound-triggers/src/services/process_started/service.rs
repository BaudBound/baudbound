use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{TriggerError, TriggerEvent, TriggerRegistration, TriggerServiceDiagnostics};

use super::{
    spec::collect_specs,
    worker::{ProcessWatcherCommand, ProcessWatcherOptions, run_process_watcher},
};

const PROCESS_POLL_INTERVAL: Duration = Duration::from_secs(1);

pub struct ProcessStartedService {
    command_sender: Option<SyncSender<ProcessWatcherCommand>>,
    handle: Option<JoinHandle<()>>,
    registration_count: usize,
    running: Arc<AtomicBool>,
}

impl ProcessStartedService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            command_sender: None,
            handle: None,
            registration_count: 0,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        event_sender: SyncSender<TriggerEvent>,
    ) -> Result<Self, TriggerError> {
        let specs = collect_specs(registrations)?;
        if specs.is_empty() {
            return Ok(Self::empty());
        }
        Self::spawn(specs, event_sender, PROCESS_POLL_INTERVAL)
    }

    pub fn start_or_reconfigure(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        event_sender: SyncSender<TriggerEvent>,
        previous: Option<Self>,
    ) -> Result<Self, TriggerError> {
        let specs = collect_specs(registrations)?;
        if specs.is_empty() {
            drop(previous);
            return Ok(Self::empty());
        }
        if let Some(mut service) = previous.filter(|service| !service.is_empty()) {
            service.reconfigure(specs)?;
            return Ok(service);
        }
        Self::spawn(specs, event_sender, PROCESS_POLL_INTERVAL)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.registration_count == 0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.registration_count
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::thread_backed(
            self.running.load(Ordering::Acquire),
            self.len(),
            "process watcher",
        )
    }

    #[cfg(test)]
    pub(super) fn start_with_interval(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        event_sender: SyncSender<TriggerEvent>,
        poll_interval: Duration,
    ) -> Result<Self, TriggerError> {
        let specs = collect_specs(registrations)?;
        if specs.is_empty() {
            return Ok(Self::empty());
        }
        Self::spawn(specs, event_sender, poll_interval)
    }

    fn spawn(
        specs: std::collections::BTreeMap<
            super::spec::RegistrationId,
            super::spec::ProcessStartedSpec,
        >,
        event_sender: SyncSender<TriggerEvent>,
        poll_interval: Duration,
    ) -> Result<Self, TriggerError> {
        let registration_count = specs.len();
        let (command_sender, command_receiver) = mpsc::sync_channel(1);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let running = Arc::new(AtomicBool::new(false));
        let thread_running = Arc::clone(&running);
        let handle = thread::Builder::new()
            .name("baudbound-process-started".to_owned())
            .spawn(move || {
                run_process_watcher(
                    specs,
                    event_sender,
                    command_receiver,
                    ready_sender,
                    thread_running,
                    ProcessWatcherOptions { poll_interval },
                );
            })
            .map_err(|source| {
                TriggerError::Failed(
                    "process_started".to_owned(),
                    format!("failed to spawn process watcher: {source}"),
                )
            })?;
        if ready_receiver.recv().is_err() {
            let _ = handle.join();
            return Err(TriggerError::Failed(
                "process_started".to_owned(),
                "process watcher stopped during initialization".to_owned(),
            ));
        }
        Ok(Self {
            command_sender: Some(command_sender),
            handle: Some(handle),
            registration_count,
            running,
        })
    }

    fn reconfigure(
        &mut self,
        specs: std::collections::BTreeMap<
            super::spec::RegistrationId,
            super::spec::ProcessStartedSpec,
        >,
    ) -> Result<(), TriggerError> {
        let registration_count = specs.len();
        let (acknowledge, acknowledgement) = mpsc::sync_channel(1);
        self.command_sender
            .as_ref()
            .ok_or_else(worker_stopped)?
            .send(ProcessWatcherCommand::Reconfigure { acknowledge, specs })
            .map_err(|_| worker_stopped())?;
        acknowledgement.recv().map_err(|_| worker_stopped())?;
        self.registration_count = registration_count;
        Ok(())
    }
}

impl Drop for ProcessStartedService {
    fn drop(&mut self) {
        if let Some(sender) = self.command_sender.take() {
            let _ = sender.send(ProcessWatcherCommand::Stop);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn worker_stopped() -> TriggerError {
    TriggerError::Failed(
        "process_started".to_owned(),
        "process watcher stopped unexpectedly".to_owned(),
    )
}
