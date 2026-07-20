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
fn executes_fixed_count_loop_body_and_done_branch() {
    let report = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-counter", "counter", "set", "number", 0),
                        {
                            "id": "n-loop",
                            "action_type": "control.loop",
                            "type": "loop",
                            "config": { "count": "3" },
                            "runtime_outputs": []
                        },
                        variable_node("n-inc", "counter", "increment", "number", 1),
                        log_node("n-done", "counter={{counter}}")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-counter"),
                        edge("n-counter", "out", "n-loop"),
                        edge("n-loop", "loop", "n-inc"),
                        edge("n-loop", "done", "n-done")
                    ]
                }
            }
        }),
        "script-1",
    )
    .expect("loop should execute");

    assert_eq!(report.variables.get("counter"), Some(&json!(3.0)));
    assert!(report.logs.iter().any(|log| log.message == "counter=3"));
}

#[test]
fn rejects_fractional_loop_count_resolved_from_a_variable() {
    let error = execute_manual_program(
        &json!({
            "entry": {
                "trigger": manual_trigger(),
                "triggers": [],
                "program": {
                    "steps": [
                        variable_node("n-count", "count", "set", "number", 1.5),
                        {
                            "id": "n-loop",
                            "action_type": "control.loop",
                            "type": "loop",
                            "config": { "count": "{{count}}" },
                            "runtime_outputs": []
                        },
                        log_node("n-body", "must not execute")
                    ],
                    "edges": [
                        edge("n-trigger", "out", "n-count"),
                        edge("n-count", "out", "n-loop"),
                        edge("n-loop", "loop", "n-body")
                    ]
                }
            }
        }),
        "script-1",
    )
    .expect_err("fractional loop count must not be truncated");

    assert!(error.to_string().contains("whole non-negative integer"));
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
                                "items": "[\"one\", \"two\"]",
                                "itemVariable": "item",
                                "indexVariable": "index"
                            },
                            "runtime_outputs": []
                        },
                        log_node("n-item", "item={{index}}:{{item}}"),
                        log_node("n-done", "done item={{item}}")
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
fn notifies_action_handler_when_a_failed_run_finishes() {
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
    let error = execute_manual_program_with_actions(
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
    .expect_err("the external action should fail");

    assert!(error.to_string().contains("expected test failure"));
    assert_eq!(handler.finished_runs.load(Ordering::SeqCst), 1);
}

#[test]
fn rejects_out_of_range_resolved_numeric_config_before_action_dispatch() {
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
    let error = execute_manual_program_with_actions(
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
    .expect_err("resolved frequency outside the contract must fail");

    assert!(error.to_string().contains("at most 20000"));
    assert!(!handler.0.load(Ordering::SeqCst));
}

#[test]
fn rejects_out_of_range_resolved_screen_coordinate_before_action_dispatch() {
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
    let error = execute_manual_program_with_actions(
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
    .expect_err("resolved coordinate outside i32 must fail");

    assert!(error.to_string().contains("at most 2147483647"));
    assert!(!handler.0.load(Ordering::SeqCst));
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
        "config": {
            "level": "info",
            "message": message
        },
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
    json!({
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
    })
}
