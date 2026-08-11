use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use serde_json::{Value, json};

use crate::{
    ResourceLimit, RunIdentity, RunVariableScope, RuntimeActionError, RuntimeActionHandler,
    RuntimeActionRequest, RuntimeActionResult, RuntimeCancellationToken, RuntimeContext,
    RuntimeDeclaredScope, RuntimeDeclaredVariable, RuntimeExecutionPolicy,
    RuntimeExecutionResources, RuntimeLogEntry, RuntimeRunObserver, RuntimeScriptSettings,
    RuntimeSecretDeclaration, RuntimeStateStore, RuntimeVariableScope, UnsupportedActionHandler,
    VersionedRuntimeVariable, execute_manual_program_with_state,
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
            .get(&variable_key(scope, script_id, name))
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
        let key = variable_key(scope, script_id, name);
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
            .remove(&variable_key(scope, script_id, name))
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

/// The key a scope stores under, matching the real store.
///
/// A persistent variable belongs to one script and is keyed by it. A global
/// belongs to none and is keyed by name alone: the SQLite store reads it with
/// `WHERE name = ?1` and no script id at all. Keying a global by script here
/// would make every script's global private, and a test written against this
/// double would then prove the opposite of what the runner does.
fn variable_key(
    scope: RuntimeVariableScope,
    script_id: &str,
    name: &str,
) -> (RuntimeVariableScopeKey, String, String) {
    let owner = match scope {
        RuntimeVariableScope::Global => String::new(),
        RuntimeVariableScope::Persistent => script_id.to_owned(),
    };
    (scope.into(), owner, name.to_owned())
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
        let declared = [declared_variable(
            "stored_password",
            match scope {
                "global" => RuntimeDeclaredScope::Global,
                _ => RuntimeDeclaredScope::Persistent,
            },
            "string",
            json!("unset"),
        )];
        let resources = RuntimeExecutionResources::new(&SensitiveFormDialogHandler)
            .with_state(&store, &[])
            .with_declared_variables(&declared);

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
        // The store is no longer empty: a persistent or global declaration
        // initialises itself there before the run starts. What must never
        // reach it is the sensitive value the node tried to write.
        assert!(
            store
                .variables
                .lock()
                .expect("variable lock should work")
                .values()
                .all(|stored| stored.value != json!("must-not-persist")),
            "a sensitive value must not reach stored state"
        );
    }
}

#[test]
fn persists_incremented_values_between_runs() {
    let store = TestStateStore::default();
    let program = variable_program("persistent", "increment", json!(1), "{{counter}}");
    let declarations = counter_declarations("persistent", "increment");

    let first = execute_manual_program_with_state(
        &program,
        "script-1",
        state_resources_with_defaults(&store, &[], &declarations),
    )
    .expect("first run should execute");
    let second = execute_manual_program_with_state(
        &program,
        "script-1",
        state_resources_with_defaults(&store, &[], &declarations),
    )
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
    let defaults = [declared_variable(
        "counter",
        RuntimeDeclaredScope::Runtime,
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
    let defaults = [declared_variable(
        "counter",
        RuntimeDeclaredScope::Persistent,
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
fn a_declared_global_is_shared_and_a_later_script_adopts_it() {
    // A global belongs to no script. The second script to declare one has to
    // find the first script's value there, not reset it to its own declared
    // starting point, or installing a package would quietly wipe state that
    // another package was keeping.
    let store = TestStateStore::default();
    let declared = [declared_variable(
        "counter",
        RuntimeDeclaredScope::Global,
        "integer",
        json!(10),
    )];
    let program = variable_program("global", "increment", json!(1), "{{counter}}");

    let first = execute_manual_program_with_state(
        &program,
        "script-1",
        state_resources_with_defaults(&store, &[], &declared),
    )
    .expect("the first script should execute");
    assert_eq!(first.variables.get("counter"), Some(&json!(11)));

    // A different script id, declaring the same name with the same declared
    // starting value. It must see 11 and carry on from there.
    let second = execute_manual_program_with_state(
        &program,
        "script-2",
        state_resources_with_defaults(&store, &[], &declared),
    )
    .expect("the second script should execute");
    assert_eq!(
        second.variables.get("counter"),
        Some(&json!(12)),
        "a declared global adopts the stored value rather than reinitialising"
    );

    // And the first script sees what the second wrote.
    let third = execute_manual_program_with_state(
        &program,
        "script-1",
        state_resources_with_defaults(&store, &[], &declared),
    )
    .expect("the first script should execute again");
    assert_eq!(third.variables.get("counter"), Some(&json!(13)));
}

#[test]
fn resetting_a_persistent_variable_stores_its_declared_value() {
    let store = TestStateStore::default();
    // The declaration says "start", so that is what reset restores — not an
    // empty string, which is what clear would give, and not nothing, which is
    // what delete used to do before a variable's existence came from its
    // declaration rather than from having been written.
    let declarations = [declared_variable(
        "counter",
        RuntimeDeclaredScope::Persistent,
        "string",
        json!("start"),
    )];
    execute_manual_program_with_state(
        &variable_program("persistent", "set", json!("saved"), "stored"),
        "script-1",
        state_resources_with_defaults(&store, &[], &declarations),
    )
    .expect("persistent value should store");

    let mut reset_program = variable_program("persistent", "reset", Value::Null, "reset");
    reset_program["entry"]["program"]["steps"][0]["config"]
        .as_object_mut()
        .expect("reset config should be an object")
        .remove("valueType");
    let reset = execute_manual_program_with_state(
        &reset_program,
        "script-1",
        state_resources_with_defaults(&store, &[], &declarations),
    )
    .expect("persistent value should reset");

    assert_eq!(reset.variables.get("counter"), Some(&json!("start")));
    assert_eq!(
        store
            .load_variable(RuntimeVariableScope::Persistent, "script-1", "counter")
            .expect("stored value should load")
            .map(|stored| stored.value),
        Some(json!("start"))
    );
}

#[test]
fn a_node_cannot_write_a_variable_the_manifest_does_not_declare() {
    // This replaces a test that a node's scope and type had to agree with the
    // declaration's. They cannot disagree any more: a node names a declared
    // variable and the declaration settles both. What can still go wrong is a
    // node naming nothing at all, which is a package fault rather than a node
    // failure, so it must stop the run rather than take the failed output.
    let store = TestStateStore::default();
    let error = execute_manual_program_with_state(
        &variable_program("runtime", "increment", json!(1), "done"),
        "script-1",
        state_resources_with_defaults(&store, &[], &[]),
    )
    .expect_err("an undeclared write must block execution");

    let message = error.to_string();
    assert!(
        message.contains("counter"),
        "the message names the variable"
    );
    assert!(
        message.contains("does not declare"),
        "unexpected message: {message}"
    );
}

#[test]
fn rejects_malformed_default_resources_before_execution() {
    let store = TestStateStore::default();
    for (variable, expected) in [
        (
            declared_variable(
                "counter",
                RuntimeDeclaredScope::Runtime,
                "integer",
                json!("ten"),
            ),
            "expected integer",
        ),
        // A name with a space cannot be an identifier. "system_counter" used
        // to belong here too; it is an ordinary name now that every built-in
        // lives behind "@", which no identifier may contain.
        (
            declared_variable(
                "counter with a space",
                RuntimeDeclaredScope::Runtime,
                "integer",
                json!(10),
            ),
            "is invalid",
        ),
        (
            declared_variable(
                "counter",
                RuntimeDeclaredScope::Runtime,
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
        state_resources_with_defaults(&store, &declarations, &counter_declarations("runtime", "set"))
            .with_observer(observer.clone()),
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
            "{{@settings.endpoint}} retries={{@settings.retries}} enabled={{@settings.enabled}} channel={{@settings.release-channel_2}}",
        ),
        "script-1",
        state_resources_with_defaults(&store, &[], &counter_declarations("runtime", "set"))
            .with_script_settings(&settings),
    )
    .expect("Script Settings should resolve during execution");

    assert_eq!(
        report.variables.get("@settings"),
        Some(&json!({
            "enabled": true,
            "endpoint": "https://example.test/api",
            "release-channel_2": "stable",
            "retries": 3
        }))
    );
    assert_eq!(
        report.variable_scopes.get("@settings"),
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
fn declared_declared_variables_reject_a_wrong_typed_value() {
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
fn declared_declared_variables_accept_a_correctly_typed_integer_value() {
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
    let mut defaults = vec![declared_variable(
        name,
        RuntimeDeclaredScope::Runtime,
        value_type,
        value,
    )];
    // The program always writes `counter`. When the declaration under test is
    // named something else, that write still needs a declaration of its own or
    // the run stops before reaching the thing the test is about.
    if name != "counter" {
        defaults.push(declared_variable(
            "counter",
            RuntimeDeclaredScope::Runtime,
            "integer",
            json!(0),
        ));
    }
    execute_manual_program_with_state(
        &variable_program("runtime", "increment", json!(1), "done"),
        "script-1",
        state_resources_with_defaults(&store, &[], &defaults),
    )
}

/// The declaration `variable_program` needs, since it always writes `counter`.
///
/// A variable exists because it is declared, so a program built inline needs
/// one before it will start. These tests are about state rather than about
/// declaring, so the declaration is supplied here instead of in each of them.
static COUNTER_DECLARATION: std::sync::LazyLock<[RuntimeDeclaredVariable; 1]> =
    std::sync::LazyLock::new(|| {
        [RuntimeDeclaredVariable {
            name: "counter".to_owned(),
            scope: RuntimeDeclaredScope::Runtime,
            value_type: "integer".to_owned(),
            item_type: None,
            value: json!(0),
        }]
    });

/// The declaration matching what [`variable_program`] builds.
///
/// Scope and type come from the declaration now, so a program that increments
/// a persistent counter only does so if the declaration says persistent and
/// integer. The two are derived from the same arguments here to keep them from
/// drifting apart.
fn counter_declarations(scope: &str, operation: &str) -> [RuntimeDeclaredVariable; 1] {
    let numeric = operation == "increment";
    [RuntimeDeclaredVariable {
        name: "counter".to_owned(),
        scope: match scope {
            "persistent" => RuntimeDeclaredScope::Persistent,
            "global" => RuntimeDeclaredScope::Global,
            _ => RuntimeDeclaredScope::Runtime,
        },
        value_type: if numeric { "integer" } else { "string" }.to_owned(),
        item_type: None,
        value: if numeric { json!(0) } else { json!("declared") },
    }]
}

fn state_resources<'a>(
    store: &'a TestStateStore,
    secrets: &'a [RuntimeSecretDeclaration],
) -> RuntimeExecutionResources<'a> {
    RuntimeExecutionResources::new(&UnsupportedActionHandler)
        .with_state(store, secrets)
        .with_declared_variables(&*COUNTER_DECLARATION)
}

fn state_resources_with_defaults<'a>(
    store: &'a TestStateStore,
    secrets: &'a [RuntimeSecretDeclaration],
    defaults: &'a [RuntimeDeclaredVariable],
) -> RuntimeExecutionResources<'a> {
    state_resources(store, secrets).with_declared_variables(defaults)
}

fn declared_variable(
    name: &str,
    scope: RuntimeDeclaredScope,
    value_type: &str,
    value: Value,
) -> RuntimeDeclaredVariable {
    RuntimeDeclaredVariable {
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
        let mut variable = declared_variable(
            "items",
            RuntimeDeclaredScope::Runtime,
            "list",
            json!([bad_element]),
        );
        variable.item_type = Some(item_type.to_owned());

        // The program writes `counter`, which needs a declaration of its own or
        // the run stops on that before reaching the malformed list.
        let declared = [
            variable,
            declared_variable(
                "counter",
                RuntimeDeclaredScope::Runtime,
                "integer",
                json!(0),
            ),
        ];
        let error = execute_manual_program_with_state(
            &variable_program("runtime", "increment", json!(1), "done"),
            "script-1",
            state_resources_with_defaults(&store, &[], &declared),
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("item 0"),
            "the error should name the mismatch, found {error}"
        );
    }
}

/// Flips a stored variable directly in the store on its first call, standing
/// in for a different run changing it. It deliberately does not touch the run
/// context, which is what a Variable Operation would do.
struct OutsideWriter {
    store: Arc<TestStateStore>,
    calls: Mutex<usize>,
}

impl std::fmt::Debug for OutsideWriter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OutsideWriter")
    }
}

impl RuntimeActionHandler for OutsideWriter {
    fn execute_action(
        &self,
        _request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        let mut calls = self.calls.lock().expect("call count lock should work");
        *calls += 1;
        if *calls == 1 {
            let existing = self
                .store
                .load_variable(RuntimeVariableScope::Persistent, "script-1", "running")
                .expect("the flag should load");
            self.store
                .compare_and_set_variable(
                    RuntimeVariableScope::Persistent,
                    "script-1",
                    "running",
                    existing.map(|variable| variable.version),
                    &json!(false),
                )
                .expect("flipping the flag should work");
        }
        Ok(RuntimeActionResult::new(serde_json::Map::new()))
    }
}

#[test]
fn a_loop_condition_sees_a_stored_variable_changed_outside_the_run() {
    // The reason the refresh exists. The flag is flipped in the store by
    // something that never touches this run's variables, so only a reload at
    // the condition can end the loop. Without it the loop runs until a
    // resource limit stops it.
    let store = Arc::new(TestStateStore::default());
    store
        .compare_and_set_variable(
            RuntimeVariableScope::Persistent,
            "script-1",
            "running",
            None,
            &json!(true),
        )
        .expect("seeding the flag should work");

    let handler = OutsideWriter {
        store: Arc::clone(&store),
        calls: Mutex::new(0),
    };
    let program = json!({
        "entry": {
            "trigger": manual_trigger(),
            "triggers": [],
            "program": {
                "steps": [
                    {
                        "id": "n-while",
                        "action_type": "control.while",
                        "type": "while",
                        "config": {
                            "conditions": [{
                                "id": "row",
                                "left": "{{running}}",
                                "operator": "==",
                                "right": "true"
                            }]
                        },
                        "runtime_outputs": []
                    },
                    {
                        "id": "n-outside",
                        "action_type": "action.clipboard.set",
                        "type": "action",
                        "config": {},
                        "runtime_outputs": []
                    }
                ],
                "edges": [
                    edge("n-trigger", "out", "n-while"),
                    edge("n-while", "loop", "n-outside")
                ]
            }
        }
    });

    let defaults = [RuntimeDeclaredVariable {
        name: "running".to_owned(),
        scope: RuntimeDeclaredScope::Persistent,
        value_type: "boolean".to_owned(),
        item_type: None,
        value: json!(true),
    }];
    let report = execute_manual_program_with_state(
        &program,
        "script-1",
        RuntimeExecutionResources::new(&handler)
            .with_state(store.as_ref(), &[])
            .with_declared_variables(&defaults)
            .with_execution_policy(RuntimeExecutionPolicy {
                max_steps_per_run: ResourceLimit::limited(200),
                max_run_duration_ms: ResourceLimit::limited(10_000),
                max_loop_iterations_per_run: ResourceLimit::limited(20),
            }),
    )
    .expect("the loop should end rather than exhaust its iteration limit");

    assert_eq!(report.variables.get("running"), Some(&json!(false)));
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
