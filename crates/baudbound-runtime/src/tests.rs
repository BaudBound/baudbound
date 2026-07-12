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
                            {"source": "n-trigger", "source_handle": "out", "target": "n-var", "target_handle": "input"},
                            {"source": "n-var", "source_handle": "out", "target": "n-log", "target_handle": "input"}
                        ]
                    }
                }
            }),
            "script-1",
        )
        .expect("program should execute");

    assert_eq!(
        report.variables.get("foo"),
        Some(&Value::String("bar".to_owned()))
    );
    assert!(
        report
            .logs
            .iter()
            .any(|log| log.message == "foo=bar length=3")
    );
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
                            {"source": "n-trigger", "source_handle": "out", "target": "n-log", "target_handle": "input"}
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
                            {"source": "n-webhook", "source_handle": "out", "target": "n-log", "target_handle": "input"}
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
                            {"source": "n-trigger", "source_handle": "out", "target": "n-log", "target_handle": "input"}
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
                            {"source": "n-trigger", "source_handle": "out", "target": "n-var", "target_handle": "input"}
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
                            {"source": "n-trigger", "source_handle": "out", "target": "n-var", "target_handle": "input"}
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
    assert!(report.logs.iter().any(|log| log.message == "counter=3.0"));
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
            .any(|log| log.message == "while counter=3.0")
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
    assert!(report.logs.iter().any(|log| log.message == "result=10.0"));
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
