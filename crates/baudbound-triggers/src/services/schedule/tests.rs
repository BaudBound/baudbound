use std::time::{Duration, Instant, SystemTime};

use baudbound_runtime::ResourceLimit;
use serde_json::json;

use super::{ScheduleService, spec::ScheduleSpec};
use crate::TriggerRegistration;

#[test]
fn accepts_sub_second_intervals_and_preserves_exact_payload_seconds() {
    let start = Instant::now();
    let mut service = ScheduleService::from_registrations(
        [registration("n-sub-second", "250", "milliseconds")],
        start,
    )
    .expect("a sub second schedule should parse");

    assert!(
        service
            .due_events(start + Duration::from_millis(249), SystemTime::UNIX_EPOCH)
            .is_empty()
    );
    let events = service.due_events(start + Duration::from_millis(250), SystemTime::UNIX_EPOCH);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["interval_seconds"], 0.25);
    assert_eq!(events[0].payload["schedule"]["every"], 250);
}

#[test]
fn a_fractional_interval_count_is_refused_rather_than_rounded() {
    let start = Instant::now();

    // Milliseconds is the smallest unit and one of them is the smallest
    // interval, so a fraction can only mean a precision that does not exist.
    for every in ["0.25", "1.5", "0.5"] {
        assert!(
            ScheduleService::from_registrations(
                [registration("n-fractional", every, "seconds")],
                start,
            )
            .is_err(),
            "every={every} should be refused"
        );
    }

    // The same interval written in whole units is accepted.
    assert!(
        ScheduleService::from_registrations(
            [registration("n-whole", "1500", "milliseconds")],
            start,
        )
        .is_ok()
    );
}

#[test]
fn interval_seconds_is_a_float_even_for_a_whole_number_of_seconds() {
    let start = Instant::now();
    let mut service =
        ScheduleService::from_registrations([registration("n-whole", "30", "seconds")], start)
            .expect("a whole second schedule should parse");

    let events = service.due_events(start + Duration::from_secs(30), SystemTime::UNIX_EPOCH);
    assert_eq!(events.len(), 1);
    let interval = &events[0].payload["interval_seconds"];
    assert!(
        interval.is_f64() && !interval.is_i64() && !interval.is_u64(),
        "interval_seconds must stay a float so its type does not follow the value: {interval}"
    );
    assert_eq!(interval.as_f64(), Some(30.0));

    // The interval count keeps the type the author entered.
    let every = &events[0].payload["schedule"]["every"];
    assert!(every.is_u64(), "every must stay an integer: {every}");
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
fn one_millisecond_intervals_are_accepted_without_clamping() {
    let start = Instant::now();
    let mut service = ScheduleService::from_registrations(
        [registration("n-one-millisecond", "1", "milliseconds")],
        start,
    )
    .expect("one millisecond schedule should parse");

    assert!(
        service
            .due_events(start + Duration::from_micros(999), SystemTime::UNIX_EPOCH)
            .is_empty()
    );
    let events = service.due_events(start + Duration::from_millis(1), SystemTime::UNIX_EPOCH);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["interval_seconds"], 0.001);
}

#[test]
fn one_millisecond_catch_up_preserves_ten_thousand_ticks_in_bounded_batches() {
    let start = Instant::now();
    let mut service = ScheduleService::from_registrations(
        [registration("n-one-millisecond", "1", "milliseconds")],
        start,
    )
    .expect("one millisecond schedule should parse");
    let delayed = start + Duration::from_secs(10);
    let mut emitted = 0_u64;

    loop {
        let dispatch = service.for_each_due_event_with_limit(
            delayed,
            SystemTime::UNIX_EPOCH + Duration::from_secs(10),
            ResourceLimit::limited(127),
            |_| {},
        );
        emitted += dispatch.emitted;
        if !dispatch.deferred {
            break;
        }
    }

    assert_eq!(emitted, 10_000);
    assert_eq!(
        service.time_until_next_due(delayed),
        Some(Duration::from_millis(1))
    );
}

#[test]
fn delayed_poll_emits_every_tick_without_cadence_drift() {
    let start = Instant::now();
    let mut service =
        ScheduleService::from_registrations([registration("n-schedule", "10", "seconds")], start)
            .expect("schedule should parse");
    let delayed = start + Duration::from_secs(35);

    let events = service.due_events(delayed, SystemTime::UNIX_EPOCH + Duration::from_secs(35));
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].payload["scheduled_at_unix"], 10);
    assert_eq!(events[1].payload["scheduled_at_unix"], 20);
    assert_eq!(events[2].payload["scheduled_at_unix"], 30);
    assert_eq!(
        service.time_until_next_due(delayed),
        Some(Duration::from_secs(5))
    );
}

#[test]
fn finite_catch_up_limit_defers_ticks_without_dropping_them() {
    let start = Instant::now();
    let mut service =
        ScheduleService::from_registrations([registration("n-schedule", "1", "seconds")], start)
            .expect("schedule should parse");
    let delayed = start + Duration::from_secs(5);

    let first = service.due_events_with_limit(
        delayed,
        SystemTime::UNIX_EPOCH + Duration::from_secs(5),
        ResourceLimit::limited(2),
    );
    assert_eq!(first.events.len(), 2);
    assert!(first.deferred);

    let second = service.due_events_with_limit(
        delayed,
        SystemTime::UNIX_EPOCH + Duration::from_secs(5),
        ResourceLimit::limited(3),
    );
    assert_eq!(second.events.len(), 3);
    assert!(!second.deferred);
    assert_eq!(
        first
            .events
            .into_iter()
            .chain(second.events)
            .map(|event| event.payload["scheduled_at_unix"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
}

#[test]
fn three_second_schedule_fires_at_three_second_boundaries() {
    let start = Instant::now();
    let mut service = ScheduleService::from_registrations(
        [registration("n-three-seconds", "3", "seconds")],
        start,
    )
    .expect("schedule should parse");

    assert!(
        service
            .due_events(start + Duration::from_millis(2_999), SystemTime::UNIX_EPOCH)
            .is_empty()
    );
    for elapsed_seconds in [3, 6, 9] {
        let now = start + Duration::from_secs(elapsed_seconds);
        let events = service.due_events(now, SystemTime::UNIX_EPOCH);
        assert_eq!(events.len(), 1, "expected a tick at {elapsed_seconds}s");
        assert_eq!(events[0].payload["interval_seconds"], 3.0);
        assert_eq!(
            service.time_until_next_due(now),
            Some(Duration::from_secs(3))
        );
    }
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

#[test]
fn missed_intervals_counts_the_backlog_behind_each_event() {
    let start = Instant::now();
    let mut service =
        ScheduleService::from_registrations([registration("n-schedule", "10", "seconds")], start)
            .expect("schedule should parse");

    // A punctual tick has nothing behind it.
    let punctual = service.due_events(start + Duration::from_secs(10), SystemTime::UNIX_EPOCH);
    assert_eq!(punctual.len(), 1);
    assert_eq!(punctual[0].payload["missed_intervals"], 0);

    // A poll that arrives 25 seconds late replays every tick it owes. The
    // first carries the deepest backlog and the last is punctual again.
    let delayed = start + Duration::from_secs(35);
    let events = service.due_events(delayed, SystemTime::UNIX_EPOCH + Duration::from_secs(35));
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].payload["missed_intervals"], 1);
    assert_eq!(events[1].payload["missed_intervals"], 0);

    // The count is an integer, matching what the node declares.
    assert!(events[0].payload["missed_intervals"].is_u64());
}
