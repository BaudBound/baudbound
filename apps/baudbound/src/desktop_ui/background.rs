use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, anyhow};
use baudbound_core::RunnerCore;
use baudbound_storage::SqliteRunnerStore;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Runtime};

use crate::service::{
    ServeOptions, ServeRuntimeControl, ServiceStatusNotifier, serve_triggers_with_control,
};

mod events;

use events::{DesktopRunnerEventSink, TauriDesktopRunnerEventSink};

#[derive(Clone, Default)]
pub struct DesktopRunnerSupervisor {
    event_sink: Arc<Mutex<Option<Arc<dyn DesktopRunnerEventSink>>>>,
    inner: Arc<Mutex<DesktopRunnerState>>,
}

#[derive(Default)]
struct DesktopRunnerState {
    handle: Option<JoinHandle<()>>,
    shutdown_requested: Option<Arc<AtomicBool>>,
    snapshot: DesktopRunnerSnapshot,
}

#[derive(Clone, Serialize)]
pub struct DesktopRunnerSnapshot {
    pub revision: u64,
    pub state: DesktopRunnerStateLabel,
    pub message: String,
    pub running: bool,
    pub started_at_unix: Option<u64>,
    pub stopped_at_unix: Option<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopRunnerStateLabel {
    Failed,
    Running,
    Stopped,
    Stopping,
}

impl Default for DesktopRunnerSnapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            state: DesktopRunnerStateLabel::Stopped,
            message: "Desktop background runner is stopped.".to_owned(),
            running: false,
            started_at_unix: None,
            stopped_at_unix: None,
        }
    }
}

impl DesktopRunnerSupervisor {
    pub fn connect_event_sink<R: Runtime>(&self, app: AppHandle<R>) {
        let sink: Arc<dyn DesktopRunnerEventSink> = Arc::new(TauriDesktopRunnerEventSink::new(app));
        match self.event_sink.lock() {
            Ok(mut event_sink) => *event_sink = Some(sink),
            Err(_) => tracing::warn!("desktop runner event sink lock is poisoned"),
        }
    }

    pub fn start(
        &self,
        core: RunnerCore,
        store: SqliteRunnerStore,
        options: ServeOptions,
    ) -> Result<String> {
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown_requested);
        let inner = Arc::clone(&self.inner);
        let event_sink = Arc::clone(&self.event_sink);
        let status_event_sink = Arc::clone(&self.event_sink);

        {
            let mut state = self.lock_state()?;
            self.join_finished_thread(&mut state);
            if state.handle.is_some() {
                return Ok("Desktop background runner is already running.".to_owned());
            }
            state.shutdown_requested = Some(Arc::clone(&shutdown_requested));
            state.snapshot = DesktopRunnerSnapshot {
                revision: state.snapshot.revision.saturating_add(1),
                state: DesktopRunnerStateLabel::Running,
                message: "Desktop background runner is running.".to_owned(),
                running: true,
                started_at_unix: Some(now_unix()),
                stopped_at_unix: None,
            };
        }

        let handle = thread::Builder::new()
            .name("baudbound-desktop-runner".to_owned())
            .spawn(move || {
                let status_change_notifier = service_status_notifier(status_event_sink);
                let result = serve_triggers_with_control(
                    &core,
                    &store,
                    options,
                    ServeRuntimeControl::desktop(thread_shutdown)
                        .with_status_change_notifier(status_change_notifier),
                );
                let mut state = match inner.lock() {
                    Ok(state) => state,
                    Err(_) => return,
                };
                state.shutdown_requested = None;
                state.snapshot = match result {
                    Ok(()) => DesktopRunnerSnapshot {
                        revision: state.snapshot.revision.saturating_add(1),
                        state: DesktopRunnerStateLabel::Stopped,
                        message: "Desktop background runner stopped.".to_owned(),
                        running: false,
                        started_at_unix: state.snapshot.started_at_unix,
                        stopped_at_unix: Some(now_unix()),
                    },
                    Err(error) => DesktopRunnerSnapshot {
                        revision: state.snapshot.revision.saturating_add(1),
                        state: DesktopRunnerStateLabel::Failed,
                        message: format!("Desktop background runner failed: {error}"),
                        running: false,
                        started_at_unix: state.snapshot.started_at_unix,
                        stopped_at_unix: Some(now_unix()),
                    },
                };
                let snapshot = state.snapshot.clone();
                drop(state);
                publish_snapshot(&event_sink, snapshot);
            })
            .map_err(|source| anyhow!("failed to spawn desktop background runner: {source}"))?;

        let mut state = self.lock_state()?;
        state.handle = Some(handle);
        let snapshot = state.snapshot.clone();
        drop(state);
        self.publish_snapshot(snapshot);
        Ok("Started desktop background runner.".to_owned())
    }

    pub fn stop(&self) -> Result<String> {
        let mut state = self.lock_state()?;
        self.join_finished_thread(&mut state);
        if !request_stop(&mut state) {
            return Ok("Desktop background runner is already stopped.".to_owned());
        }
        let snapshot = state.snapshot.clone();
        drop(state);
        self.publish_snapshot(snapshot);
        Ok("Requested desktop background runner stop.".to_owned())
    }

    pub fn stop_and_wait(&self, timeout: Duration) -> Result<String> {
        let started_at = Instant::now();

        loop {
            {
                let mut state = self.lock_state()?;
                self.join_finished_thread(&mut state);
                if state.handle.is_none() {
                    return Ok("Desktop background runner is stopped.".to_owned());
                }
                request_stop(&mut state);
            }

            if started_at.elapsed() >= timeout {
                return Err(anyhow!(
                    "timed out after {} milliseconds while waiting for the desktop background runner to stop",
                    timeout.as_millis()
                ));
            }

            thread::sleep(Duration::from_millis(25));
        }
    }

    pub fn snapshot(&self) -> Result<DesktopRunnerSnapshot> {
        let mut state = self.lock_state()?;
        self.join_finished_thread(&mut state);
        Ok(state.snapshot.clone())
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, DesktopRunnerState>> {
        self.inner
            .lock()
            .map_err(|_| anyhow!("desktop background runner state lock is poisoned"))
    }

    fn publish_snapshot(&self, snapshot: DesktopRunnerSnapshot) {
        publish_snapshot(&self.event_sink, snapshot);
    }

    fn join_finished_thread(&self, state: &mut DesktopRunnerState) {
        if state.handle.as_ref().is_some_and(JoinHandle::is_finished)
            && let Some(handle) = state.handle.take()
        {
            let _ = handle.join();
        }
    }
}

fn request_stop(state: &mut DesktopRunnerState) -> bool {
    let Some(shutdown_requested) = &state.shutdown_requested else {
        return false;
    };
    shutdown_requested.store(true, Ordering::SeqCst);
    if state.snapshot.state != DesktopRunnerStateLabel::Stopping {
        state.snapshot.revision = state.snapshot.revision.saturating_add(1);
        state.snapshot.state = DesktopRunnerStateLabel::Stopping;
        state.snapshot.message = "Desktop background runner is stopping.".to_owned();
        state.snapshot.running = true;
    }
    true
}

fn publish_snapshot(
    event_sink: &Arc<Mutex<Option<Arc<dyn DesktopRunnerEventSink>>>>,
    snapshot: DesktopRunnerSnapshot,
) {
    let sink = match event_sink.lock() {
        Ok(event_sink) => event_sink.clone(),
        Err(_) => {
            tracing::warn!("desktop runner event sink lock is poisoned");
            None
        }
    };
    if let Some(sink) = sink {
        sink.publish_snapshot(snapshot);
    }
}

fn publish_service_status(
    event_sink: &Arc<Mutex<Option<Arc<dyn DesktopRunnerEventSink>>>>,
    status: Value,
) {
    let sink = match event_sink.lock() {
        Ok(event_sink) => event_sink.clone(),
        Err(_) => {
            tracing::warn!("desktop runner event sink lock is poisoned");
            None
        }
    };
    if let Some(sink) = sink {
        sink.publish_service_status(status);
    }
}

fn service_status_notifier(
    event_sink: Arc<Mutex<Option<Arc<dyn DesktopRunnerEventSink>>>>,
) -> ServiceStatusNotifier {
    Arc::new(move |status| publish_service_status(&event_sink, status.clone()))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingEventSink {
        service_statuses: Mutex<Vec<Value>>,
        snapshots: Mutex<Vec<DesktopRunnerSnapshot>>,
    }

    impl DesktopRunnerEventSink for RecordingEventSink {
        fn publish_service_status(&self, status: Value) {
            self.service_statuses
                .lock()
                .expect("recorded service statuses should be available")
                .push(status);
        }

        fn publish_snapshot(&self, snapshot: DesktopRunnerSnapshot) {
            self.snapshots
                .lock()
                .expect("recorded snapshots should be available")
                .push(snapshot);
        }
    }

    #[test]
    fn service_status_notifier_publishes_the_committed_status() {
        let sink = Arc::new(RecordingEventSink::default());
        let event_sink: Arc<Mutex<Option<Arc<dyn DesktopRunnerEventSink>>>> =
            Arc::new(Mutex::new(Some(sink.clone())));
        let notifier = service_status_notifier(event_sink);

        notifier(&serde_json::json!({ "state": "running", "status_revision": 4 }));

        let statuses = sink
            .service_statuses
            .lock()
            .expect("recorded service statuses should be available");
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0]["status_revision"], 4);
    }

    #[test]
    fn stop_publishes_a_revisioned_stopping_snapshot() {
        let supervisor = test_supervisor(Duration::from_millis(10));
        let sink = Arc::new(RecordingEventSink::default());
        *supervisor
            .event_sink
            .lock()
            .expect("event sink should be available") = Some(sink.clone());

        supervisor.stop().expect("stop request should succeed");

        let snapshots = sink
            .snapshots
            .lock()
            .expect("recorded snapshots should be available");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].revision, 2);
        assert!(snapshots[0].state == DesktopRunnerStateLabel::Stopping);
        drop(snapshots);
        supervisor
            .stop_and_wait(Duration::from_secs(1))
            .expect("the test worker should stop");
    }

    #[test]
    fn stop_and_wait_reports_timeout_instead_of_claiming_success() {
        let supervisor = test_supervisor(Duration::from_millis(100));

        let error = supervisor
            .stop_and_wait(Duration::from_millis(10))
            .expect_err("a live worker must not be reported as stopped after a timeout");

        assert!(error.to_string().contains("timed out"));
        supervisor
            .stop_and_wait(Duration::from_secs(1))
            .expect("the test worker should finish after receiving the stop request");
    }

    fn test_supervisor(shutdown_delay: Duration) -> DesktopRunnerSupervisor {
        let supervisor = DesktopRunnerSupervisor::default();
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown_requested);
        let inner = Arc::clone(&supervisor.inner);
        let handle = thread::spawn(move || {
            while !thread_shutdown.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(1));
            }
            thread::sleep(shutdown_delay);
            if let Ok(mut state) = inner.lock() {
                state.shutdown_requested = None;
                state.snapshot = DesktopRunnerSnapshot::default();
            }
        });

        let mut state = supervisor
            .inner
            .lock()
            .expect("test supervisor state should be available");
        state.handle = Some(handle);
        state.shutdown_requested = Some(shutdown_requested);
        state.snapshot = DesktopRunnerSnapshot {
            revision: 1,
            state: DesktopRunnerStateLabel::Running,
            message: "Test runner is running.".to_owned(),
            running: true,
            started_at_unix: Some(now_unix()),
            stopped_at_unix: None,
        };
        drop(state);
        supervisor
    }
}
