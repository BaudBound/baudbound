use baudbound_runtime::{
    RuntimeActionError, RuntimeActionHandler, RuntimeActionRequest, RuntimeActionResult,
};
use baudbound_storage::ScriptStore;
use serde_json::Value;

use crate::RunnerCore;

pub(crate) struct CoreRuntimeActionHandler<'a, S: ScriptStore> {
    call_stack: Vec<String>,
    core: &'a RunnerCore,
    delegate: &'a dyn RuntimeActionHandler,
    store: &'a S,
}

impl<'a, S: ScriptStore> CoreRuntimeActionHandler<'a, S> {
    pub(crate) fn new(
        call_stack: Vec<String>,
        core: &'a RunnerCore,
        delegate: &'a dyn RuntimeActionHandler,
        store: &'a S,
    ) -> Self {
        Self {
            call_stack,
            core,
            delegate,
            store,
        }
    }
}

impl<S: ScriptStore> RuntimeActionHandler for CoreRuntimeActionHandler<'_, S> {
    fn execute_action(
        &self,
        request: &RuntimeActionRequest,
        context: &baudbound_runtime::RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        if request.action_type == "action.script.run" {
            return self.execute_sub_script(request);
        }

        self.delegate.execute_action(request, context)
    }
}

impl<S: ScriptStore> CoreRuntimeActionHandler<'_, S> {
    fn execute_sub_script(
        &self,
        request: &RuntimeActionRequest,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        let script = required_action_config_string(request, "script")?;
        let report = self
            .core
            .run_installed_with_trigger_in_stack(
                self.store,
                &script,
                None,
                serde_json::Value::Null,
                self.call_stack.clone(),
            )
            .map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("sub-script {script:?} failed: {source}"),
            })?;

        Ok(RuntimeActionResult {
            output_data: serde_json::Map::from_iter([
                ("status".to_owned(), Value::String("completed".to_owned())),
                ("exit_code".to_owned(), Value::Number(0.into())),
                ("run_id".to_owned(), Value::String(report.identity.run_id)),
                (
                    "script_id".to_owned(),
                    Value::String(report.identity.script_id),
                ),
                (
                    "trigger_node_id".to_owned(),
                    Value::String(report.identity.trigger_node_id),
                ),
            ]),
        })
    }
}

fn required_action_config_string(
    request: &RuntimeActionRequest,
    key: &str,
) -> Result<String, RuntimeActionError> {
    request
        .config
        .get(key)
        .map(action_config_value_to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("missing required config field {key}"),
        })
}

fn action_config_value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}
