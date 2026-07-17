use std::time::{Duration, Instant, SystemTime};

use serde_json::json;

use super::{ScheduleService, spec::ScheduleSpec};
use crate::TriggerRegistration;

#[test]
fn accepts_fractional_intervals_and_preserves_exact_payload_seconds() {
    let start = Instant::now();
    let mut service = ScheduleService::from_registrations(
        [registration("n-fractional", "0.25", "seconds")],
        start,
    )
    .expect("fractional schedule should parse");

    assert!(
        service
            .due_events(start + Duration::from_millis(249), SystemTime::UNIX_EPOCH)
            .is_empty()
    );
    let events = service.due_events(start + Duration::from_millis(250), SystemTime::UNIX_EPOCH);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["interval_seconds"], 0.25);
}

#[test]
fn accepts_millisecond_intervals() {
    let start = Instant::now();
    let mut service = ScheduleService::from_registrations(
        [registration("n-milliseconds", "25", "milliseconds")],
        start,
    )
    .expect("millisecond schedule should parse");

    assert!(
        service
            .due_events(start + Duration::from_millis(24), SystemTime::UNIX_EPOCH)
            .is_empty()
    );
    let events = service.due_events(start + Duration::from_millis(25), SystemTime::UNIX_EPOCH);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["interval_seconds"], 0.025);
    assert_eq!(events[0].payload["schedule"]["unit"], "milliseconds");
}

#[test]
fn delayed_poll_coalesces_missed_ticks_without_cadence_drift() {
    let start = Instant::now();
    let mut service =
        ScheduleService::from_registrations([registration("n-schedule", "10", "seconds")], start)
            .expect("schedule should parse");
    let delayed = start + Duration::from_secs(35);

    let events = service.due_events(delayed, SystemTime::UNIX_EPOCH);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["missed_intervals"], 2);
    assert_eq!(
        service.time_until_next_due(delayed),
        Some(Duration::from_secs(5))
    );
}

#[test]
fn reload_preserves_unchanged_deadlines_and_resets_changed_schedules() {
    let start = Instant::now();
    let original = ScheduleService::from_registrations(
        [
            registration("n-unchanged", "10", "seconds"),
            registration("n-changed", "10", "seconds"),
            registration("n-removed", "10", "seconds"),
        ],
        start,
    )
    .expect("original schedules should parse");
    let reload_at = start + Duration::from_secs(9);
    let mut reloaded = ScheduleService::start_or_reconfigure(
        [
            registration("n-unchanged", "10", "seconds"),
            registration("n-changed", "20", "seconds"),
            registration("n-added", "20", "seconds"),
        ],
        reload_at,
        Some(original),
    )
    .expect("schedules should reload");

    assert_eq!(reloaded.len(), 3);
    assert_eq!(
        reloaded.time_until_next_due(reload_at),
        Some(Duration::from_secs(1))
    );
    let events = reloaded.due_events(start + Duration::from_secs(10), SystemTime::UNIX_EPOCH);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].node_id, "n-unchanged");
    assert_eq!(
        reloaded.time_until_next_due(start + Duration::from_secs(10)),
        Some(Duration::from_secs(10))
    );
}

#[test]
fn rejects_non_finite_zero_sub_millisecond_and_duplicate_schedules() {
    for (every, unit) in [
        ("0", "seconds"),
        ("NaN", "seconds"),
        ("inf", "seconds"),
        ("0.0009", "seconds"),
        ("0.999", "milliseconds"),
    ] {
        assert!(
            ScheduleService::from_registrations(
                [registration("n-invalid", every, unit)],
                Instant::now(),
            )
            .is_err(),
            "{every} {unit} must be rejected"
        );
    }

    let duplicate = registration("n-duplicate", "1", "seconds");
    assert!(
        ScheduleService::from_registrations([duplicate.clone(), duplicate], Instant::now())
            .is_err()
    );
}

#[test]
fn rejects_unknown_units() {
    let error = ScheduleService::from_registrations(
        [registration("n-invalid-unit", "1", "fortnights")],
        Instant::now(),
    )
    .expect_err("unknown schedule units must be rejected");

    assert!(error.to_string().contains("unsupported schedule unit"));
}

#[test]
fn accepts_duration_boundary_below_rust_limit_and_rejects_the_limit() {
    let rust_limit = 2_f64.powi(64);
    let largest_supported = f64::from_bits(rust_limit.to_bits() - 1);
    ScheduleSpec::from_registration(&registration(
        "n-largest",
        &largest_supported.to_string(),
        "seconds",
    ))
    .expect("largest f64 duration below the Rust limit should parse");

    assert!(
        ScheduleSpec::from_registration(&registration(
            "n-overflow",
            &rust_limit.to_string(),
            "seconds",
        ))
        .is_err(),
    );
}

fn registration(node_id: &str, every: &str, unit: &str) -> TriggerRegistration {
    TriggerRegistration {
        action_type: "trigger.schedule".to_owned(),
        config: json!({ "every": every, "unit": unit }),
        node_id: node_id.to_owned(),
        runner_type: "schedule".to_owned(),
        script_id: "script-schedule".to_owned(),
        script_name: "Schedule Script".to_owned(),
    }
}
