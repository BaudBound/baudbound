use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError, SyncSender},
    },
    time::{Duration, SystemTime},
};

use crate::{TriggerEvent, try_send_trigger_event};

use super::{
    engine::ProcessStartedEngine,
    snapshot::ProcessSource,
    spec::{ProcessStartedSpec, RegistrationId},
};

pub(super) enum ProcessWatcherCommand {
    Reconfigure {
        acknowledge: SyncSender<()>,
        specs: BTreeMap<RegistrationId, ProcessStartedSpec>,
    },
    Stop,
}

pub(super) struct ProcessWatcherOptions {
    pub(super) poll_interval: Duration,
}

pub(super) fn run_process_watcher(
    initial_specs: BTreeMap<RegistrationId, ProcessStartedSpec>,
    event_sender: SyncSender<TriggerEvent>,
    command_receiver: Receiver<ProcessWatcherCommand>,
    ready_sender: SyncSender<()>,
    running: Arc<AtomicBool>,
    options: ProcessWatcherOptions,
) {
    let _running_guard = RunningGuard(Arc::clone(&running));
    let mut source = ProcessSource::new();
    let baseline = source.refresh();
    let mut engine = ProcessStartedEngine::new(initial_specs, &baseline);
    running.store(true, Ordering::Release);
    if ready_sender.send(()).is_err() {
        return;
    }

    loop {
        match command_receiver.recv_timeout(options.poll_interval) {
            Ok(ProcessWatcherCommand::Reconfigure { acknowledge, specs }) => {
                let processes = source.refresh();
                send_events(
                    &event_sender,
                    engine.reconfigure_and_poll(
                        specs,
                        &processes,
                        SystemTime::now(),
                        matching_window_title,
                    ),
                );
                if acknowledge.send(()).is_err() {
                    break;
                }
            }
            Ok(ProcessWatcherCommand::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                let processes = source.refresh();
                send_events(
                    &event_sender,
                    engine.poll(&processes, SystemTime::now(), matching_window_title),
                );
            }
        }
    }
}

fn send_events(sender: &SyncSender<TriggerEvent>, events: Vec<TriggerEvent>) {
    for event in events {
        try_send_trigger_event(sender, event, "process started");
    }
}

struct RunningGuard(Arc<AtomicBool>);

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(windows)]
fn matching_window_title(process_id: u32, target: &str) -> Option<String> {
    super::windows::matching_window_title(process_id, target)
}

#[cfg(not(windows))]
fn matching_window_title(_process_id: u32, _target: &str) -> Option<String> {
    None
}
