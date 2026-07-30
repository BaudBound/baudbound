use std::path::PathBuf;

use serde_json::{Map, Value, json};

use super::*;

#[path = "tests/calculation_matrix.rs"]
mod calculation_matrix;
#[path = "tests/cancellation.rs"]
mod cancellation;
#[path = "tests/control_flow_matrix.rs"]
mod control_flow_matrix;
#[path = "tests/state.rs"]
mod state;
#[path = "tests/variable_operations.rs"]
mod variable_operations;

#[test]
fn executes_manual_log_and_variable_operation() {
    let started_at_unix_ms = unix_timestamp_millis_now();
    let report = execute_manual_program(
            &json!({
                "entry": {
                    "trigger": {
                        "id": "n-trigger",
                        "action_type": "trigger.manual",
                        "type": "manual",
                        "config": {},
                        "runtime_outputs": []
                    },
                    "triggers": [],
                    "program": {
                        "type": "block",
                        "execution_model": "directed_graph",
                        "runtime_context": {
                            "expression_reference": "{{node-id.data_name}}",
                            "template_reference": "{{node-id.data_name}}",
                            "variables": [],
                            "built_in_variables": {"syntax": "{{variable_name}}", "variables": []},
                            "node_outputs": []
                        },
                        "steps": [
                            {
                                "id": "n-var",
                                "action_type": "runtime.set_variable",
                                "type": "set_variable",
                                "config": {
                                    "name": "foo",
                                    "operation": "set",
                                    "scope": "runtime",
                                    "valueType": "string",
                                    "value": "bar"
                                },
                                "runtime_outputs": []
                            },
                            {
                                "id": "n-log",
                                "action_type": "action.log",
                                "type": "action",
                                "action": "log",
                                "config": {
                                    "level": "info",
                                    "message": "foo={{foo}} length={{foo.$length}}"
                                },
                                "runtime_outputs": []
                            }
                        ],
                        "edges": [
                            {"execution_order": 0, "source": "n-trigger", "source_handle": "out", "target": "n-var", "target_handle": "input"},
                            {"execution_order": 0, "source": "n-var", "source_handle": "out", "target": "n-log", "target_handle": "input"}
                        ]
                    }
                }
            }),
            "script-1",
        )
        .expect("program should execute");
    let completed_at_unix_ms = unix_timestamp_millis_now();

    assert_eq!(
        report.variables.get("foo"),
        Some(&Value::String("bar".to_owned()))
    );
    assert_eq!(
        report.variable_scopes.get("foo"),
        Some(&RunVariableScope::Runtime)
    );
    assert_eq!(
        report.variable_scopes.get("foo.$length"),
        Some(&RunVariableScope::Metadata)
    );
    assert!(report.logs.iter().any(|log| {
        log.message == "foo=bar length=3"
            && log.node_id.as_deref() == Some("n-log")
            && log.action_type.as_deref() == Some("action.log")
    }));
    assert_eq!(
        report
            .logs
            .first()
            .and_then(|log| log.action_type.as_deref()),
        Some("trigger.manual")
    );
    assert_eq!(
        report
            .logs
            .last()
            .and_then(|log| log.action_type.as_deref()),
        None
    );
    assert!(report.logs.iter().all(|log| {
        log.timestamp_unix_ms >= started_at_unix_ms && log.timestamp_unix_ms <= completed_at_unix_ms
    }));
}

#[test]
fn rejects_active_variable_values_over_the_configured_limit() {
    let program = json!({
        "entry": {
            "trigger": manual_trigger(),
            "triggers": [],
            "program": {
                "steps": [
                    variable_node("n-var", "large", "set", "string", "x".repeat(128))
                ],
                "edges": [
                    edge("n-trigger", "out", "n-var")
                ]
            }
        }
    });
    let resources = RuntimeExecutionResources::new(&UnsupportedActionHandler).with_output_limits(
        RuntimeOutputLimits {
            max_runtime_variable_bytes: 32,
            ..RuntimeOutputLimits::default()
        },
    );

    let error = execute_manual_program_with_state(&program, "script-value-limit", resources)
        .expect_err("oversized active value should fail");

    assert!(error.to_string().contains("32 byte active value limit"));
}

#[test]
fn executes_fan_out_branches_sequentially_in_explicit_order() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        log_node("n-alpha", "second"),
                        log_node("n-zulu", "first")
                    ],
                    "edges": [
                        {
                            "execution_order": 1,
                            "source": "n-trigger",
                            "source_handle": "out",
                            "target": "n-alpha",
                            "target_handle": "input"
                        },
                        {
                            "execution_order": 0,
                            "source": "n-trigger",
                            "source_handle": "out",
                            "target": "n-zulu",
                            "target_handle": "input"
                        }
                    ]
                }
            }
        }),
        "script-ordered-fan-out",
    )
    .expect("ordered fan-out should execute");

    let branch_messages = report
        .logs
        .iter()
        .filter_map(|log| match log.message.as_str() {
            "first" | "second" => Some(log.message.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(branch_messages, vec!["first", "second"]);
}

#[test]
fn accepts_primary_trigger_also_listed_in_triggers() {
    let report = execute_manual_program(
            &json!({
                "entry": {
                    "trigger": {
                        "id": "n-trigger",
                        "action_type": "trigger.manual",
                        "type": "manual",
                        "config": {},
                        "runtime_outputs": []
                    },
                    "triggers": [
                        {
                            "id": "n-trigger",
                            "action_type": "trigger.manual",
                            "type": "manual",
                            "config": {},
                            "runtime_outputs": []
                        }
                    ],
                    "program": {
                        "type": "block",
                        "execution_model": "directed_graph",
                        "runtime_context": {
                            "expression_reference": "{{node-id.data_name}}",
                            "template_reference": "{{node-id.data_name}}",
                            "variables": [],
                            "built_in_variables": {"syntax": "{{variable_name}}", "variables": []},
                            "node_outputs": []
                        },
                        "steps": [
                            {
                                "id": "n-log",
                                "action_type": "action.log",
                                "type": "action",
                                "action": "log",
                                "config": {
                                    "level": "info",
                                    "message": "hello"
                                },
                                "runtime_outputs": []
                            }
                        ],
                        "edges": [
                            {"execution_order": 0, "source": "n-trigger", "source_handle": "out", "target": "n-log", "target_handle": "input"}
                        ]
                    }
                }
            }),
            "script-1",
        )
        .expect("duplicate primary trigger export shape should execute");

    assert!(report.logs.iter().any(|log| log.message == "hello"));
}

#[test]
fn executes_from_selected_trigger_with_payload_outputs() {
    let report = execute_trigger_program_with_actions(
            &json!({
                "entry": {
                    "trigger": {
                        "id": "n-manual",
                        "action_type": "trigger.manual",
                        "type": "manual",
                        "config": {},
                        "runtime_outputs": []
                    },
                    "triggers": [
                        {
                            "id": "n-manual",
                            "action_type": "trigger.manual",
                            "type": "manual",
                            "config": {},
                            "runtime_outputs": []
                        },
                        {
                            "id": "n-webhook",
                            "action_type": "trigger.webhook",
                            "type": "webhook",
                            "config": {"method": "POST", "hookName": "status"},
                            "runtime_outputs": []
                        }
                    ],
                    "program": {
                        "steps": [
                            {
                                "id": "n-log",
                                "action_type": "action.log",
                                "type": "action",
                                "action": "log",
                                "config": {
                                    "level": "info",
                                    "message": "webhook={{n-webhook.body}} status={{n-webhook.json.status}}"
                                },
                                "runtime_outputs": []
                            }
                        ],
                        "edges": [
                            {"execution_order": 0, "source": "n-webhook", "source_handle": "out", "target": "n-log", "target_handle": "input"}
                        ]
                    }
                }
            }),
            "script-1",
            "n-webhook",
            json!({
                "body": "ok",
                "json": {"status": "healthy"}
            }),
            &UnsupportedActionHandler,
        )
        .expect("webhook trigger run should execute");

    assert_eq!(report.identity.trigger_node_id, "n-webhook");
    assert_eq!(
        report.variables.get("n-webhook.body"),
        Some(&Value::String("ok".to_owned()))
    );
    assert!(
        report
            .logs
            .iter()
            .any(|log| log.message == "webhook=ok status=healthy")
    );
}

#[test]
fn rejects_starting_from_non_trigger_node() {
    let error = execute_trigger_program_with_actions(
            &json!({
                "entry": {
                    "trigger": {
                        "id": "n-trigger",
                        "action_type": "trigger.manual",
                        "type": "manual",
                        "config": {},
                        "runtime_outputs": []
                    },
                    "triggers": [],
                    "program": {
                        "steps": [
                            {
                                "id": "n-log",
                                "action_type": "action.log",
                                "type": "action",
                                "action": "log",
                                "config": {"level": "info", "message": "hello"},
                                "runtime_outputs": []
                            }
                        ],
                        "edges": [
                            {"execution_order": 0, "source": "n-trigger", "source_handle": "out", "target": "n-log", "target_handle": "input"}
                        ]
                    }
                }
            }),
            "script-1",
            "n-log",
            Value::Null,
            &UnsupportedActionHandler,
        )
        .expect_err("normal node should not be runnable as trigger");

    assert!(error.to_string().contains("not a registered trigger"));
}

#[test]
fn rejects_reserved_variable_writes() {
    let error = execute_manual_program(
            &json!({
                "entry": {
                    "trigger": {
                        "id": "n-trigger",
                        "action_type": "trigger.manual",
                        "type": "manual",
                        "config": {},
                        "runtime_outputs": []
                    },
                    "triggers": [],
                    "program": {
                        "steps": [
                            {
                                "id": "n-var",
                                "action_type": "runtime.set_variable",
                                "type": "set_variable",
                                "config": {
                                    "name": "system_os",
                                    "operation": "set",
                                    "scope": "runtime",
                                    "valueType": "string",
                                    "value": "bad"
                                },
                                "runtime_outputs": []
                            }
                        ],
                        "edges": [
                            {"execution_order": 0, "source": "n-trigger", "source_handle": "out", "target": "n-var", "target_handle": "input"}
                        ]
                    }
                }
            }),
            "script-1",
        )
        .expect_err("reserved variable should fail");

    assert!(error.to_string().contains("reserved"));
}

#[test]
fn rejects_derived_variable_writes() {
    let error = execute_manual_program(
            &json!({
                "entry": {
                    "trigger": {
                        "id": "n-trigger",
                        "action_type": "trigger.manual",
                        "type": "manual",
                        "config": {},
                        "runtime_outputs": []
                    },
                    "triggers": [],
                    "program": {
                        "steps": [
                            {
                                "id": "n-var",
                                "action_type": "runtime.set_variable",
                                "type": "set_variable",
                                "config": {
                                    "name": "foo.$length",
                                    "operation": "set",
                                    "scope": "runtime",
                                    "valueType": "number",
                                    "value": 10
                                },
                                "runtime_outputs": []
                            }
                        ],
                        "edges": [
                            {"execution_order": 0, "source": "n-trigger", "source_handle": "out", "target": "n-var", "target_handle": "input"}
                        ]
                    }
                }
            }),
            "script-1",
        )
        .expect_err("derived variable should fail");

    assert!(error.to_string().contains("reserved"));
}

#[test]
fn branches_with_if_else_conditions() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-status", "status", "set", "string", "ok"),
                        {
                            "id": "n-if",
                            "action_type": "control.if",
                            "type": "if",
                            "config": {
                                "conditions": [
                                    {
                                        "id": "c-1",
                                        "left": "{{status}}",
                                        "operator": "==",
                                        "right": "ok"
                                    }
                                ]
                            },
                            "runtime_outputs": []
                        },
                        log_node("n-true", "true branch"),
                        log_node("n-false", "false branch")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-status"),
                        edge("n-status", "out", "n-if"),
                        edge("n-if", "true", "n-true"),
                        edge("n-if", "false", "n-false")
                    ]
                }
            }
        }),
        "script-1",
    )
    .expect("if/else should execute");

    assert!(report.logs.iter().any(|log| log.message == "true branch"));
    assert!(!report.logs.iter().any(|log| log.message == "false branch"));
}

#[test]
fn executes_fixed_count_repeat_body_and_done_branch() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-counter", "counter", "set", "number", 0),
                        {
                            "id": "n-repeat",
                            "action_type": "control.repeat",
                            "type": "repeat",
                            "config": { "count": "3" },
                            "runtime_outputs": []
                        },
                        variable_node("n-inc", "counter", "increment", "number", 1),
                        log_node("n-done", "counter={{counter}}")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-counter"),
                        edge("n-counter", "out", "n-repeat"),
                        edge("n-repeat", "repeat", "n-inc"),
                        edge("n-repeat", "done", "n-done")
                    ]
                }
            }
        }),
        "script-1",
    )
    .expect("repeat should execute");

    assert_eq!(report.variables.get("counter"), Some(&json!(3.0)));
    assert!(report.logs.iter().any(|log| log.message == "counter=3"));
}

#[test]
fn rejects_fractional_repeat_count_resolved_from_a_variable() {
    let error = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-count", "count", "set", "number", 1.5),
                        {
                            "id": "n-repeat",
                            "action_type": "control.repeat",
                            "type": "repeat",
                            "config": { "count": "{{count}}" },
                            "runtime_outputs": []
                        },
                        log_node("n-body", "must not execute")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-count"),
                        edge("n-count", "out", "n-repeat"),
                        edge("n-repeat", "repeat", "n-body")
                    ]
                }
            }
        }),
        "script-1",
    )
    .expect_err("fractional repeat count must not be truncated");

    assert!(error.to_string().contains("whole non-negative integer"));
}

#[test]
fn continue_loop_skips_the_remaining_repeat_body() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-counter", "counter", "set", "number", 0),
                        {
                            "id": "n-repeat",
                            "action_type": "control.repeat",
                            "type": "repeat",
                            "config": { "count": "3" },
                            "runtime_outputs": []
                        },
                        variable_node("n-inc", "counter", "increment", "number", 1),
                        loop_control_node("n-continue", "control.continue_loop", "continue_loop"),
                        log_node("n-skipped", "must not execute"),
                        log_node("n-done", "counter={{counter}}")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-counter"),
                        edge("n-counter", "out", "n-repeat"),
                        edge("n-repeat", "repeat", "n-inc"),
                        ordered_edge("n-inc", "out", "n-continue", 0),
                        ordered_edge("n-inc", "out", "n-skipped", 1),
                        edge("n-repeat", "done", "n-done")
                    ]
                }
            }
        }),
        "script-1",
    )
    .expect("continue loop should advance the repeat");

    assert_eq!(report.variables.get("counter"), Some(&json!(3.0)));
    assert!(
        report
            .logs
            .iter()
            .all(|log| log.message != "must not execute")
    );
    assert!(report.logs.iter().any(|log| log.message == "counter=3"));
}

#[test]
fn break_loop_exits_repeat_and_follows_done() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-counter", "counter", "set", "number", 0),
                        {
                            "id": "n-repeat",
                            "action_type": "control.repeat",
                            "type": "repeat",
                            "config": { "count": "10" },
                            "runtime_outputs": []
                        },
                        variable_node("n-inc", "counter", "increment", "number", 1),
                        {
                            "id": "n-if",
                            "action_type": "control.if",
                            "type": "if",
                            "config": {
                                "conditions": [{
                                    "id": "c-1",
                                    "left": "{{counter}}",
                                    "operator": "==",
                                    "right": "2"
                                }]
                            },
                            "runtime_outputs": []
                        },
                        loop_control_node("n-break", "control.break_loop", "break_loop"),
                        log_node("n-done", "counter={{counter}}")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-counter"),
                        edge("n-counter", "out", "n-repeat"),
                        edge("n-repeat", "repeat", "n-inc"),
                        edge("n-inc", "out", "n-if"),
                        edge("n-if", "true", "n-break"),
                        edge("n-repeat", "done", "n-done")
                    ]
                }
            }
        }),
        "script-1",
    )
    .expect("break loop should exit the repeat");

    assert_eq!(report.variables.get("counter"), Some(&json!(2.0)));
    assert!(report.logs.iter().any(|log| log.message == "counter=2"));
}

#[test]
fn break_loop_only_exits_the_innermost_loop() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-outer-count", "outer_count", "set", "number", 0),
                        variable_node("n-inner-count", "inner_count", "set", "number", 0),
                        {
                            "id": "n-outer",
                            "action_type": "control.repeat",
                            "type": "repeat",
                            "config": { "count": "2" },
                            "runtime_outputs": []
                        },
                        {
                            "id": "n-inner",
                            "action_type": "control.repeat",
                            "type": "repeat",
                            "config": { "count": "3" },
                            "runtime_outputs": []
                        },
                        variable_node("n-inner-inc", "inner_count", "increment", "number", 1),
                        loop_control_node("n-break", "control.break_loop", "break_loop"),
                        variable_node("n-outer-inc", "outer_count", "increment", "number", 1),
                        log_node(
                            "n-done",
                            "outer={{outer_count}}, inner={{inner_count}}"
                        )
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-outer-count"),
                        edge("n-outer-count", "out", "n-inner-count"),
                        edge("n-inner-count", "out", "n-outer"),
                        edge("n-outer", "repeat", "n-inner"),
                        edge("n-inner", "repeat", "n-inner-inc"),
                        edge("n-inner-inc", "out", "n-break"),
                        edge("n-inner", "done", "n-outer-inc"),
                        edge("n-outer", "done", "n-done")
                    ]
                }
            }
        }),
        "script-1",
    )
    .expect("nested break loop should execute");

    assert_eq!(report.variables.get("outer_count"), Some(&json!(2.0)));
    assert_eq!(report.variables.get("inner_count"), Some(&json!(2.0)));
    assert!(
        report
            .logs
            .iter()
            .any(|log| log.message == "outer=2, inner=2")
    );
}

#[test]
fn rejects_loop_control_outside_an_active_loop() {
    for (action_type, node_type) in [
        ("control.break_loop", "break_loop"),
        ("control.continue_loop", "continue_loop"),
    ] {
        let error = execute_manual_program(
            &json!({
                "entry": {
                    "trigger": manual_trigger(),
                    "triggers": [],
                    "program": {
                        "steps": [
                            loop_control_node("n-control", action_type, node_type)
                        ],
                        "edges": [
                            edge("n-trigger", "out", "n-control")
                        ]
                    }
                }
            }),
            "script-1",
        )
        .expect_err("loop control outside a loop must fail");

        assert!(
            error
                .to_string()
                .contains("must run inside a Repeat, While, or For Each loop")
        );
    }
}

#[test]
fn executes_while_until_condition_fails() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-counter", "counter", "set", "number", 0),
                        {
                            "id": "n-while",
                            "action_type": "control.while",
                            "type": "while",
                            "config": {
                                "conditions": [
                                    {
                                        "id": "c-1",
                                        "left": "{{counter}}",
                                        "operator": "<",
                                        "right": "3"
                                    }
                                ]
                            },
                            "runtime_outputs": []
                        },
                        variable_node("n-inc", "counter", "increment", "number", 1),
                        log_node("n-done", "while counter={{counter}}")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-counter"),
                        edge("n-counter", "out", "n-while"),
                        edge("n-while", "loop", "n-inc"),
                        edge("n-while", "done", "n-done")
                    ]
                }
            }
        }),
        "script-1",
    )
    .expect("while should execute");

    assert_eq!(report.variables.get("counter"), Some(&json!(3.0)));
    assert!(
        report
            .logs
            .iter()
            .any(|log| log.message == "while counter=3")
    );
}

#[test]
fn executes_for_each_over_json_list() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        {
                            "id": "n-each",
                            "action_type": "control.for_each",
                            "type": "for_each",
                            "config": {
                                "items": "[\"one\", \"two\"]"
                            },
                            "runtime_outputs": []
                        },
                        log_node("n-item", "item={{n-each.index}}:{{n-each.item}}"),
                        log_node("n-done", "done item={{n-each.item}}")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-each"),
                        edge("n-each", "loop", "n-item"),
                        edge("n-each", "done", "n-done")
                    ]
                }
            }
        }),
        "script-1",
    )
    .expect("for-each should execute");

    assert!(report.logs.iter().any(|log| log.message == "item=0:one"));
    assert!(report.logs.iter().any(|log| log.message == "item=1:two"));
    assert!(report.logs.iter().any(|log| log.message == "done item=two"));
    assert_eq!(report.variables.get("n-each.item"), Some(&json!("two")));
    assert_eq!(report.variables.get("n-each.index"), Some(&json!(1)));
}

#[test]
fn switches_to_matching_case_handle() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-status", "status", "set", "string", "warning"),
                        {
                            "id": "n-switch",
                            "action_type": "control.switch",
                            "type": "switch",
                            "config": {
                                "value": "{{status}}",
                                "cases": [
                                    { "id": "ok", "name": "ok", "value": "ok" },
                                    { "id": "warning", "name": "warning", "value": "warning" }
                                ]
                            },
                            "runtime_outputs": []
                        },
                        log_node("n-ok", "ok branch"),
                        log_node("n-warning", "warning branch")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-status"),
                        edge("n-status", "out", "n-switch"),
                        edge("n-switch", "case-ok", "n-ok"),
                        edge("n-switch", "case-warning", "n-warning")
                    ]
                }
            }
        }),
        "script-1",
    )
    .expect("switch should execute");

    assert!(!report.logs.iter().any(|log| log.message == "ok branch"));
    assert!(
        report
            .logs
            .iter()
            .any(|log| log.message == "warning branch")
    );
}

#[test]
fn calculates_expression_and_exposes_result_reference() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        {
                            "id": "n-calc",
                            "action_type": "action.calculate",
                            "type": "action",
                            "action": "calculate",
                            "config": {
                                "expression": "round(2.4) + 2 ^ 3"
                            },
                            "runtime_outputs": []
                        },
                        log_node("n-log", "result={{n-calc.result}}")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-calc"),
                        edge("n-calc", "out", "n-log")
                    ]
                }
            }
        }),
        "script-1",
    )
    .expect("calculate should execute");

    assert_eq!(report.variables.get("n-calc.result"), Some(&json!(10.0)));
    assert_eq!(
        report.variable_scopes.get("n-calc.result"),
        Some(&RunVariableScope::NodeOutput)
    );
    assert!(report.logs.iter().any(|log| log.message == "result=10"));
}

#[test]
fn executes_external_action_handler_and_exposes_output_reference() {
    #[derive(Debug)]
    struct EchoActionHandler;

    impl RuntimeActionHandler for EchoActionHandler {
        fn execute_action(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<RuntimeActionResult, RuntimeActionError> {
            assert_eq!(request.action_type, "action.text.format");
            Ok(RuntimeActionResult {
                output_data: Map::from_iter([(
                    "text".to_owned(),
                    request
                        .config
                        .get("input")
                        .cloned()
                        .unwrap_or_else(|| Value::String("missing".to_owned())),
                )]),
            })
        }
    }

    let report = execute_manual_program_with_actions(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-var", "item", "set", "string", "hello"),
                        {
                            "id": "n-format",
                            "action_type": "action.text.format",
                            "type": "action",
                            "action": "format_text",
                            "config": {
                                "operation": "uppercase",
                                "input": "{{item}}"
                            },
                            "runtime_outputs": []
                        },
                        log_node("n-log", "external={{n-format.text}}")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-var"),
                        edge("n-var", "out", "n-format"),
                        edge("n-format", "out", "n-log")
                    ]
                }
            }
        }),
        "script-1",
        &EchoActionHandler,
    )
    .expect("external action should execute");

    assert_eq!(report.variables.get("n-format.text"), Some(&json!("hello")));
    assert!(
        report
            .logs
            .iter()
            .any(|log| log.message == "external=hello")
    );
}

#[test]
fn webhook_response_requires_a_waiting_webhook_request() {
    #[derive(Debug)]
    struct UnexpectedActionHandler;

    impl RuntimeActionHandler for UnexpectedActionHandler {
        fn execute_action(
            &self,
            _request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<RuntimeActionResult, RuntimeActionError> {
            panic!("webhook response handler must not run without a waiting request");
        }
    }

    let report = execute_trigger_program_with_actions(
        &json!({
            "entry": {
                "trigger": {
                    "id": "n-trigger",
                    "action_type": "trigger.webhook",
                    "type": "webhook",
                    "config": {},
                    "runtime_outputs": []
                },
                "triggers": [],
                "program": {
                    "steps": [{
                        "id": "n-response",
                        "action_type": "action.webhook_response",
                        "type": "action",
                            "action": "webhook_response",
                        "config": {
                            "statusCode": 200,
                            "contentType": "text/plain",
                            "headers": [],
                            "body": "ok"
                        },
                        "runtime_outputs": []
                    }],
                    "edges": [
                        edge("n-trigger", "out", "n-response")
                    ]
                }
            }
        }),
        "script-webhook",
        "n-trigger",
        json!({"response": {"waiting": false}, "trigger_id": "n-trigger"}),
        &UnexpectedActionHandler,
    )
    .expect("fallible webhook response should follow its failed branch");

    assert_eq!(
        report.variables["n-response.error"]["message"],
        json!("Webhook Response reached without a waiting webhook request.")
    );
}

#[test]
fn webhook_response_can_only_be_sent_once_per_run() {
    #[derive(Debug)]
    struct WebhookResponseHandler;

    impl RuntimeActionHandler for WebhookResponseHandler {
        fn execute_action(
            &self,
            _request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<RuntimeActionResult, RuntimeActionError> {
            Ok(RuntimeActionResult {
                output_data: Map::from_iter([
                    ("sent".to_owned(), json!(true)),
                    ("trigger_id".to_owned(), json!("n-trigger")),
                    ("status_code".to_owned(), json!(200)),
                ]),
            })
        }
    }

    let response_node = |id: &str| {
        json!({
            "id": id,
            "action_type": "action.webhook_response",
            "type": "action",
            "action": "webhook_response",
            "config": {
                "statusCode": 200,
                "contentType": "text/plain",
                "headers": [],
                "body": "ok"
            },
            "runtime_outputs": []
        })
    };
    let report = execute_trigger_program_with_actions(
        &json!({
            "entry": {
                "trigger": {
                    "id": "n-trigger",
                    "action_type": "trigger.webhook",
                    "type": "webhook",
                    "config": {},
                    "runtime_outputs": []
                },
                "triggers": [],
                "program": {
                    "steps": [
                        response_node("n-response-1"),
                        response_node("n-response-2")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-response-1"),
                        edge("n-response-1", "out", "n-response-2")
                    ]
                }
            }
        }),
        "script-webhook",
        "n-trigger",
        json!({"response": {"waiting": true}, "trigger_id": "n-trigger"}),
        &WebhookResponseHandler,
    )
    .expect("second webhook response should follow its failed branch");

    assert_eq!(report.variables["n-response-1.sent"], json!(true));
    assert_eq!(
        report.variables["n-response-2.error"]["message"],
        json!("Webhook response was already sent for this request.")
    );
}

#[test]
fn resolves_bracket_paths_and_nested_external_action_inputs() {
    #[derive(Debug)]
    struct NestedInputActionHandler;

    impl RuntimeActionHandler for NestedInputActionHandler {
        fn execute_action(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<RuntimeActionResult, RuntimeActionError> {
            assert_eq!(request.config.get("input"), Some(&json!("Ada")));
            assert_eq!(
                request.config.get("operations"),
                Some(&json!([
                    {
                        "operation": "replace",
                        "search": "Ada",
                        "replacement": "admin"
                    }
                ]))
            );
            Ok(RuntimeActionResult {
                output_data: Map::from_iter([("text".to_owned(), json!("admin"))]),
            })
        }
    }

    let report = execute_manual_program_with_actions(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node(
                            "n-records",
                            "records",
                            "set",
                            "list",
                            r#"[{"name":"Ada","meta":{"role-name":"admin"}}]"#
                        ),
                        {
                            "id": "n-format",
                            "action_type": "action.text.format",
                            "type": "action",
                            "action": "format_text",
                            "config": {
                                "input": "{{records[0].name}}",
                                "operations": [{
                                    "operation": "replace",
                                    "search": "{{records[0].name}}",
                                    "replacement": "{{records[0].meta[\"role-name\"]}}"
                                }]
                            },
                            "runtime_outputs": []
                        }
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-records"),
                        edge("n-records", "out", "n-format")
                    ]
                }
            }
        }),
        "script-nested-inputs",
        &NestedInputActionHandler,
    )
    .expect("all nested action inputs should resolve");

    assert!(
        report.logs.iter().any(|log| {
            log.message == r#"Applied 1 text transform operation. Result: "admin"."#
        })
    );
}

#[test]
fn dispatches_json_http_bodies_with_safely_serialized_variables() {
    #[derive(Debug)]
    struct JsonBodyActionHandler;

    impl RuntimeActionHandler for JsonBodyActionHandler {
        fn execute_action(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<RuntimeActionResult, RuntimeActionError> {
            assert_eq!(request.action_type, "action.http");
            let body = request
                .config
                .get("body")
                .and_then(Value::as_str)
                .expect("HTTP body should be resolved to text");
            assert_eq!(
                serde_json::from_str::<Value>(body).expect("HTTP body should remain valid JSON"),
                json!({"data": "scanner value\r"})
            );
            Ok(RuntimeActionResult {
                output_data: Map::from_iter([
                    ("status_code".to_owned(), json!(422)),
                    ("status_text".to_owned(), json!("Unprocessable Entity")),
                    ("duration_ms".to_owned(), json!(15)),
                    ("body".to_owned(), json!("response\r\nbody")),
                ]),
            })
        }
    }

    let report = execute_manual_program_with_actions(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-var", "scanner_data", "set", "string", "scanner value\r"),
                        {
                            "id": "n-http",
                            "action_type": "action.http",
                            "type": "action",
                            "action": "http_request",
                            "config": {
                                "method": "POST",
                                "url": "https://example.com",
                                "timeoutSeconds": "30",
                                "headers": [{"name": "Content-Type", "value": "application/json"}],
                                "bodyFormat": "json",
                                "body": "{\"data\":\"{{scanner_data}}\"}"
                            },
                            "runtime_outputs": []
                        }
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-var"),
                        edge("n-var", "out", "n-http")
                    ]
                }
            }
        }),
        "script-json-http-body",
        &JsonBodyActionHandler,
    )
    .expect("JSON HTTP body should reach the action handler safely");

    assert!(
        report.logs.iter().any(|log| {
            log.level == "debug"
                && log.message.contains("HTTP request body: 26 bytes, sha256 ")
                && log
                    .message
                    .contains("preview: {\"data\":\"scanner value\\r\"}")
        }),
        "logs were {:#?}",
        report.logs
    );
    assert!(report.logs.iter().any(|log| {
        log.message
            .contains("returned 422 Unprocessable Entity in 15 ms")
    }));
    assert!(report.logs.iter().any(|log| {
        log.level == "debug"
            && log
                .message
                .contains("HTTP response body: 14 bytes, sha256 ")
            && log.message.ends_with("preview: response\\r\\nbody")
    }));
}

#[test]
fn notifies_action_handler_when_an_expected_action_failure_finishes() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Default)]
    struct LifecycleActionHandler {
        finished_runs: AtomicUsize,
    }

    impl RuntimeActionHandler for LifecycleActionHandler {
        fn execute_action(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<RuntimeActionResult, RuntimeActionError> {
            Err(RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: "expected test failure".to_owned(),
            })
        }

        fn run_finished(&self, identity: &RunIdentity) {
            assert_eq!(identity.script_id, "script-lifecycle");
            self.finished_runs.fetch_add(1, Ordering::SeqCst);
        }
    }

    let handler = LifecycleActionHandler::default();
    let report = execute_manual_program_with_actions(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [{
                        "id": "n-external",
                        "action_type": "action.text.format",
                        "type": "action",
                        "action": "format_text",
                        "config": { "operation": "uppercase", "input": "test" },
                        "runtime_outputs": []
                    }],
                    "edges": [edge("n-trigger", "out", "n-external")]
                }
            }
        }),
        "script-lifecycle",
        &handler,
    )
    .expect("the expected action failure should complete through the failed outcome");

    assert!(report.logs.iter().any(|log| {
        log.level == "error"
            && log.node_id.as_deref() == Some("n-external")
            && log.message.contains("expected test failure")
    }));
    assert_eq!(
        report
            .variables
            .get("n-external.error")
            .and_then(Value::as_object)
            .and_then(|error| error.get("message")),
        Some(&json!("expected test failure"))
    );
    assert_eq!(handler.finished_runs.load(Ordering::SeqCst), 1);
}

#[test]
fn routes_out_of_range_resolved_numeric_config_to_failed_before_action_dispatch() {
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Debug)]
    struct TrackingActionHandler(AtomicBool);

    impl RuntimeActionHandler for TrackingActionHandler {
        fn execute_action(
            &self,
            _request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<RuntimeActionResult, RuntimeActionError> {
            self.0.store(true, Ordering::SeqCst);
            Ok(RuntimeActionResult {
                output_data: Map::new(),
            })
        }
    }

    let handler = TrackingActionHandler(AtomicBool::new(false));
    let report = execute_manual_program_with_actions(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-frequency", "frequency", "set", "number", 25_000),
                        {
                            "id": "n-beep",
                            "action_type": "action.beep",
                            "type": "action",
                            "action": "beep",
                            "config": {
                                "frequencyHz": "{{frequency}}",
                                "durationMs": "200"
                            },
                            "runtime_outputs": []
                        }
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-frequency"),
                        edge("n-frequency", "out", "n-beep")
                    ]
                }
            }
        }),
        "script-1",
        &handler,
    )
    .expect("resolved frequency outside the contract should use the failed outcome");

    assert!(
        report
            .logs
            .iter()
            .any(|log| { log.level == "error" && log.message.contains("at most 20000") })
    );
    assert!(!handler.0.load(Ordering::SeqCst));
}

#[test]
fn routes_out_of_range_resolved_screen_coordinate_to_failed_before_action_dispatch() {
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Debug)]
    struct TrackingActionHandler(AtomicBool);

    impl RuntimeActionHandler for TrackingActionHandler {
        fn execute_action(
            &self,
            _request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<RuntimeActionResult, RuntimeActionError> {
            self.0.store(true, Ordering::SeqCst);
            Ok(RuntimeActionResult {
                output_data: Map::new(),
            })
        }
    }

    let handler = TrackingActionHandler(AtomicBool::new(false));
    let report = execute_manual_program_with_actions(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-x", "screen_x", "set", "string", "2147483648"),
                        {
                            "id": "n-pixel",
                            "action_type": "action.pixel.get",
                            "type": "action",
                            "action": "get_pixel_color",
                            "config": {
                                "x": "{{screen_x}}",
                                "y": "-120"
                            },
                            "runtime_outputs": []
                        }
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-x"),
                        edge("n-x", "out", "n-pixel")
                    ]
                }
            }
        }),
        "script-1",
        &handler,
    )
    .expect("resolved coordinate outside i32 should use the failed outcome");

    assert!(
        report
            .logs
            .iter()
            .any(|log| { log.level == "error" && log.message.contains("at most 2147483647") })
    );
    assert!(!handler.0.load(Ordering::SeqCst));
}

#[test]
fn expected_action_failures_follow_failed_edges_with_structured_error_data() {
    #[derive(Debug)]
    struct FailingActionHandler;

    impl RuntimeActionHandler for FailingActionHandler {
        fn execute_action(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<RuntimeActionResult, RuntimeActionError> {
            Err(RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: "device was unavailable".to_owned(),
            })
        }
    }

    let report = execute_manual_program_with_actions(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        {
                            "id": "n-action",
                            "action_type": "action.beep",
                            "type": "action",
                            "action": "beep",
                            "config": {"frequencyHz": "800", "durationMs": "200"},
                            "runtime_outputs": []
                        },
                        log_node("n-failed", "failure={{n-action.error.message}} code={{n-action.error.code}}")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-action"),
                        edge("n-action", "failed", "n-failed")
                    ]
                }
            }
        }),
        "script-failed-edge",
        &FailingActionHandler,
    )
    .expect("expected action failure should follow its failed edge");

    assert!(
        report
            .logs
            .iter()
            .any(|log| { log.message == "failure=device was unavailable code=ACTION_FAILED" })
    );
    let error = report
        .variables
        .get("n-action.error")
        .and_then(Value::as_object)
        .expect("structured error should be stored");
    assert_eq!(error.get("message"), Some(&json!("device was unavailable")));
    assert_eq!(error.get("code"), Some(&json!("ACTION_FAILED")));
    assert_eq!(error.get("type"), Some(&json!("runtime")));
    assert_eq!(error.get("retryable"), Some(&json!(false)));
    assert_eq!(
        error
            .get("details")
            .and_then(Value::as_object)
            .and_then(|details| details.get("action_type")),
        Some(&json!("action.beep"))
    );
}

#[test]
fn successful_actions_never_fall_back_to_a_connected_failed_edge() {
    #[derive(Debug)]
    struct SuccessfulActionHandler;

    impl RuntimeActionHandler for SuccessfulActionHandler {
        fn execute_action(
            &self,
            _request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<RuntimeActionResult, RuntimeActionError> {
            Ok(RuntimeActionResult {
                output_data: Map::new(),
            })
        }
    }

    let report = execute_manual_program_with_actions(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        {
                            "id": "n-action",
                            "action_type": "action.beep",
                            "type": "action",
                            "action": "beep",
                            "config": {"frequencyHz": "800", "durationMs": "200"},
                            "runtime_outputs": []
                        },
                        log_node("n-failed", "must not execute")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-action"),
                        edge("n-action", "failed", "n-failed")
                    ]
                }
            }
        }),
        "script-success-with-failed-edge",
        &SuccessfulActionHandler,
    )
    .expect("successful action should complete");

    assert!(
        report
            .logs
            .iter()
            .all(|log| log.message != "must not execute")
    );
    assert!(!report.variables.contains_key("n-action.error"));
}

#[test]
fn calculation_failures_follow_failed_edges() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        {
                            "id": "n-calculate",
                            "action_type": "action.calculate",
                            "type": "action",
                            "action": "calculate",
                            "config": {"expression": "("},
                            "runtime_outputs": []
                        },
                        log_node(
                            "n-failed",
                            "calculation={{n-calculate.error.code}} type={{n-calculate.error.type}}"
                        )
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-calculate"),
                        edge("n-calculate", "failed", "n-failed")
                    ]
                }
            }
        }),
        "script-calculation-failed-edge",
    )
    .expect("calculation failure should follow its failed edge");

    assert!(
        report
            .logs
            .iter()
            .any(|log| { log.message == "calculation=CALCULATION_FAILED type=validation" })
    );
}

#[test]
fn delay_failures_follow_failed_edges() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        {
                            "id": "n-delay",
                            "action_type": "action.delay",
                            "type": "action",
                            "action": "delay",
                            "config": {"amount": "not-a-duration", "unit": "seconds"},
                            "runtime_outputs": []
                        },
                        log_node("n-failed", "delay={{n-delay.error.code}}")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-delay"),
                        edge("n-delay", "failed", "n-failed")
                    ]
                }
            }
        }),
        "script-delay-failed-edge",
    )
    .expect("delay failure should follow its failed edge");

    assert!(
        report
            .logs
            .iter()
            .any(|log| { log.message == "delay=DELAY_DURATION_INVALID" })
    );
}

#[test]
fn variable_operation_failures_follow_failed_edges() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node(
                            "n-variable",
                            "count",
                            "increment",
                            "number",
                            "not-a-number"
                        ),
                        log_node("n-failed", "variable={{n-variable.error.code}}")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-variable"),
                        edge("n-variable", "failed", "n-failed")
                    ]
                }
            }
        }),
        "script-variable-failed-edge",
    )
    .expect("variable operation failure should follow its failed edge");

    assert!(
        report
            .logs
            .iter()
            .any(|log| { log.message == "variable=VARIABLE_OPERATION_FAILED" })
    );
    assert!(!report.variables.contains_key("count"));
}

#[test]
fn preserves_negative_screen_coordinates_through_runtime_dispatch() {
    use std::sync::Mutex;

    #[derive(Debug, Default)]
    struct RecordingActionHandler(Mutex<Vec<RuntimeActionRequest>>);

    impl RuntimeActionHandler for RecordingActionHandler {
        fn execute_action(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<RuntimeActionResult, RuntimeActionError> {
            self.0
                .lock()
                .expect("request recorder should lock")
                .push(request.clone());
            Ok(RuntimeActionResult {
                output_data: Map::new(),
            })
        }
    }

    let handler = RecordingActionHandler::default();
    execute_manual_program_with_actions(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        {
                            "id": "n-pixel",
                            "action_type": "action.pixel.get",
                            "type": "action",
                            "action": "get_pixel_color",
                            "config": { "x": "-1920", "y": "-120" },
                            "runtime_outputs": []
                        },
                        {
                            "id": "n-mouse",
                            "action_type": "action.mouse.move",
                            "type": "action",
                            "action": "move_mouse",
                            "config": { "relative": false, "x": "-1600", "y": "-80" },
                            "runtime_outputs": []
                        }
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-pixel"),
                        edge("n-pixel", "success", "n-mouse")
                    ]
                }
            }
        }),
        "script-negative-coordinates",
        &handler,
    )
    .expect("negative signed coordinates should reach the action dispatcher");

    let requests = handler.0.lock().expect("request recorder should lock");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].action_type, "action.pixel.get");
    assert_eq!(requests[0].config.get("x"), Some(&json!("-1920")));
    assert_eq!(requests[0].config.get("y"), Some(&json!("-120")));
    assert_eq!(requests[1].action_type, "action.mouse.move");
    assert_eq!(requests[1].config.get("x"), Some(&json!("-1600")));
    assert_eq!(requests[1].config.get("y"), Some(&json!("-80")));
    assert_eq!(
        requests[1].config.get("relative"),
        Some(&Value::Bool(false))
    );
}

#[test]
fn passes_package_path_to_action_handler() {
    #[derive(Debug)]
    struct PackagePathActionHandler;

    impl RuntimeActionHandler for PackagePathActionHandler {
        fn execute_action(
            &self,
            _request: &RuntimeActionRequest,
            context: &RuntimeContext,
        ) -> Result<RuntimeActionResult, RuntimeActionError> {
            Ok(RuntimeActionResult {
                output_data: Map::from_iter([(
                    "package_path".to_owned(),
                    Value::String(
                        context
                            .package_path
                            .as_ref()
                            .expect("package path should be available")
                            .display()
                            .to_string(),
                    ),
                )]),
            })
        }
    }

    let package_path = PathBuf::from("installed-script.bbs");
    let report = execute_manual_program_with_actions_and_package_path(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        {
                            "id": "n-action",
                            "action_type": "action.sound.play",
                            "type": "action",
                            "action": "play_sound",
                            "config": {
                                "source": "asset",
                                "assetPath": "assets/sounds/beep.wav"
                            },
                            "runtime_outputs": []
                        }
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-action")
                    ]
                }
            }
        }),
        "script-1",
        Some(package_path.clone()),
        &PackagePathActionHandler,
    )
    .expect("action should receive package path");

    assert_eq!(
        report.variables.get("n-action.package_path"),
        Some(&Value::String(package_path.display().to_string()))
    );
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
    ordered_edge(source, source_handle, target, 0)
}

fn ordered_edge(source: &str, source_handle: &str, target: &str, execution_order: u32) -> Value {
    json!({
        "execution_order": execution_order,
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
        "config": {
            "level": "info",
            "message": message
        },
        "runtime_outputs": []
    })
}

fn loop_control_node(id: &str, action_type: &str, node_type: &str) -> Value {
    json!({
        "id": id,
        "action_type": action_type,
        "type": node_type,
        "config": {},
        "runtime_outputs": []
    })
}

fn variable_node(
    id: &str,
    name: &str,
    operation: &str,
    value_type: &str,
    value: impl Into<Value>,
) -> Value {
    let mut node = json!({
        "id": id,
        "action_type": "runtime.set_variable",
        "type": "set_variable",
        "config": {
            "name": name,
            "operation": operation,
            "scope": "runtime",
            "valueType": value_type,
            "value": value.into()
        },
        "runtime_outputs": []
    });
    if value_type == "list" {
        node["config"]["itemType"] = json!("object");
    }
    node
}
