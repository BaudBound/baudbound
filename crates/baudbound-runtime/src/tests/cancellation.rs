use std::{
    thread,
    time::{Duration, Instant},
};

use serde_json::json;

use super::*;

#[test]
fn cancellation_interrupts_a_long_delay_promptly() {
    let token = RuntimeCancellationToken::new();
    let cancellation = token.clone();
    let cancel_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(25));
        cancellation.cancel();
    });
    let started = Instant::now();
    let error = execute_manual_program_with_state(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [{
                        "id": "n-delay",
                        "action_type": "action.delay",
                        "type": "delay",
                        "config": {"amount": 30, "unit": "seconds"},
                        "runtime_outputs": []
                    }],
                    "edges": [edge("n-trigger", "out", "n-delay")]
                }
            }
        }),
        "script-1",
        RuntimeExecutionResources::new(&UnsupportedActionHandler).with_cancellation(token),
    )
    .expect_err("cancelled delay must stop execution");
    cancel_thread
        .join()
        .expect("cancellation thread should join");

    assert!(matches!(error, RuntimeError::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn cancellation_stops_unbounded_control_flow_at_frame_boundaries() {
    let token = RuntimeCancellationToken::new();
    let cancellation = token.clone();
    let cancel_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        cancellation.cancel();
    });
    let error = execute_manual_program_with_state(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [{
                        "id": "n-while",
                        "action_type": "control.while",
                        "type": "while",
                        "config": {
                            "conditions": [{
                                "id": "always",
                                "left": "1",
                                "operator": "==",
                                "right": "1"
                            }]
                        },
                        "runtime_outputs": []
                    }],
                    "edges": [edge("n-trigger", "out", "n-while")]
                }
            }
        }),
        "script-1",
        RuntimeExecutionResources::new(&UnsupportedActionHandler).with_cancellation(token),
    )
    .expect_err("cancelled control flow must stop execution");
    cancel_thread
        .join()
        .expect("cancellation thread should join");

    assert!(matches!(error, RuntimeError::Cancelled));
}

#[test]
fn pre_cancelled_execution_does_not_start_the_graph() {
    let token = RuntimeCancellationToken::new();
    token.cancel();
    let error = execute_manual_program_with_state(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {"steps": [], "edges": []}
            }
        }),
        "script-1",
        RuntimeExecutionResources::new(&UnsupportedActionHandler).with_cancellation(token),
    )
    .expect_err("pre-cancelled run must not start");

    assert!(matches!(error, RuntimeError::Cancelled));
}
