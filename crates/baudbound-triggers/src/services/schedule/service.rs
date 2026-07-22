use std::{
    collections::BTreeMap,
    time::{Duration, Instant, SystemTime},
};

use serde_json::json;

use crate::{
    TriggerError, TriggerEvent, TriggerRegistration, TriggerServiceDiagnostics, unix_timestamp,
};

use super::spec::ScheduleSpec;

#[derive(Debug, Clone)]
pub struct ScheduleService {
    schedules: BTreeMap<ScheduleId, ScheduleTask>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct ScheduleId {
    node_id: String,
    script_id: String,
}

#[derive(Debug, Clone)]
struct ScheduleTask {
    next_due: Instant,
    registration: TriggerRegistration,
    spec: ScheduleSpec,
}

impl ScheduleService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schedules: BTreeMap::new(),
        }
    }

    pub fn from_registrations(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        start: Instant,
    ) -> Result<Self, TriggerError> {
        Self::start_or_reconfigure(registrations, start, None)
    }

    pub fn start_or_reconfigure(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        start: Instant,
        previous: Option<Self>,
    ) -> Result<Self, TriggerError> {
        let mut previous = previous.unwrap_or_else(Self::empty).schedules;
        let mut schedules = BTreeMap::new();
        for registration in registrations
            .into_iter()
            .filter(|registration| registration.action_type == "trigger.schedule")
        {
            let spec = ScheduleSpec::from_registration(&registration)?;
            let id = ScheduleId {
                node_id: registration.node_id.clone(),
                script_id: registration.script_id.clone(),
            };
            let next_due = match previous.remove(&id) {
                Some(task) if task.spec == spec => task.next_due,
                _ => start.checked_add(spec.interval).ok_or_else(|| {
                    TriggerError::Failed(
                        registration.node_id.clone(),
                        "schedule interval exceeds the monotonic clock range".to_owned(),
                    )
                })?,
            };
            if schedules
                .insert(
                    id,
                    ScheduleTask {
                        next_due,
                        registration: registration.clone(),
                        spec,
                    },
                )
                .is_some()
            {
                return Err(TriggerError::Failed(
                    registration.node_id,
                    "duplicate schedule trigger registration".to_owned(),
                ));
            }
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
        for schedule in self.schedules.values_mut() {
            schedule.next_due = now;
        }
    }

    #[must_use]
    pub fn time_until_next_due(&self, now: Instant) -> Option<Duration> {
        self.schedules
            .values()
            .map(|schedule| schedule.next_due.saturating_duration_since(now))
            .min()
    }

    pub fn due_events(&mut self, now: Instant, timestamp: SystemTime) -> Vec<TriggerEvent> {
        let timestamp_unix = unix_timestamp(timestamp);
        self.schedules
            .values_mut()
            .filter_map(|schedule| {
                (now >= schedule.next_due).then(|| {
                    let missed_intervals = advance_schedule(schedule, now);
                    TriggerEvent {
                        action_type: schedule.registration.action_type.clone(),
                        node_id: schedule.registration.node_id.clone(),
                        payload: json!({
                            "interval_seconds": schedule_number_payload(
                                schedule.spec.interval.as_secs_f64()
                            ),
                            "missed_intervals": missed_intervals,
                            "schedule": {
                                "every": schedule_number_payload(schedule.spec.every),
                                "unit": schedule.spec.unit,
                            },
                            "scheduled_at_unix": timestamp_unix,
                        }),
                        script_id: schedule.registration.script_id.clone(),
                    }
                })
            })
            .collect()
    }
}

fn advance_schedule(schedule: &mut ScheduleTask, now: Instant) -> u64 {
    let overdue = now.saturating_duration_since(schedule.next_due);
    let missed_intervals = overdue.as_nanos() / schedule.spec.interval.as_nanos();
    let advance_intervals = missed_intervals.saturating_add(1);
    let advance = duration_from_nanos(
        schedule
            .spec
            .interval
            .as_nanos()
            .saturating_mul(advance_intervals),
    );
    schedule.next_due = schedule
        .next_due
        .checked_add(advance)
        .or_else(|| now.checked_add(schedule.spec.interval))
        .unwrap_or(now);
    u64::try_from(missed_intervals).unwrap_or(u64::MAX)
}

fn duration_from_nanos(nanos: u128) -> Duration {
    const NANOS_PER_SECOND: u128 = 1_000_000_000;
    let seconds = u64::try_from(nanos / NANOS_PER_SECOND).unwrap_or(u64::MAX);
    let subsecond_nanos = (nanos % NANOS_PER_SECOND) as u32;
    Duration::new(seconds, subsecond_nanos)
}

fn schedule_number_payload(value: f64) -> serde_json::Value {
    if value.fract() == 0.0 && value <= u64::MAX as f64 {
        json!(value as u64)
    } else {
        json!(value)
    }
}
