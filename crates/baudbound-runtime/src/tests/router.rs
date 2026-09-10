use serde_json::{Value, json};

use crate::{RuntimeError, RuntimeLogEntry, tests::execute_manual_program};

fn manual_trigger() -> Value {
    json!({
        "id": "n-trigger",
        "action_type": "trigger.manual",
        "type": "manual",
        "config": {},
        "runtime_outputs": []
    })
}

fn program(steps: Vec<Value>, edges: Vec<Value>) -> Value {
    json!({
        "entry": {
            "trigger": manual_trigger(),
            "triggers": [],
            "program": {"steps": steps, "edges": edges}
        }
    })
}

fn edge(source: &str, source_handle: &str, target: &str, target_handle: &str) -> Value {
    json!({
        "execution_order": 0,
        "source": source,
        "source_handle": source_handle,
        "target": target,
        "target_handle": target_handle
    })
}

fn log_node(id: &str, message: &str) -> Value {
    json!({
        "id": id,
        "action_type": "action.log",
        "type": "action",
        "action": "log",
        "config": {"level": "info", "message": message},
        "runtime_outputs": []
    })
}

fn router_node(config: Value) -> Value {
    json!({
        "id": "n-router",
        "action_type": "control.router",
        "type": "router",
        "config": config,
        "runtime_outputs": []
    })
}

fn two_by_two_router() -> Value {
    json!({
        "inputs": [{"id": "a", "label": "Alpha"}, {"id": "b", "label": "Beta"}],
        "outputs": [{"id": "x", "label": "X"}, {"id": "y", "label": "Y"}],
        "routes": [
            {"id": "r1", "inputId": "a", "outputId": "y", "order": 0},
            {"id": "r2", "inputId": "a", "outputId": "x", "order": 1},
            {"id": "r3", "inputId": "b", "outputId": "x", "order": 0}
        ]
    })
}

/// The log messages the test graphs emit, in execution order. Filtering by
/// exact text keeps runtime diagnostics such as "Branch ended." out of the
/// comparison.
fn logged_messages(logs: &[RuntimeLogEntry]) -> Vec<&str> {
    logs.iter()
        .map(|entry| entry.message.as_str())
        .filter(|message| *message == "x ran" || *message == "y ran")
        .collect()
}

#[test]
fn router_routes_one_input_to_one_output() {
    let report = execute_manual_program(
        &program(
            vec![
                router_node(json!({
                    "inputs": [{"id": "a", "label": "Alpha"}],
                    "outputs": [{"id": "x", "label": "X"}],
                    "routes": [{"id": "r", "inputId": "a", "outputId": "x", "order": 0}]
                })),
                log_node("n-x", "x ran"),
            ],
            vec![
                edge("n-trigger", "out", "n-router", "in-a"),
                edge("n-router", "out-x", "n-x", "input"),
            ],
        ),
        "router-one-to-one",
    )
    .expect("router should route to its single output");

    assert_eq!(logged_messages(&report.logs), vec!["x ran"]);
}

#[test]
fn router_follows_multiple_outputs_in_configured_order() {
    let report = execute_manual_program(
        &program(
            vec![
                router_node(two_by_two_router()),
                log_node("n-x", "x ran"),
                log_node("n-y", "y ran"),
            ],
            vec![
                edge("n-trigger", "out", "n-router", "in-a"),
                edge("n-router", "out-x", "n-x", "input"),
                edge("n-router", "out-y", "n-y", "input"),
            ],
        ),
        "router-ordered-fan-out",
    )
    .expect("router should fan out");

    assert_eq!(logged_messages(&report.logs), vec!["y ran", "x ran"]);
    assert!(
        report.logs.iter().any(|entry| {
            entry.node_id.as_deref() == Some("n-router")
                && entry.message.contains("input \"Alpha\"")
                && entry.message.contains("out-y, out-x")
        }),
        "router diagnostics should name the input and the ordered outputs"
    );
}

#[test]
fn router_only_follows_routes_for_the_incoming_input() {
    let report = execute_manual_program(
        &program(
            vec![
                router_node(two_by_two_router()),
                log_node("n-x", "x ran"),
                log_node("n-y", "y ran"),
            ],
            vec![
                edge("n-trigger", "out", "n-router", "in-b"),
                edge("n-router", "out-x", "n-x", "input"),
                edge("n-router", "out-y", "n-y", "input"),
            ],
        ),
        "router-input-selection",
    )
    .expect("router should route input b");

    assert_eq!(logged_messages(&report.logs), vec!["x ran"]);
}

#[test]
fn router_rejects_an_unknown_input_handle() {
    let error = execute_manual_program(
        &program(
            vec![router_node(two_by_two_router()), log_node("n-x", "x ran")],
            vec![
                edge("n-trigger", "out", "n-router", "in-missing"),
                edge("n-router", "out-x", "n-x", "input"),
            ],
        ),
        "router-unknown-input",
    )
    .expect_err("an unknown input handle must fail control flow");

    assert!(
        matches!(error, RuntimeError::ControlFlow { ref node_id, .. } if node_id == "n-router"),
        "{error}"
    );
    assert!(error.to_string().contains("in-missing"), "{error}");
}

#[test]
fn router_reports_malformed_config_clearly() {
    let error = execute_manual_program(
        &program(
            vec![router_node(
                json!({"inputs": "nope", "outputs": [], "routes": []}),
            )],
            vec![edge("n-trigger", "out", "n-router", "in-a")],
        ),
        "router-malformed",
    )
    .expect_err("malformed router config must fail");

    assert!(
        error.to_string().contains("router config is malformed"),
        "{error}"
    );
}

#[test]
fn router_output_without_connection_ends_that_branch_only() {
    let report = execute_manual_program(
        &program(
            vec![router_node(two_by_two_router()), log_node("n-x", "x ran")],
            vec![
                edge("n-trigger", "out", "n-router", "in-a"),
                edge("n-router", "out-x", "n-x", "input"),
            ],
        ),
        "router-dangling-output",
    )
    .expect("an unconnected router output is not an error");

    assert_eq!(logged_messages(&report.logs), vec!["x ran"]);
}
