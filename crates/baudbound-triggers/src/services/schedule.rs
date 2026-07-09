use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};

use crate::{
    TriggerError, TriggerEvent, TriggerRegistration, TriggerServiceDiagnostics, unix_timestamp,
};

#[derive(Debug, Clone)]
pub struct ScheduleService {
    schedules: Vec<ScheduleTask>,
}

impl ScheduleService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schedules: Vec::new(),
        }
    }

    pub fn from_registrations(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        start: Instant,
    ) -> Result<Self, TriggerError> {
        let mut schedules = Vec::new();

        for registration in registrations {
            if registration.action_type != "trigger.schedule" {
                continue;
            }

            let spec = ScheduleSpec::from_registration(&registration)?;
            schedules.push(ScheduleTask {
                interval: spec.interval,
                next_due: start + spec.interval,
                registration,
                spec,
            });
        }

        Ok(Self { schedules })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.schedules.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.schedules.len()
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::active(self.len(), "schedule")
    }

    pub fn mark_all_due_now(&mut self, now: Instant) {
        for schedule in &mut self.schedules {
            schedule.next_due = now;
        }
    }

    #[must_use]
    pub fn time_until_next_due(&self, now: Instant) -> Option<Duration> {
        self.schedules
            .iter()
            .map(|schedule| schedule.next_due.saturating_duration_since(now))
            .min()
    }

    pub fn due_events(&mut self, now: Instant, timestamp: SystemTime) -> Vec<TriggerEvent> {
        let timestamp_unix = unix_timestamp(timestamp);
        let mut events = Vec::new();

        for schedule in &mut self.schedules {
            if now < schedule.next_due {
                continue;
            }

            events.push(TriggerEvent {
                node_id: schedule.registration.node_id.clone(),
                payload: json!({
                    "scheduled_at_unix": timestamp_unix,
                    "interval_seconds": schedule.interval.as_secs(),
                    "schedule": {
                        "every": schedule.spec.every,
                        "unit": schedule.spec.unit,
                    },
                }),
                script_id: schedule.registration.script_id.clone(),
            });

            while schedule.next_due <= now {
                schedule.next_due += schedule.interval;
            }
        }

        events
    }
}

#[derive(Debug, Clone)]
struct ScheduleTask {
    interval: Duration,
    next_due: Instant,
    registration: TriggerRegistration,
    spec: ScheduleSpec,
}

#[derive(Debug, Clone)]
struct ScheduleSpec {
    every: u64,
    interval: Duration,
    unit: String,
}

impl ScheduleSpec {
    fn from_registration(registration: &TriggerRegistration) -> Result<Self, TriggerError> {
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
        let unit_seconds = schedule_unit_seconds(unit).ok_or_else(|| {
            TriggerError::Failed(
                registration.node_id.clone(),
                format!("unsupported schedule unit {unit:?}"),
            )
        })?;

        Ok(Self {
            every,
            interval: Duration::from_secs(every.saturating_mul(unit_seconds)),
            unit: normalized_schedule_unit(unit).to_owned(),
        })
    }
}

fn schedule_every(config: &Value) -> Option<u64> {
    match config.get("every")? {
        Value::Number(value) => value.as_u64().filter(|value| *value > 0),
        Value::String(value) => value.trim().parse::<u64>().ok().filter(|value| *value > 0),
        _ => None,
    }
}

fn schedule_unit_seconds(unit: &str) -> Option<u64> {
    match normalized_schedule_unit(unit) {
        "seconds" => Some(1),
        "minutes" => Some(60),
        "hours" => Some(60 * 60),
        "days" => Some(24 * 60 * 60),
        _ => None,
    }
}

fn normalized_schedule_unit(unit: &str) -> &str {
    match unit.trim().to_ascii_lowercase().as_str() {
        "s" | "sec" | "second" | "seconds" => "seconds",
        "m" | "min" | "minute" | "minutes" => "minutes",
        "h" | "hr" | "hour" | "hours" => "hours",
        "d" | "day" | "days" => "days",
        _ => "",
    }
}
