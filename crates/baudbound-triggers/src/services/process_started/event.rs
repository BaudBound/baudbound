use std::time::SystemTime;

use serde_json::json;

use crate::{TriggerEvent, unix_timestamp_millis};

use super::{snapshot::ProcessSnapshot, spec::ProcessStartedSpec};

pub(super) fn process_started_event(
    spec: &ProcessStartedSpec,
    process: &ProcessSnapshot,
    window_title: &str,
    detected_at: SystemTime,
) -> TriggerEvent {
    TriggerEvent {
        node_id: spec.registration.node_id.clone(),
        payload: json!({
            "executable_path": process
                .executable_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            "process_id": process.identity.process_id,
            "process_name": process.process_name.to_string_lossy(),
            "timestamp": unix_timestamp_millis(detected_at).to_string(),
            "window_title": window_title,
        }),
        script_id: spec.registration.script_id.clone(),
    }
}
