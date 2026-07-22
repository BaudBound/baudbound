use std::time::SystemTime;

use serde_json::json;

use crate::{
    TriggerError, TriggerEvent, TriggerRegistration, TriggerServiceDiagnostics,
    unix_timestamp_millis,
};

#[derive(Debug, Clone)]
pub struct StartupService {
    events: Vec<TriggerEvent>,
}

impl StartupService {
    #[must_use]
    pub fn empty() -> Self {
        Self { events: Vec::new() }
    }

    pub fn from_registrations(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        startup_time: SystemTime,
    ) -> Result<Self, TriggerError> {
        let events = registrations
            .into_iter()
            .filter(|registration| registration.action_type == "trigger.startup")
            .map(|registration| TriggerEvent {
                action_type: registration.action_type,
                node_id: registration.node_id,
                payload: json!({
                    "reason": "runner_startup",
                    "timestamp": unix_timestamp_millis(startup_time).to_string(),
                }),
                script_id: registration.script_id,
            })
            .collect();

        Ok(Self { events })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::active(self.len(), "startup trigger")
    }

    pub fn drain_events(&mut self) -> Vec<TriggerEvent> {
        std::mem::take(&mut self.events)
    }
}
