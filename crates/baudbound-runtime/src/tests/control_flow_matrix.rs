use serde_json::{Value, json};

use crate::{execute_manual_program, runtime::compare_condition_values};

#[derive(serde::Deserialize)]
struct ConditionEqualityCase {
    name: String,
    left: Value,
    right: Value,
    expected: bool,
}

#[test]
fn condition_equality_follows_the_shared_editor_parity_matrix() {
    let cases: Vec<ConditionEqualityCase> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../baudbound-script/contracts/condition-equality-cases.json"
    )))
    .expect("condition equality contract should be valid JSON");

    for case in cases {
        assert_eq!(
            compare_condition_values(&case.left, "==", &case.right),
            Ok(case.expected),
            "condition equality case {:?} produced an unexpected result",
            case.name
        );
        assert_eq!(
            compare_condition_values(&case.left, "!=", &case.right),
            Ok(!case.expected),
            "condition inequality case {:?} produced an unexpected result",
            case.name
        );
    }
}

#[test]
fn calculated_whole_number_matches_an_integer_condition_literal() {
    let report = execute_manual_program(
        &program(
            vec![
                json!({
                    "id": "n-calculate",
                    "action_type": "action.calculate",
                    "type": "action",
                    "config": {"expression": "3 % 2"},
                    "runtime_outputs": []
                }),
                json!({
                    "id": "n-if",
                    "action_type": "control.if",
                    "type": "if",
                    "config": {
                        "conditions": [{
                            "id": "condition-1",
                            "left": "{{n-calculate.result}}",
                            "operator": "==",
                            "right": "1",
                            "invert": false
                        }]
                    },
                    "runtime_outputs": []
                }),
                log_node("n-true", "numeric condition matched"),
                log_node("n-false", "numeric condition did not match"),
            ],
            vec![
                edge("n-trigger", "out", "n-calculate"),
                edge("n-calculate", "out", "n-if"),
                edge("n-if", "true", "n-true"),
                edge("n-if", "false", "n-false"),
            ],
        ),
        "calculated-condition-equality",
    )
    .expect("calculated numeric condition should execute");

    assert!(has_log(&report.logs, "numeric condition matched"));
    assert!(!has_log(&report.logs, "numeric condition did not match"));
}

#[test]
fn supports_every_exported_condition_operator() {
    let cases = [
        (json!("42"), "==", json!(42), true),
        (json!("42"), "!=", json!(41), true),
        (json!(10), ">", json!(9), true),
        (json!(10), ">=", json!(10), true),
        (json!(9), "<", json!(10), true),
        (json!(10), "<=", json!(10), true),
        (json!("BaudBound"), "contains", json!("Bound"), true),
        (json!("BaudBound"), "starts_with", json!("Baud"), true),
        (json!("BaudBound"), "ends_with", json!("Bound"), true),
        (
            json!("device-42"),
            "regex_match",
            json!(r"^device-\d+$"),
            true,
        ),
        (json!([]), "is_empty", Value::Null, true),
        (Value::Null, "is_null", Value::Null, true),
    ];

    for (left, operator, right, expected) in cases {
        assert_eq!(
            compare_condition_values(&left, operator, &right),
            Ok(expected),
            "operator {operator} produced an unexpected result"
        );
    }
}

#[test]
fn rejects_invalid_numeric_and_regex_conditions() {
    let numeric_error = compare_condition_values(&json!("not-a-number"), ">", &json!(1))
        .expect_err("invalid numeric comparison must fail");
    assert!(numeric_error.contains("numeric values"));

    let regex_error = compare_condition_values(&json!("value"), "regex_match", &json!("["))
        .expect_err("invalid regular expression must fail");
    assert!(regex_error.contains("invalid regex pattern"));
}

#[test]
fn inverted_condition_selects_the_opposite_branch() {
    let report = execute_manual_program(
        &program(
            vec![
                variable_node("n-status", "status", "set", "string", json!("ok")),
                json!({
                    "id": "n-if",
                    "action_type": "control.if",
                    "type": "if",
                    "config": {
                        "conditions": [{
                            "id": "condition-1",
                            "left": "{{status}}",
                            "operator": "==",
                            "right": "ok",
                            "invert": true
                        }]
                    },
                    "runtime_outputs": []
                }),
                log_node("n-true", "true branch"),
                log_node("n-false", "false branch"),
            ],
            vec![
                edge("n-trigger", "out", "n-status"),
                edge("n-status", "out", "n-if"),
                edge("n-if", "true", "n-true"),
                edge("n-if", "false", "n-false"),
            ],
        ),
        "condition-invert",
    )
    .expect("inverted condition should execute");

    assert!(!has_log(&report.logs, "true branch"));
    assert!(has_log(&report.logs, "false branch"));
}

#[test]
fn color_match_routes_both_branches_and_exposes_diagnostics() {
    let matching = execute_manual_program(
        &program(
            vec![
                color_match_node(
                    json!("rgb(250, 10, 20)"),
                    json!("#FF0A14"),
                    "per_channel",
                    json!(2),
                ),
                log_node("n-match", "matched"),
                log_node("n-no-match", "did not match"),
            ],
            vec![
                edge("n-trigger", "out", "n-color"),
                edge("n-color", "match", "n-match"),
                edge("n-color", "no_match", "n-no-match"),
            ],
        ),
        "color-match-yes",
    )
    .expect("matching colors should execute");
    assert!(has_log(&matching.logs, "matched"));
    assert!(!has_log(&matching.logs, "did not match"));
    assert_eq!(
        matching.variables.get("n-color.matches"),
        Some(&json!(true))
    );
    assert_eq!(
        matching.variables.get("n-color.red_difference"),
        Some(&json!(5))
    );

    let not_matching = execute_manual_program(
        &program(
            vec![
                color_match_node(
                    json!("#FF0000"),
                    json!("#000000"),
                    "total_distance",
                    json!(57.7),
                ),
                log_node("n-match", "matched"),
                log_node("n-no-match", "did not match"),
            ],
            vec![
                edge("n-trigger", "out", "n-color"),
                edge("n-color", "match", "n-match"),
                edge("n-color", "no_match", "n-no-match"),
            ],
        ),
        "color-match-no",
    )
    .expect("non-matching colors should execute");
    assert!(!has_log(&not_matching.logs, "matched"));
    assert!(has_log(&not_matching.logs, "did not match"));
    assert_eq!(
        not_matching.variables.get("n-color.matches"),
        Some(&json!(false))
    );
}

#[test]
fn color_match_resolves_rgb_objects_and_tolerance_variables() {
    let report = execute_manual_program(
        &program(
            vec![
                variable_node(
                    "n-sample",
                    "sample",
                    "set",
                    "object",
                    json!(r#"{"r":120,"g":80,"b":40}"#),
                ),
                variable_node("n-tolerance", "tolerance", "set", "number", json!("5")),
                color_match_node(
                    json!("{{sample}}"),
                    json!("rgb(125, 80, 40)"),
                    "per_channel",
                    json!("{{tolerance}}"),
                ),
                log_node("n-match", "object matched"),
            ],
            vec![
                edge("n-trigger", "out", "n-sample"),
                edge("n-sample", "out", "n-tolerance"),
                edge("n-tolerance", "out", "n-color"),
                edge("n-color", "match", "n-match"),
            ],
        ),
        "color-match-object",
    )
    .expect("RGB object and tolerance variables should resolve");

    assert!(has_log(&report.logs, "object matched"));
    assert_eq!(
        report.variables.get("n-color.green_difference"),
        Some(&json!(0))
    );
}

#[test]
fn color_match_rejects_invalid_resolved_values_instead_of_routing_no_match() {
    let error = execute_manual_program(
        &program(
            vec![
                variable_node("n-invalid", "sample", "set", "string", json!("not-a-color")),
                color_match_node(
                    json!("{{sample}}"),
                    json!("#000000"),
                    "per_channel",
                    json!(0),
                ),
            ],
            vec![
                edge("n-trigger", "out", "n-invalid"),
                edge("n-invalid", "out", "n-color"),
            ],
        ),
        "color-match-invalid",
    )
    .expect_err("invalid runtime colors must fail control flow");

    assert!(
        error.to_string().contains("actual color must be"),
        "{error}"
    );
}

#[test]
fn switch_without_a_matching_case_ends_the_branch() {
    let report = execute_manual_program(
        &program(
            vec![
                json!({
                    "id": "n-switch",
                    "action_type": "control.switch",
                    "type": "switch",
                    "config": {
                        "value": "missing",
                        "cases": [{"id": "ok", "name": "OK", "expectedValue": "ok"}]
                    },
                    "runtime_outputs": []
                }),
                log_node("n-case", "case ran"),
            ],
            vec![
                edge("n-trigger", "out", "n-switch"),
                edge("n-switch", "case-ok", "n-case"),
            ],
        ),
        "switch-no-match",
    )
    .expect("an unmatched switch should end cleanly");

    assert!(!has_log(&report.logs, "case ran"));
    assert!(report.logs.iter().any(|entry| {
        entry.node_id.as_deref() == Some("n-switch") && entry.message.contains("matched no case")
    }));
}

#[test]
fn while_with_false_first_condition_skips_its_body() {
    let report = execute_manual_program(
        &program(
            vec![
                json!({
                    "id": "n-while",
                    "action_type": "control.while",
                    "type": "while",
                    "config": {
                        "conditions": [{
                            "id": "condition-1",
                            "left": "0",
                            "operator": ">",
                            "right": "1"
                        }]
                    },
                    "runtime_outputs": []
                }),
                log_node("n-body", "body ran"),
                log_node("n-done", "done ran"),
            ],
            vec![
                edge("n-trigger", "out", "n-while"),
                edge("n-while", "loop", "n-body"),
                edge("n-while", "done", "n-done"),
            ],
        ),
        "while-false-first",
    )
    .expect("false-first while should execute its done branch");

    assert!(!has_log(&report.logs, "body ran"));
    assert!(has_log(&report.logs, "done ran"));
}

#[test]
fn for_each_resolves_a_nested_list_variable() {
    let report = execute_manual_program(
        &program(
            vec![
                variable_node(
                    "n-payload",
                    "payload",
                    "set",
                    "object",
                    json!(r#"{"items":["one","two"]}"#),
                ),
                for_each_node("{{payload.items}}"),
                log_node("n-item", "{{index}}={{item}}"),
                log_node("n-done", "iteration complete"),
            ],
            vec![
                edge("n-trigger", "out", "n-payload"),
                edge("n-payload", "out", "n-each"),
                edge("n-each", "loop", "n-item"),
                edge("n-each", "done", "n-done"),
            ],
        ),
        "for-each-nested",
    )
    .expect("nested list should be iterable");

    assert!(has_log(&report.logs, "0=one"));
    assert!(has_log(&report.logs, "1=two"));
    assert!(has_log(&report.logs, "iteration complete"));
}

#[test]
fn for_each_empty_list_skips_its_body() {
    let report = execute_manual_program(
        &program(
            vec![
                for_each_node("[]"),
                log_node("n-item", "body ran"),
                log_node("n-done", "done ran"),
            ],
            vec![
                edge("n-trigger", "out", "n-each"),
                edge("n-each", "loop", "n-item"),
                edge("n-each", "done", "n-done"),
            ],
        ),
        "for-each-empty",
    )
    .expect("empty list should complete cleanly");

    assert!(!has_log(&report.logs, "body ran"));
    assert!(has_log(&report.logs, "done ran"));
}

#[test]
fn for_each_rejects_every_non_list_value() {
    for items in ["plain text", r#"{"item":1}"#, "42", "true", "null"] {
        let error = execute_manual_program(
            &program(
                vec![for_each_node(items)],
                vec![edge("n-trigger", "out", "n-each")],
            ),
            "for-each-invalid",
        )
        .expect_err("non-list for-each input must fail");

        assert!(
            error.to_string().contains("must resolve to a list"),
            "unexpected error for {items:?}: {error}"
        );
    }
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

fn manual_trigger() -> Value {
    json!({
        "id": "n-trigger",
        "action_type": "trigger.manual",
        "type": "manual",
        "config": {},
        "runtime_outputs": []
    })
}

fn edge(source: &str, source_handle: &str, target: &str) -> Value {
    json!({
        "execution_order": 0,
        "source": source,
        "source_handle": source_handle,
        "target": target,
        "target_handle": "input"
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

fn variable_node(id: &str, name: &str, operation: &str, value_type: &str, value: Value) -> Value {
    json!({
        "id": id,
        "action_type": "runtime.set_variable",
        "type": "set_variable",
        "config": {
            "name": name,
            "operation": operation,
            "scope": "runtime",
            "valueType": value_type,
            "value": value
        },
        "runtime_outputs": []
    })
}

fn for_each_node(items: &str) -> Value {
    json!({
        "id": "n-each",
        "action_type": "control.for_each",
        "type": "for_each",
        "config": {
            "items": items,
            "itemVariable": "item",
            "indexVariable": "index"
        },
        "runtime_outputs": []
    })
}

fn color_match_node(
    actual_color: Value,
    expected_color: Value,
    comparison_mode: &str,
    tolerance_percent: Value,
) -> Value {
    json!({
        "id": "n-color",
        "action_type": "control.color_match",
        "type": "color_match",
        "config": {
            "actualColor": actual_color,
            "expectedColor": expected_color,
            "comparisonMode": comparison_mode,
            "tolerancePercent": tolerance_percent
        },
        "runtime_outputs": []
    })
}

fn has_log(logs: &[crate::RuntimeLogEntry], message: &str) -> bool {
    logs.iter().any(|entry| entry.message == message)
}
