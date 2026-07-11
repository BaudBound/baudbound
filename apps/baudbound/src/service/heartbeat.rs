use std::time::{Duration, Instant};

use anyhow::Result;
use baudbound_core::{RunReport, TriggerEvent};
use baudbound_storage::SqliteRunnerStore;

use super::{
    activity::ServiceActivity,
    ipc::ServiceControlDescriptor,
    options::ServeOptions,
    status::{ServeStatusSnapshot, write_serve_status},
    triggers::TriggerServices,
};
use crate::paths::current_unix_timestamp;

pub(super) const SERVICE_STATUS_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

pub(super) struct ServeStatusTracker {
    activity: ServiceActivity,
    last_reload_at_unix: u64,
    last_status_write: Instant,
    service_control: ServiceControlDescriptor,
    started_at_unix: u64,
}

impl ServeStatusTracker {
    pub(super) fn start(service_control: ServiceControlDescriptor) -> Self {
        let started_at_unix = current_unix_timestamp();
        Self {
            activity: ServiceActivity::default(),
            last_reload_at_unix: started_at_unix,
            last_status_write: Instant::now(),
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
        write_serve_status(
            store,
            options,
            services,
            ServeStatusSnapshot {
                activity: &self.activity,
                last_reload_at_unix: self.last_reload_at_unix,
                service_control: &self.service_control,
                started_at_unix: self.started_at_unix,
                state: "running",
            },
        )?;
        self.last_status_write = Instant::now();
        Ok(())
    }

    pub(super) fn write_stopped(
        &self,
        store: &SqliteRunnerStore,
        options: &ServeOptions,
        services: &TriggerServices,
    ) -> Result<()> {
        write_serve_status(
            store,
            options,
            services,
            ServeStatusSnapshot {
                activity: &self.activity,
                last_reload_at_unix: self.last_reload_at_unix,
                service_control: &self.service_control,
                started_at_unix: self.started_at_unix,
                state: "stopped",
            },
        )
    }

    pub(super) fn mark_reloaded(&mut self) {
        self.last_reload_at_unix = current_unix_timestamp();
    }

    pub(super) fn write_heartbeat_if_due(
        &mut self,
        store: &SqliteRunnerStore,
        options: &ServeOptions,
        services: &TriggerServices,
    ) -> Result<()> {
        if self.last_status_write.elapsed() >= SERVICE_STATUS_HEARTBEAT_INTERVAL {
            self.write_running(store, options, services)?;
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
}
