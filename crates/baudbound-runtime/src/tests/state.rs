use std::{collections::BTreeMap, sync::Mutex};

use serde_json::{Value, json};

use crate::{
    RuntimeExecutionResources, RuntimeSecretDeclaration, RuntimeStateStore, RuntimeVariableScope,
    UnsupportedActionHandler, VersionedRuntimeVariable, execute_manual_program_with_state,
};

#[derive(Default)]
struct TestStateStore {
    secrets: Mutex<BTreeMap<(String, String), Value>>,
    variables: Mutex<BTreeMap<(RuntimeVariableScopeKey, String, String), VersionedRuntimeVariable>>,
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
    let report = execute_manual_program_with_state(
        &program,
        "script-1",
        state_resources(&store, &declarations),
    )
    .expect("secret-backed run should execute");

    assert!(!report.variables.contains_key("api_key"));
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

fn state_resources<'a>(
    store: &'a TestStateStore,
    secrets: &'a [RuntimeSecretDeclaration],
) -> RuntimeExecutionResources<'a> {
    RuntimeExecutionResources::new(&UnsupportedActionHandler).with_state(store, secrets)
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
                            "valueType": if operation == "increment" { "number" } else { "string" },
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
                    {"source": "n-trigger", "source_handle": "out", "target": "n-variable", "target_handle": "input"},
                    {"source": "n-variable", "source_handle": "out", "target": "n-log", "target_handle": "input"}
                ]
            }
        }
    })
}
