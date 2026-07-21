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

use crate::service::{ServeOptions, ServeRuntimeControl, serve_triggers_with_control};

#[derive(Clone, Default)]
pub struct DesktopRunnerSupervisor {
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
    pub state: DesktopRunnerStateLabel,
    pub message: String,
    pub running: bool,
    pub started_at_unix: Option<u64>,
    pub stopped_at_unix: Option<u64>,
}

#[derive(Clone, Copy, Serialize)]
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
            state: DesktopRunnerStateLabel::Stopped,
            message: "Desktop background runner is stopped.".to_owned(),
            running: false,
            started_at_unix: None,
            stopped_at_unix: None,
        }
    }
}

impl DesktopRunnerSupervisor {
    pub fn start(
        &self,
        core: RunnerCore,
        store: SqliteRunnerStore,
        options: ServeOptions,
    ) -> Result<String> {
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown_requested);
        let inner = Arc::clone(&self.inner);

        {
            let mut state = self.lock_state()?;
            self.join_finished_thread(&mut state);
            if state.handle.is_some() {
                return Ok("Desktop background runner is already running.".to_owned());
            }
            state.shutdown_requested = Some(Arc::clone(&shutdown_requested));
            state.snapshot = DesktopRunnerSnapshot {
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
                let result = serve_triggers_with_control(
                    &core,
                    &store,
                    options,
                    ServeRuntimeControl::desktop(thread_shutdown),
                );
                let mut state = match inner.lock() {
                    Ok(state) => state,
                    Err(_) => return,
                };
                state.shutdown_requested = None;
                state.snapshot = match result {
                    Ok(()) => DesktopRunnerSnapshot {
                        state: DesktopRunnerStateLabel::Stopped,
                        message: "Desktop background runner stopped.".to_owned(),
                        running: false,
                        started_at_unix: state.snapshot.started_at_unix,
                        stopped_at_unix: Some(now_unix()),
                    },
                    Err(error) => DesktopRunnerSnapshot {
                        state: DesktopRunnerStateLabel::Failed,
                        message: format!("Desktop background runner failed: {error}"),
                        running: false,
                        started_at_unix: state.snapshot.started_at_unix,
                        stopped_at_unix: Some(now_unix()),
                    },
                };
            })
            .map_err(|source| anyhow!("failed to spawn desktop background runner: {source}"))?;

        let mut state = self.lock_state()?;
        state.handle = Some(handle);
        Ok("Started desktop background runner.".to_owned())
    }

    pub fn stop(&self) -> Result<String> {
        let mut state = self.lock_state()?;
        self.join_finished_thread(&mut state);
        if !request_stop(&mut state) {
            return Ok("Desktop background runner is already stopped.".to_owned());
        }
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
    state.snapshot.state = DesktopRunnerStateLabel::Stopping;
    state.snapshot.message = "Desktop background runner is stopping.".to_owned();
    state.snapshot.running = true;
    true
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
