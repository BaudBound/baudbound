use baudbound_runtime::{RuntimeStateStore, RuntimeVariableScope, VersionedRuntimeVariable};
use baudbound_storage::{ScriptStore, StoredVariableScope};
use serde_json::Value;

pub(crate) struct CoreRuntimeStateStore<'a, S: ScriptStore> {
    store: &'a S,
}

impl<'a, S: ScriptStore> CoreRuntimeStateStore<'a, S> {
    pub(crate) fn new(store: &'a S) -> Self {
        Self { store }
    }
}

impl<S: ScriptStore> RuntimeStateStore for CoreRuntimeStateStore<'_, S> {
    fn load_variable(
        &self,
        scope: RuntimeVariableScope,
        script_id: &str,
        name: &str,
    ) -> Result<Option<VersionedRuntimeVariable>, String> {
        self.store
            .load_variable(to_stored_scope(scope), script_id, name)
            .map(|variable| {
                variable.map(|variable| VersionedRuntimeVariable {
                    value: variable.value,
                    version: variable.version,
                })
            })
            .map_err(|error| error.to_string())
    }

    fn compare_and_set_variable(
        &self,
        scope: RuntimeVariableScope,
        script_id: &str,
        name: &str,
        expected_version: Option<u64>,
        value: &Value,
    ) -> Result<bool, String> {
        self.store
            .compare_and_set_variable(
                to_stored_scope(scope),
                script_id,
                name,
                expected_version,
                value,
            )
            .map_err(|error| error.to_string())
    }

    fn read_secret(&self, script_id: &str, name: &str) -> Result<Option<Value>, String> {
        self.store
            .read_secret(script_id, name)
            .map_err(|error| error.to_string())
    }
}

fn to_stored_scope(scope: RuntimeVariableScope) -> StoredVariableScope {
    match scope {
        RuntimeVariableScope::Persistent => StoredVariableScope::Persistent,
        RuntimeVariableScope::Global => StoredVariableScope::Global,
    }
}
