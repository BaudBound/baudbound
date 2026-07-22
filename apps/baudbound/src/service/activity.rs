use std::collections::BTreeMap;

use baudbound_core::{RunReport, TriggerEvent};
use serde::Serialize;

use crate::paths::current_unix_timestamp;

#[derive(Clone, Debug, Default, Serialize)]
pub(super) struct ServiceActivity {
    pub(super) failed_dispatch_count: u64,
    pub(super) last_dispatch: Option<DispatchActivity>,
    pub(super) total_dispatch_count: u64,
    pub(super) triggers: BTreeMap<String, TriggerDispatchActivity>,
}

impl ServiceActivity {
    pub(super) fn record_report(&mut self, source: &'static str, report: &RunReport) {
        self.total_dispatch_count = self.total_dispatch_count.saturating_add(1);
        let dispatch = DispatchActivity {
            completed_at_unix: current_unix_timestamp(),
            error: None,
            node_id: report.identity.trigger_node_id.clone(),
            run_id: Some(report.identity.run_id.clone()),
            script_id: report.identity.script_id.clone(),
            source,
            status: "completed",
        };
        self.record_trigger_dispatch(&dispatch);
        self.last_dispatch = Some(dispatch);
    }

    pub(super) fn record_event_failure(
        &mut self,
        source: &'static str,
        event: &TriggerEvent,
        error: impl Into<String>,
    ) {
        self.total_dispatch_count = self.total_dispatch_count.saturating_add(1);
        self.failed_dispatch_count = self.failed_dispatch_count.saturating_add(1);
        let dispatch = DispatchActivity {
            completed_at_unix: current_unix_timestamp(),
            error: Some(error.into()),
            node_id: event.node_id.clone(),
            run_id: None,
            script_id: event.script_id.clone(),
            source,
            status: "failed",
        };
        self.record_trigger_dispatch(&dispatch);
        self.last_dispatch = Some(dispatch);
    }

    fn record_trigger_dispatch(&mut self, dispatch: &DispatchActivity) {
        let key = trigger_key(&dispatch.script_id, &dispatch.node_id);
        self.triggers
            .entry(key)
            .or_default()
            .record_dispatch(dispatch);
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct DispatchActivity {
    completed_at_unix: u64,
    error: Option<String>,
    node_id: String,
    run_id: Option<String>,
    script_id: String,
    source: &'static str,
    status: &'static str,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(super) struct TriggerDispatchActivity {
    failed_dispatch_count: u64,
    last_dispatch: Option<DispatchActivity>,
    last_failure_unix: Option<u64>,
    last_success_unix: Option<u64>,
    successful_dispatch_count: u64,
    total_dispatch_count: u64,
}

impl TriggerDispatchActivity {
    fn record_dispatch(&mut self, dispatch: &DispatchActivity) {
        self.total_dispatch_count = self.total_dispatch_count.saturating_add(1);
        self.last_dispatch = Some(dispatch.clone());
        if dispatch.status == "completed" {
            self.successful_dispatch_count = self.successful_dispatch_count.saturating_add(1);
            self.last_success_unix = Some(dispatch.completed_at_unix);
        } else {
            self.failed_dispatch_count = self.failed_dispatch_count.saturating_add(1);
            self.last_failure_unix = Some(dispatch.completed_at_unix);
        }
    }
}

fn trigger_key(script_id: &str, node_id: &str) -> String {
    format!("{script_id}::{node_id}")
}

#[cfg(test)]
mod tests {
    use baudbound_runtime::RunIdentity;
    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn records_successful_dispatch_report() {
        let mut activity = ServiceActivity::default();
        let report = RunReport {
            identity: RunIdentity {
                run_id: "run-1".to_owned(),
                script_id: "script-1".to_owned(),
                trigger_node_id: "trigger-1".to_owned(),
            },
            logs: Vec::new(),
            variable_scopes: Default::default(),
            variables: Default::default(),
        };

        activity.record_report("schedule", &report);
        let serialized = serde_json::to_value(&activity).expect("activity should serialize");

        assert_eq!(serialized["total_dispatch_count"], 1);
        assert_eq!(serialized["failed_dispatch_count"], 0);
        assert_eq!(serialized["last_dispatch"]["source"], "schedule");
        assert_eq!(serialized["last_dispatch"]["status"], "completed");
        assert_eq!(serialized["last_dispatch"]["run_id"], "run-1");
        assert_eq!(
            serialized["triggers"]["script-1::trigger-1"]["successful_dispatch_count"],
            1
        );
        assert_eq!(
            serialized["triggers"]["script-1::trigger-1"]["failed_dispatch_count"],
            0
        );
    }

    #[test]
    fn records_failed_dispatch_event() {
        let mut activity = ServiceActivity::default();
        let event = TriggerEvent {
            action_type: "trigger.webhook".to_owned(),
            node_id: "trigger-1".to_owned(),
            payload: json!({ "value": 1 }),
            script_id: "script-1".to_owned(),
        };

        activity.record_event_failure("webhook", &event, "failed hard");
        let serialized = serde_json::to_value(&activity).expect("activity should serialize");

        assert_eq!(serialized["total_dispatch_count"], 1);
        assert_eq!(serialized["failed_dispatch_count"], 1);
        assert_eq!(serialized["last_dispatch"]["source"], "webhook");
        assert_eq!(serialized["last_dispatch"]["status"], "failed");
        assert_eq!(serialized["last_dispatch"]["run_id"], Value::Null);
        assert_eq!(serialized["last_dispatch"]["error"], "failed hard");
        assert_eq!(
            serialized["triggers"]["script-1::trigger-1"]["successful_dispatch_count"],
            0
        );
        assert_eq!(
            serialized["triggers"]["script-1::trigger-1"]["failed_dispatch_count"],
            1
        );
    }
}
