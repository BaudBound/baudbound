use baudbound_runtime::RuntimeLogEntry;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

use super::ActiveRunSnapshot;

pub(super) const ACTIVE_RUN_EVENT_CHANNEL: &str = "runner-active-run";
const MAIN_WINDOW_LABEL: &str = "main";

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum ActiveRunEvent {
    Started {
        revision: u64,
        run: ActiveRunSnapshot,
    },
    LogEmitted {
        discarded_log_count: usize,
        log: RuntimeLogEntry,
        revision: u64,
        run_id: String,
    },
    CancellationRequested {
        revision: u64,
        run_id: String,
    },
    Finished {
        revision: u64,
        run_id: String,
    },
    RunRecorded {
        revision: u64,
    },
}

pub(super) trait ActiveRunEventSink: Send + Sync {
    fn publish(&self, event: ActiveRunEvent);
}

pub(super) struct TauriActiveRunEventSink<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriActiveRunEventSink<R> {
    pub(super) fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> ActiveRunEventSink for TauriActiveRunEventSink<R> {
    fn publish(&self, event: ActiveRunEvent) {
        let _ = self
            .app
            .emit_to(MAIN_WINDOW_LABEL, ACTIVE_RUN_EVENT_CHANNEL, event);
    }
}
