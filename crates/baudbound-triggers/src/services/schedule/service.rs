use std::{
    collections::BTreeMap,
    time::{Duration, Instant, SystemTime},
};

use baudbound_runtime::ResourceLimit;
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
        self.due_events_with_limit(now, timestamp, ResourceLimit::Unlimited)
            .events
    }

    pub fn due_events_with_limit(
        &mut self,
        now: Instant,
        timestamp: SystemTime,
        limit: ResourceLimit,
    ) -> DueScheduleBatch {
        let mut events = Vec::new();
        let dispatch = self.for_each_due_event_with_limit(now, timestamp, limit, |event| {
            events.push(event);
        });
        DueScheduleBatch {
            deferred: dispatch.deferred,
            events,
        }
    }

    pub fn for_each_due_event_with_limit(
        &mut self,
        now: Instant,
        timestamp: SystemTime,
        limit: ResourceLimit,
        mut emit: impl FnMut(TriggerEvent),
    ) -> DueScheduleDispatch {
        let mut emitted = 0_u64;
        while emitted
            .checked_add(1)
            .is_some_and(|next| limit.permits(next))
        {
            let Some(id) = self
                .schedules
                .iter()
                .filter(|(_, schedule)| schedule.next_due <= now)
                .min_by_key(|(_, schedule)| schedule.next_due)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            let schedule = self
                .schedules
                .get_mut(&id)
                .expect("selected schedule must remain registered");
            let scheduled_at = timestamp
                .checked_sub(now.saturating_duration_since(schedule.next_due))
                .unwrap_or(timestamp);
            emit(TriggerEvent {
                action_type: schedule.registration.action_type.clone(),
                node_id: schedule.registration.node_id.clone(),
                payload: json!({
                    "interval_seconds": schedule_number_payload(
                        schedule.spec.interval.as_secs_f64()
                    ),
                    "schedule": {
                        "every": schedule_number_payload(schedule.spec.every),
                        "unit": schedule.spec.unit,
                    },
                    "scheduled_at_unix": unix_timestamp(scheduled_at),
                }),
                script_id: schedule.registration.script_id.clone(),
            });
            emitted = emitted
                .checked_add(1)
                .expect("schedule emission count was checked before dispatch");
            schedule.next_due = schedule
                .next_due
                .checked_add(schedule.spec.interval)
                .unwrap_or_else(|| now.checked_add(schedule.spec.interval).unwrap_or(now));
        }
        let deferred = self
            .schedules
            .values()
            .any(|schedule| schedule.next_due <= now);
        DueScheduleDispatch { deferred, emitted }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DueScheduleDispatch {
    pub deferred: bool,
    pub emitted: u64,
}

#[derive(Debug)]
pub struct DueScheduleBatch {
    pub events: Vec<TriggerEvent>,
    pub deferred: bool,
}

fn schedule_number_payload(value: f64) -> serde_json::Value {
    if value.fract() == 0.0 && value <= u64::MAX as f64 {
        json!(value as u64)
    } else {
        json!(value)
    }
}
