use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use serde_json::{Value, json};

use crate::{
    RunIdentity, RunVariableScope, RuntimeActionError, RuntimeActionHandler, RuntimeActionRequest,
    RuntimeActionResult, RuntimeCancellationToken, RuntimeContext, RuntimeDefaultVariable,
    RuntimeDefaultVariableScope, RuntimeExecutionResources, RuntimeLogEntry, RuntimeRunObserver,
    RuntimeScriptSettings, RuntimeSecretDeclaration, RuntimeStateStore, RuntimeVariableScope,
    UnsupportedActionHandler, VersionedRuntimeVariable, execute_manual_program_with_state,
};

#[derive(Default)]
struct TestStateStore {
    secrets: Mutex<BTreeMap<(String, String), Value>>,
    variables: Mutex<BTreeMap<(RuntimeVariableScopeKey, String, String), VersionedRuntimeVariable>>,
}

#[derive(Default)]
struct LogObserver {
    logs: Mutex<Vec<RuntimeLogEntry>>,
}

impl RuntimeRunObserver for LogObserver {
    fn run_started(&self, _identity: &RunIdentity, _cancellation: RuntimeCancellationToken) {}

    fn log_emitted(&self, _identity: &RunIdentity, entry: &RuntimeLogEntry) {
        self.logs
            .lock()
            .expect("observer log lock should work")
            .push(entry.clone());
    }

    fn run_finished(&self, _identity: &RunIdentity) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RuntimeVariableScopeKey {
    Persistent,
    Global,
}

impl RuntimeStateStore for TestStateStore {
    fn load_variable(
        &self,
        scope: RuntimeVariableScope,
        script_id: &str,
        name: &str,
    ) -> Result<Option<VersionedRuntimeVariable>, String> {
        Ok(self
            .variables
            .lock()
            .map_err(|_| "test variable lock poisoned".to_owned())?
            .get(&(scope.into(), script_id.to_owned(), name.to_owned()))
            .cloned())
    }

    fn compare_and_set_variable(
        &self,
        scope: RuntimeVariableScope,
        script_id: &str,
        name: &str,
        expected_version: Option<u64>,
        value: &Value,
    ) -> Result<bool, String> {
        let key = (scope.into(), script_id.to_owned(), name.to_owned());
        let mut variables = self
            .variables
            .lock()
            .map_err(|_| "test variable lock poisoned".to_owned())?;
        match (variables.get(&key), expected_version) {
            (None, None) => {
                variables.insert(
                    key,
                    VersionedRuntimeVariable {
                        value: value.clone(),
                        version: 1,
                    },
                );
                Ok(true)
            }
            (Some(current), Some(expected)) if current.version == expected => {
                variables.insert(
                    key,
                    VersionedRuntimeVariable {
                        value: value.clone(),
                        version: expected + 1,
                    },
                );
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn delete_variable(
        &self,
        scope: RuntimeVariableScope,
        script_id: &str,
        name: &str,
    ) -> Result<bool, String> {
        Ok(self
            .variables
            .lock()
            .map_err(|_| "test variable lock poisoned".to_owned())?
            .remove(&(scope.into(), script_id.to_owned(), name.to_owned()))
            .is_some())
    }

    fn read_secret(&self, script_id: &str, name: &str) -> Result<Option<Value>, String> {
        Ok(self
            .secrets
            .lock()
            .map_err(|_| "test secret lock poisoned".to_owned())?
            .get(&(script_id.to_owned(), name.to_owned()))
            .cloned())
    }
}

impl From<RuntimeVariableScope> for RuntimeVariableScopeKey {
    fn from(value: RuntimeVariableScope) -> Self {
        match value {
            RuntimeVariableScope::Persistent => Self::Persistent,
            RuntimeVariableScope::Global => Self::Global,
        }
    }
}

#[derive(Debug)]
struct SensitiveFormDialogHandler;

impl RuntimeActionHandler for SensitiveFormDialogHandler {
    fn execute_action(
        &self,
        _request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        Ok(RuntimeActionResult::new(serde_json::Map::from_iter([
            (
                "values".to_owned(),
                json!({"password":"must-not-persist","username":"Ada"}),
            ),
            ("submitted".to_owned(), json!(true)),
        ]))
        .with_sensitive_output_path("values", ["password"]))
    }
}

#[test]
fn sensitive_action_outputs_cannot_be_written_to_persistent_or_global_state() {
    for scope in ["persistent", "global"] {
        let store = TestStateStore::default();
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
                            "id": "n-form-dialog",
                            "action_type": "action.form_dialog",
                            "type": "action",
                            "action": "form_dialog",
                            "config": {"title": "Credentials", "fields": []},
                            "runtime_outputs": []
                        },
                        {
                            "id": "n-user-log",
                            "action_type": "action.log",
                            "type": "action",
                            "config": {"level": "info", "message": "username={{n-form-dialog.values.username}}"},
                            "runtime_outputs": []
                        },
                        {
                            "id": "n-store",
                            "action_type": "runtime.set_variable",
                            "type": "set_variable",
                            "config": {
                                "name": "stored_password",
                                "operation": "set",
                                "scope": scope,
                                "valueType": "string",
                                "value": "{{n-form-dialog.values.password}}"
                            },
                            "runtime_outputs": []
                        }
                    ],
                    "edges": [
                        {"execution_order": 0, "source": "n-trigger", "source_handle": "out", "target": "n-form-dialog", "target_handle": "input"},
                        {"execution_order": 0, "source": "n-form-dialog", "source_handle": "success", "target": "n-user-log", "target_handle": "input"},
                        {"execution_order": 0, "source": "n-user-log", "source_handle": "out", "target": "n-store", "target_handle": "input"}
                    ]
                }
            }
        });
        let resources =
            RuntimeExecutionResources::new(&SensitiveFormDialogHandler).with_state(&store, &[]);

        let report =
            execute_manual_program_with_state(&program, "script-sensitive-state", resources)
                .expect("a fallible state write should complete through its failed output");

        assert!(
            report
                .variables
                .get("n-store.error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("sensitive form dialog output is transient")
        );
        assert!(!report.variables.contains_key("n-form-dialog.values"));
        assert!(
            report
                .logs
                .iter()
                .any(|entry| entry.message == "username=Ada")
        );
        assert!(
            report
                .logs
                .iter()
                .all(|entry| !entry.message.contains("must-not-persist"))
        );
        assert!(
            store
                .variables
                .lock()
                .expect("variable lock should work")
                .is_empty()
        );
    }
}

#[test]
fn persists_incremented_values_between_runs() {
    let store = TestStateStore::default();
    let program = variable_program("persistent", "increment", json!(1), "{{counter}}");

    let first =
        execute_manual_program_with_state(&program, "script-1", state_resources(&store, &[]))
            .expect("first run should execute");
    let second =
        execute_manual_program_with_state(&program, "script-1", state_resources(&store, &[]))
            .expect("second run should execute");

    assert_eq!(
        first.variables.get("counter").and_then(Value::as_f64),
        Some(1.0)
    );
    assert_eq!(
        second.variables.get("counter").and_then(Value::as_f64),
        Some(2.0)
    );
    assert_eq!(
        second.variable_scopes.get("counter"),
        Some(&RunVariableScope::Persistent)
    );
}

#[test]
fn runtime_default_resets_before_each_run() {
    let store = TestStateStore::default();
    let defaults = [default_variable(
        "counter",
        RuntimeDefaultVariableScope::Runtime,
        "integer",
        json!(10),
    )];
    let program = variable_program("runtime", "increment", json!(1), "{{counter}}");

    let first = execute_manual_program_with_state(
        &program,
        "script-1",
        state_resources_with_defaults(&store, &[], &defaults),
    )
    .expect("first run should execute");
    let second = execute_manual_program_with_state(
        &program,
        "script-1",
        state_resources_with_defaults(&store, &[], &defaults),
    )
    .expect("second run should execute");

    assert_eq!(first.variables.get("counter"), Some(&json!(11)));
    assert_eq!(second.variables.get("counter"), Some(&json!(11)));
}

#[test]
fn persistent_default_initializes_once_then_retains_changes() {
    let store = TestStateStore::default();
    let defaults = [default_variable(
        "counter",
        RuntimeDefaultVariableScope::Persistent,
        "integer",
        json!(10),
    )];
    let program = variable_program("persistent", "increment", json!(1), "{{counter}}");

    let first = execute_manual_program_with_state(
        &program,
        "script-1",
        state_resources_with_defaults(&store, &[], &defaults),
    )
    .expect("first run should execute");
    let second = execute_manual_program_with_state(
        &program,
        "script-1",
        state_resources_with_defaults(&store, &[], &defaults),
    )
    .expect("second run should execute");

    assert_eq!(first.variables.get("counter"), Some(&json!(11)));
    assert_eq!(second.variables.get("counter"), Some(&json!(12)));
}

#[test]
fn deleting_a_persistent_variable_removes_its_stored_value() {
    let store = TestStateStore::default();
    execute_manual_program_with_state(
        &variable_program("persistent", "set", json!("saved"), "stored"),
        "script-1",
        state_resources(&store, &[]),
    )
    .expect("persistent value should store");

    let mut delete_program = variable_program("persistent", "delete", Value::Null, "deleted");
    delete_program["entry"]["program"]["steps"][0]["config"]
        .as_object_mut()
        .expect("delete config should be an object")
        .remove("valueType");
    let deleted = execute_manual_program_with_state(
        &delete_program,
        "script-1",
        state_resources(&store, &[]),
    )
    .expect("persistent value should delete");

    assert!(!deleted.variables.contains_key("counter"));
    assert!(
        store
            .load_variable(RuntimeVariableScope::Persistent, "script-1", "counter")
            .expect("stored value should load")
            .is_none()
    );
}

#[test]
fn rejects_default_that_disagrees_with_variable_operation() {
    let store = TestStateStore::default();
    let defaults = [default_variable(
        "counter",
        RuntimeDefaultVariableScope::Persistent,
        "integer",
        json!(10),
    )];
    let error = execute_manual_program_with_state(
        &variable_program("runtime", "increment", json!(1), "done"),
        "script-1",
        state_resources_with_defaults(&store, &[], &defaults),
    )
    .expect_err("scope mismatch must block execution");

    assert!(
        error
            .to_string()
            .contains("does not match Variable Operation")
    );
}

#[test]
fn rejects_malformed_default_resources_before_execution() {
    let store = TestStateStore::default();
    for (variable, expected) in [
        (
            default_variable(
                "counter",
                RuntimeDefaultVariableScope::Runtime,
                "integer",
                json!("ten"),
            ),
            "expected integer",
        ),
        (
            default_variable(
                "system_counter",
                RuntimeDefaultVariableScope::Runtime,
                "integer",
                json!(10),
            ),
            "invalid or reserved",
        ),
        (
            default_variable(
                "counter",
                RuntimeDefaultVariableScope::Runtime,
                "string",
                json!(""),
            ),
            "value does not match type",
        ),
    ] {
        let defaults = [variable];
        let error = execute_manual_program_with_state(
            &variable_program("runtime", "increment", json!(1), "done"),
            "script-1",
            state_resources_with_defaults(&store, &[], &defaults),
        )
        .expect_err("malformed runtime resources must block execution");

        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn loads_required_secret_and_redacts_reports() {
    let store = TestStateStore::default();
    store
        .secrets
        .lock()
        .expect("test secret lock should work")
        .insert(
            ("script-1".to_owned(), "api_key".to_owned()),
            json!("actual-secret"),
        );
    let program = variable_program("runtime", "set", json!("{{api_key}}"), "key={{api_key}}");
    let declarations = [RuntimeSecretDeclaration {
        name: "api_key".to_owned(),
        required: true,
        value_type: "string".to_owned(),
    }];
    let observer = Arc::new(LogObserver::default());
    let report = execute_manual_program_with_state(
        &program,
        "script-1",
        state_resources(&store, &declarations).with_observer(observer.clone()),
    )
    .expect("secret-backed run should execute");

    assert!(!report.variables.contains_key("api_key"));
    assert!(!report.variable_scopes.contains_key("api_key"));
    assert!(!report.variable_scopes.contains_key("api_key.$type"));
    assert_eq!(report.variables.get("counter"), Some(&json!("[REDACTED]")));
    assert!(
        report
            .logs
            .iter()
            .all(|log| !log.message.contains("actual-secret"))
    );
    assert!(
        report
            .logs
            .iter()
            .any(|log| log.message.contains("[REDACTED]"))
    );
    assert!(
        observer
            .logs
            .lock()
            .expect("observer log lock should work")
            .iter()
            .all(|log| !log.message.contains("actual-secret"))
    );
}

#[test]
fn rejects_missing_required_secret_before_execution() {
    let store = TestStateStore::default();
    let declarations = [RuntimeSecretDeclaration {
        name: "api_key".to_owned(),
        required: true,
        value_type: "string".to_owned(),
    }];
    let error = execute_manual_program_with_state(
        &variable_program("runtime", "set", json!("ok"), "done"),
        "script-1",
        state_resources(&store, &declarations),
    )
    .expect_err("missing required secret must block execution");
    assert!(error.to_string().contains("required secret"));
}

#[test]
fn exposes_script_settings_through_the_read_only_settings_object() {
    let store = TestStateStore::default();
    let settings = RuntimeScriptSettings {
        values: json!({
            "enabled": true,
            "endpoint": "https://example.test/api",
            "release-channel_2": "stable",
            "retries": 3
        }),
    };
    let report = execute_manual_program_with_state(
        &variable_program(
            "runtime",
            "set",
            json!("ok"),
            "{{settings.endpoint}} retries={{settings.retries}} enabled={{settings.enabled}} channel={{settings.release-channel_2}}",
        ),
        "script-1",
        state_resources(&store, &[]).with_script_settings(&settings),
    )
    .expect("Script Settings should resolve during execution");

    assert_eq!(
        report.variables.get("settings"),
        Some(&json!({
            "enabled": true,
            "endpoint": "https://example.test/api",
            "release-channel_2": "stable",
            "retries": 3
        }))
    );
    assert_eq!(
        report.variable_scopes.get("settings"),
        Some(&RunVariableScope::Setting)
    );
    assert!(report.logs.iter().any(|entry| {
        entry.message == "https://example.test/api retries=3 enabled=true channel=stable"
    }));
}

#[test]
fn rejects_a_non_object_script_settings_snapshot() {
    let store = TestStateStore::default();
    let settings = RuntimeScriptSettings {
        values: json!(["invalid"]),
    };
    let error = execute_manual_program_with_state(
        &variable_program("runtime", "set", json!("ok"), "done"),
        "script-1",
        state_resources(&store, &[]).with_script_settings(&settings),
    )
    .expect_err("Script Settings must use an object root");

    assert!(
        error
            .to_string()
            .contains("Script Settings must be provided as an object")
    );
}

#[test]
fn declared_default_variables_reject_a_wrong_typed_value() {
    let error = build_initial_state_with_default("retries", "integer", json!("three"))
        .expect_err("a wrong-typed default must be rejected");

    assert!(
        matches!(error, crate::RuntimeError::Type { .. }),
        "a type mismatch must stop the run as a type error rather than any other failure: {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("retries"),
        "message names the variable: {message}"
    );
    assert!(
        message.contains("integer"),
        "message names the type: {message}"
    );
}

#[test]
fn declared_default_variables_accept_a_correctly_typed_integer_value() {
    let report = build_initial_state_with_default("retries", "integer", json!(3))
        .expect("a correctly typed integer default should be accepted");

    assert_eq!(report.variables.get("retries"), Some(&json!(3)));
}

fn build_initial_state_with_default(
    name: &str,
    value_type: &str,
    value: Value,
) -> Result<crate::RunReport, crate::RuntimeError> {
    let store = TestStateStore::default();
    let defaults = [default_variable(
        name,
        RuntimeDefaultVariableScope::Runtime,
        value_type,
        value,
    )];
    execute_manual_program_with_state(
        &variable_program("runtime", "increment", json!(1), "done"),
        "script-1",
        state_resources_with_defaults(&store, &[], &defaults),
    )
}

fn state_resources<'a>(
    store: &'a TestStateStore,
    secrets: &'a [RuntimeSecretDeclaration],
) -> RuntimeExecutionResources<'a> {
    RuntimeExecutionResources::new(&UnsupportedActionHandler).with_state(store, secrets)
}

fn state_resources_with_defaults<'a>(
    store: &'a TestStateStore,
    secrets: &'a [RuntimeSecretDeclaration],
    defaults: &'a [RuntimeDefaultVariable],
) -> RuntimeExecutionResources<'a> {
    state_resources(store, secrets).with_default_variables(defaults)
}

fn default_variable(
    name: &str,
    scope: RuntimeDefaultVariableScope,
    value_type: &str,
    value: Value,
) -> RuntimeDefaultVariable {
    RuntimeDefaultVariable {
        name: name.to_owned(),
        scope,
        value_type: value_type.to_owned(),
        item_type: None,
        value,
    }
}

fn variable_program(scope: &str, operation: &str, value: Value, message: &str) -> Value {
    json!({
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
                "steps": [
                    {
                        "id": "n-variable",
                        "action_type": "runtime.set_variable",
                        "type": "set_variable",
                        "config": {
                            "name": "counter",
                            "operation": operation,
                            "scope": scope,
                            "valueType": if operation == "increment" { "integer" } else { "string" },
                            "value": value
                        },
                        "runtime_outputs": []
                    },
                    {
                        "id": "n-log",
                        "action_type": "action.log",
                        "type": "action",
                        "action": "log",
                        "config": {"level": "info", "message": message},
                        "runtime_outputs": []
                    }
                ],
                "edges": [
                    {"execution_order": 0, "source": "n-trigger", "source_handle": "out", "target": "n-variable", "target_handle": "input"},
                    {"execution_order": 0, "source": "n-variable", "source_handle": "out", "target": "n-log", "target_handle": "input"}
                ]
            }
        }
    })
}

#[test]
fn a_list_default_rejects_elements_that_do_not_match_the_item_type() {
    // The whole-value check only confirms a list is an array, so element types
    // have to be checked where the list is declared.
    for (item_type, bad_element) in [
        ("integer", json!("not a number")),
        ("float", json!(3)),
        ("color", json!("red")),
        ("hotkey", json!("NotARealKey")),
    ] {
        let store = TestStateStore::default();
        let mut variable = default_variable(
            "items",
            RuntimeDefaultVariableScope::Runtime,
            "list",
            json!([bad_element]),
        );
        variable.item_type = Some(item_type.to_owned());

        let error = execute_manual_program_with_state(
            &variable_program("runtime", "increment", json!(1), "done"),
            "script-1",
            state_resources_with_defaults(&store, &[], &[variable]),
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("item 0"),
            "the error should name the mismatch, found {error}"
        );
    }
}
