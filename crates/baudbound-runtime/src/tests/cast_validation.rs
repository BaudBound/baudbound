use std::collections::BTreeMap;

use serde_json::json;

use crate::execution::cast_validation::validate_config_casts;

#[test]
fn a_failing_cast_is_reported_before_resolution() {
    let mut variables = BTreeMap::new();
    variables.insert("item".to_owned(), json!(3.7));
    let mut config = serde_json::Map::new();
    config.insert(
        "url".to_owned(),
        json!("https://example.test/{{item|integer}}"),
    );

    let error = validate_config_casts("n-http", &config, &variables)
        .expect_err("a fractional value cannot cast to integer");

    let message = error.to_string();
    assert!(message.contains("item"), "names the variable: {message}");
    assert!(message.contains("integer"), "names the target: {message}");
}

#[test]
fn an_unset_variable_reports_that_it_is_not_set() {
    let variables = BTreeMap::new();
    let mut config = serde_json::Map::new();
    config.insert("url".to_owned(), json!("{{missing|string}}"));

    let error = validate_config_casts("n-http", &config, &variables)
        .expect_err("an unset variable cannot cast");

    assert!(
        error.to_string().contains("not set"),
        "an unset variable must say so rather than reporting a null: {error}"
    );
}

#[test]
fn a_config_without_casts_passes() {
    let mut variables = BTreeMap::new();
    variables.insert("item".to_owned(), json!(42));
    let mut config = serde_json::Map::new();
    config.insert("url".to_owned(), json!("https://example.test/{{item}}"));

    validate_config_casts("n-http", &config, &variables)
        .expect("no cast means nothing to validate");
}

#[test]
fn a_failing_cast_stops_the_run_without_taking_the_failure_output() {
    // Build a two-step program where the first node has a failing cast and its
    // failed output is connected to a second node. Follow the construction the
    // neighbouring tests in tests/control_flow_matrix.rs already use.
    let report = run_program_with_failing_cast();

    assert!(
        matches!(report, Err(crate::RuntimeError::Cast { .. })),
        "a failing cast must stop the run rather than routing to the failure output"
    );
}

/// Runs a program whose only action node has a cast in its config that
/// cannot succeed (a fractional value cast to `integer`), with that node's
/// `failed` output wired to a second node. The action handler records
/// whether it was ever invoked, proving the run stops before the node does
/// anything rather than quietly continuing down the failed branch.
fn run_program_with_failing_cast() -> Result<crate::RunReport, crate::RuntimeError> {
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Debug, Default)]
    struct TrackingActionHandler(AtomicBool);

    impl crate::RuntimeActionHandler for TrackingActionHandler {
        fn execute_action(
            &self,
            _request: &crate::RuntimeActionRequest,
            _context: &crate::RuntimeContext,
        ) -> Result<crate::RuntimeActionResult, crate::RuntimeActionError> {
            self.0.store(true, Ordering::SeqCst);
            Ok(crate::RuntimeActionResult::new(serde_json::Map::new()))
        }
    }

    let handler = TrackingActionHandler::default();
    let program = json!({
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
                        "id": "n-pitch",
                        "action_type": "runtime.set_variable",
                        "type": "set_variable",
                        "config": {
                            "name": "pitch",
                            "operation": "set",
                            "scope": "runtime",
                            "valueType": "float",
                            "value": 3.7
                        },
                        "runtime_outputs": []
                    },
                    {
                        "id": "n-beep",
                        "action_type": "action.beep",
                        "type": "action",
                        "action": "beep",
                        "config": {
                            "frequencyHz": "{{pitch|integer}}",
                            "durationMs": "200"
                        },
                        "runtime_outputs": []
                    },
                    {
                        "id": "n-failed-log",
                        "action_type": "action.log",
                        "type": "action",
                        "action": "log",
                        "config": {"level": "info", "message": "took the failed branch"},
                        "runtime_outputs": []
                    }
                ],
                "edges": [
                    {"execution_order": 0, "source": "n-trigger", "source_handle": "out", "target": "n-pitch", "target_handle": "input"},
                    {"execution_order": 0, "source": "n-pitch", "source_handle": "out", "target": "n-beep", "target_handle": "input"},
                    {"execution_order": 0, "source": "n-beep", "source_handle": "failed", "target": "n-failed-log", "target_handle": "input"}
                ]
            }
        }
    });

    let result = crate::execute_manual_program_with_actions(
        &program,
        "cast-validation-no-side-effect",
        &handler,
    );
    assert!(
        !handler.0.load(Ordering::SeqCst),
        "the action handler must not run when a cast fails before resolution"
    );
    result
}
