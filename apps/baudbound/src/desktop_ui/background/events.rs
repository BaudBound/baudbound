use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Runtime};

use super::DesktopRunnerSnapshot;

pub(super) const DESKTOP_RUNNER_EVENT_CHANNEL: &str = "runner-desktop-background";
pub(super) const SERVICE_STATUS_EVENT_CHANNEL: &str = "runner-service-status";
const MAIN_WINDOW_LABEL: &str = "main";

pub(super) trait DesktopRunnerEventSink: Send + Sync {
    fn publish_service_status(&self, status: Value);
    fn publish_snapshot(&self, snapshot: DesktopRunnerSnapshot);
}

#[derive(Clone, Serialize)]
struct ServiceStatusEvent {
    service_health: Value,
    service_status: Value,
}

pub(super) struct TauriDesktopRunnerEventSink<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriDesktopRunnerEventSink<R> {
    pub(super) fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> DesktopRunnerEventSink for TauriDesktopRunnerEventSink<R> {
    fn publish_service_status(&self, status: Value) {
        let payload = service_status_event(status);
        let _ = self
            .app
            .emit_to(MAIN_WINDOW_LABEL, SERVICE_STATUS_EVENT_CHANNEL, payload);
    }

    fn publish_snapshot(&self, snapshot: DesktopRunnerSnapshot) {
        let _ = self
            .app
            .emit_to(MAIN_WINDOW_LABEL, DESKTOP_RUNNER_EVENT_CHANNEL, snapshot);
    }
}

fn service_status_event(mut service_status: Value) -> ServiceStatusEvent {
    let service_health =
        crate::commands::service_health::service_health_document(Some(&service_status));
    crate::service::redact_service_control(&mut service_status);
    ServiceStatusEvent {
        service_health,
        service_status,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn service_status_events_do_not_expose_the_control_token() {
        let payload = service_status_event(json!({
            "control": { "address": "127.0.0.1:1234", "token": "secret" },
            "last_heartbeat_unix": crate::paths::current_unix_timestamp(),
            "reload_interval_seconds": 2,
            "state": "running",
            "status_revision": 1
        }));

        assert_eq!(payload.service_health["health"], "healthy");
        assert_eq!(
            payload.service_status["control"]["address"],
            "127.0.0.1:1234"
        );
        assert!(payload.service_status["control"].get("token").is_none());
    }
}
