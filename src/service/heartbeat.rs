use std::time::{Duration, Instant};

use anyhow::Result;
use baudbound_core::{RunReport, TriggerEvent};
use baudbound_storage::SqliteRunnerStore;
use serde_json::Value;

use super::{
    ServiceStatusNotifier,
    activity::ServiceActivity,
    ipc::ServiceControlDescriptor,
    options::ServeOptions,
    status::{ServeStatusSnapshot, build_serve_status_document, write_serve_status_document},
    triggers::TriggerServices,
};
use crate::paths::current_unix_timestamp;

pub(super) const SERVICE_STATUS_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

pub(super) struct ServeStatusTracker {
    activity: ServiceActivity,
    last_reload_at_unix: u64,
    last_status_fingerprint: Option<Value>,
    last_status_write: Instant,
    status_change_notifier: Option<ServiceStatusNotifier>,
    status_revision: u64,
    service_control: ServiceControlDescriptor,
    started_at_unix: u64,
}

impl ServeStatusTracker {
    pub(super) fn start(
        service_control: ServiceControlDescriptor,
        status_change_notifier: Option<ServiceStatusNotifier>,
        status_revision: u64,
    ) -> Self {
        let started_at_unix = current_unix_timestamp();
        Self {
            activity: ServiceActivity::default(),
            last_reload_at_unix: started_at_unix,
            last_status_fingerprint: None,
            last_status_write: Instant::now(),
            status_change_notifier,
            status_revision,
            service_control,
            started_at_unix,
        }
    }

    pub(super) fn write_running(
        &mut self,
        store: &SqliteRunnerStore,
        options: &ServeOptions,
        services: &TriggerServices,
    ) -> Result<()> {
        let next_revision = self.status_revision.saturating_add(1);
        let document = self.build_document(store, options, services, next_revision, "running");
        self.persist_running(store, document, next_revision)
    }

    fn build_document(
        &self,
        store: &SqliteRunnerStore,
        options: &ServeOptions,
        services: &TriggerServices,
        status_revision: u64,
        state: &str,
    ) -> Value {
        build_serve_status_document(
            store,
            options,
            services,
            ServeStatusSnapshot {
                activity: &self.activity,
                last_reload_at_unix: self.last_reload_at_unix,
                service_control: &self.service_control,
                status_revision,
                started_at_unix: self.started_at_unix,
                state,
            },
        )
    }

    fn persist_running(
        &mut self,
        store: &SqliteRunnerStore,
        document: Value,
        status_revision: u64,
    ) -> Result<()> {
        write_serve_status_document(store, &document)?;
        self.last_status_fingerprint = Some(status_fingerprint(&document));
        self.last_status_write = Instant::now();
        self.status_revision = status_revision;
        self.notify_status_changed(&document);
        Ok(())
    }

    pub(super) fn prepare_stopped(
        &mut self,
        store: &SqliteRunnerStore,
        options: &ServeOptions,
        services: &TriggerServices,
    ) -> Value {
        self.status_revision = self.status_revision.saturating_add(1);
        build_serve_status_document(
            store,
            options,
            services,
            ServeStatusSnapshot {
                activity: &self.activity,
                last_reload_at_unix: self.last_reload_at_unix,
                service_control: &self.service_control,
                status_revision: self.status_revision,
                started_at_unix: self.started_at_unix,
                state: "stopped",
            },
        )
    }

    pub(super) fn write_prepared_stopped(
        &self,
        store: &SqliteRunnerStore,
        document: &Value,
    ) -> Result<()> {
        write_serve_status_document(store, document)?;
        self.notify_status_changed(document);
        Ok(())
    }

    pub(super) fn mark_reloaded(&mut self) {
        self.last_reload_at_unix = current_unix_timestamp();
    }

    pub(super) fn write_if_changed_or_due(
        &mut self,
        store: &SqliteRunnerStore,
        options: &ServeOptions,
        services: &TriggerServices,
    ) -> Result<()> {
        let next_revision = self.status_revision.saturating_add(1);
        let document = self.build_document(store, options, services, next_revision, "running");
        let fingerprint = status_fingerprint(&document);
        let status_changed = self.last_status_fingerprint.as_ref() != Some(&fingerprint);
        if status_changed || self.last_status_write.elapsed() >= SERVICE_STATUS_HEARTBEAT_INTERVAL {
            self.persist_running(store, document, next_revision)?;
        }
        Ok(())
    }

    pub(super) fn time_until_next_heartbeat(&self) -> Duration {
        SERVICE_STATUS_HEARTBEAT_INTERVAL.saturating_sub(self.last_status_write.elapsed())
    }

    pub(super) fn record_event_failure(
        &mut self,
        source: &'static str,
        event: &TriggerEvent,
        error: impl Into<String>,
    ) {
        self.activity.record_event_failure(source, event, error);
    }

    pub(super) fn record_report(&mut self, source: &'static str, report: &RunReport) {
        self.activity.record_report(source, report);
    }

    fn notify_status_changed(&self, document: &Value) {
        if let Some(notifier) = &self.status_change_notifier {
            notifier(document);
        }
    }
}

fn status_fingerprint(document: &Value) -> Value {
    let mut fingerprint = document.clone();
    if let Some(object) = fingerprint.as_object_mut() {
        object.remove("last_heartbeat_unix");
        object.remove("status_revision");
    }
    fingerprint
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::status_fingerprint;

    #[test]
    fn fingerprint_ignores_heartbeat_metadata_but_detects_service_changes() {
        let first = json!({
            "last_heartbeat_unix": 10,
            "services": [{ "details": { "state": "starting" } }],
            "status_revision": 4
        });
        let heartbeat = json!({
            "last_heartbeat_unix": 15,
            "services": [{ "details": { "state": "starting" } }],
            "status_revision": 5
        });
        let changed = json!({
            "last_heartbeat_unix": 15,
            "services": [{ "details": { "state": "reading" } }],
            "status_revision": 5
        });

        assert_eq!(status_fingerprint(&first), status_fingerprint(&heartbeat));
        assert_ne!(status_fingerprint(&first), status_fingerprint(&changed));
    }
}
