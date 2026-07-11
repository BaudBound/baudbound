use std::time::Duration;

use serde_json::Value;

use crate::{TriggerError, TriggerRegistration};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ScheduleSpec {
    pub(super) every: f64,
    pub(super) interval: Duration,
    pub(super) unit: String,
}

impl ScheduleSpec {
    pub(super) fn from_registration(
        registration: &TriggerRegistration,
    ) -> Result<Self, TriggerError> {
        let every = schedule_every(&registration.config).ok_or_else(|| {
            TriggerError::Failed(
                registration.node_id.clone(),
                "schedule trigger must define a positive every value".to_owned(),
            )
        })?;
        let unit = registration
            .config
            .get("unit")
            .and_then(Value::as_str)
            .unwrap_or("minutes");
        let normalized_unit = normalize_schedule_unit(unit).ok_or_else(|| {
            TriggerError::Failed(
                registration.node_id.clone(),
                format!("unsupported schedule unit {unit:?}"),
            )
        })?;
        let seconds = every * schedule_unit_seconds(normalized_unit);
        let interval = Duration::try_from_secs_f64(seconds)
            .ok()
            .filter(|interval| !interval.is_zero())
            .ok_or_else(|| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    "schedule interval must fit the supported duration range and be at least one nanosecond"
                        .to_owned(),
                )
            })?;

        Ok(Self {
            every,
            interval,
            unit: normalized_unit.to_owned(),
        })
    }
}

fn schedule_every(config: &Value) -> Option<f64> {
    let value = match config.get("every")? {
        Value::Number(value) => value.as_f64()?,
        Value::String(value) => value.trim().parse::<f64>().ok()?,
        _ => return None,
    };
    (value.is_finite() && value > 0.0).then_some(value)
}

fn schedule_unit_seconds(unit: &str) -> f64 {
    match unit {
        "seconds" => 1.0,
        "minutes" => 60.0,
        "hours" => 60.0 * 60.0,
        "days" => 24.0 * 60.0 * 60.0,
        _ => unreachable!("schedule unit is normalized before conversion"),
    }
}

fn normalize_schedule_unit(unit: &str) -> Option<&'static str> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "s" | "sec" | "second" | "seconds" => Some("seconds"),
        "m" | "min" | "minute" | "minutes" => Some("minutes"),
        "h" | "hr" | "hour" | "hours" => Some("hours"),
        "d" | "day" | "days" => Some("days"),
        _ => None,
    }
}
