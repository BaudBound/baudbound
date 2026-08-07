use std::time::Duration;

use serde_json::Value;

use crate::{TriggerError, TriggerRegistration};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ScheduleSpec {
    pub(super) every: u64,
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
        let seconds = every as f64 * schedule_unit_seconds(normalized_unit);
        let interval = Duration::try_from_secs_f64(seconds)
            .ok()
            .filter(|interval| *interval >= Duration::from_millis(1))
            .ok_or_else(|| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    "schedule interval must fit the supported duration range and be at least one millisecond"
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

/// Reads the interval count, which is a whole number of the chosen unit.
///
/// A fraction is refused rather than rounded. Milliseconds is the smallest
/// unit and the interval cannot go below one of them, so every interval the
/// runner will accept can be written exactly as whole units, and a fraction
/// only ever means the author expected a precision that does not exist.
fn schedule_every(config: &Value) -> Option<u64> {
    let value = match config.get("every")? {
        Value::Number(value) => value.as_u64()?,
        Value::String(value) => value.trim().parse::<u64>().ok()?,
        _ => return None,
    };
    (value > 0).then_some(value)
}

fn schedule_unit_seconds(unit: &str) -> f64 {
    match unit {
        "milliseconds" => 0.001,
        "seconds" => 1.0,
        "minutes" => 60.0,
        "hours" => 60.0 * 60.0,
        "days" => 24.0 * 60.0 * 60.0,
        _ => unreachable!("schedule unit is normalized before conversion"),
    }
}

fn normalize_schedule_unit(unit: &str) -> Option<&'static str> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "ms" | "millisecond" | "milliseconds" => Some("milliseconds"),
        "s" | "sec" | "second" | "seconds" => Some("seconds"),
        "m" | "min" | "minute" | "minutes" => Some("minutes"),
        "h" | "hr" | "hour" | "hours" => Some("hours"),
        "d" | "day" | "days" => Some("days"),
        _ => None,
    }
}
