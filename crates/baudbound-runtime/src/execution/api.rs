use std::path::PathBuf;

use crate::runtime::RuntimeGraph;
use serde_json::Value;

use super::{
    RunIdentity, RunReport, RuntimeActionHandler, RuntimeError, RuntimeExecutionResources,
    RuntimeExecutor, UnsupportedActionHandler,
};

pub fn execute_manual_program(program: &Value, script_id: &str) -> Result<RunReport, RuntimeError> {
    execute_manual_program_with_actions(program, script_id, &UnsupportedActionHandler)
}

pub fn execute_manual_program_with_actions(
    program: &Value,
    script_id: &str,
    action_handler: &dyn RuntimeActionHandler,
) -> Result<RunReport, RuntimeError> {
    execute_manual_program_with_actions_and_package_path(program, script_id, None, action_handler)
}

pub fn execute_manual_program_with_actions_and_package_path(
    program: &Value,
    script_id: &str,
    package_path: Option<PathBuf>,
    action_handler: &dyn RuntimeActionHandler,
) -> Result<RunReport, RuntimeError> {
    let mut resources = RuntimeExecutionResources::new(action_handler);
    resources.package_path = package_path;
    execute_manual_program_with_state(program, script_id, resources)
}

pub fn execute_manual_program_with_state(
    program: &Value,
    script_id: &str,
    resources: RuntimeExecutionResources<'_>,
) -> Result<RunReport, RuntimeError> {
    let graph = RuntimeGraph::from_program_value(program)?;
    let trigger_node_id = graph.manual_trigger()?.id.clone();
    execute_graph_from_trigger(graph, script_id, &trigger_node_id, Value::Null, resources)
}

pub fn execute_trigger_program_with_actions(
    program: &Value,
    script_id: &str,
    trigger_node_id: &str,
    trigger_payload: Value,
    action_handler: &dyn RuntimeActionHandler,
) -> Result<RunReport, RuntimeError> {
    execute_trigger_program_with_actions_and_package_path(
        program,
        script_id,
        trigger_node_id,
        None,
        trigger_payload,
        action_handler,
    )
}

pub fn execute_trigger_program_with_actions_and_package_path(
    program: &Value,
    script_id: &str,
    trigger_node_id: &str,
    package_path: Option<PathBuf>,
    trigger_payload: Value,
    action_handler: &dyn RuntimeActionHandler,
) -> Result<RunReport, RuntimeError> {
    let mut resources = RuntimeExecutionResources::new(action_handler);
    resources.package_path = package_path;
    execute_trigger_program_with_state(
        program,
        script_id,
        trigger_node_id,
        trigger_payload,
        resources,
    )
}

pub fn execute_trigger_program_with_state(
    program: &Value,
    script_id: &str,
    trigger_node_id: &str,
    trigger_payload: Value,
    resources: RuntimeExecutionResources<'_>,
) -> Result<RunReport, RuntimeError> {
    let graph = RuntimeGraph::from_program_value(program)?;
    execute_graph_from_trigger(
        graph,
        script_id,
        trigger_node_id,
        trigger_payload,
        resources,
    )
}

fn execute_graph_from_trigger(
    graph: RuntimeGraph,
    script_id: &str,
    trigger_node_id: &str,
    trigger_payload: Value,
    resources: RuntimeExecutionResources<'_>,
) -> Result<RunReport, RuntimeError> {
    let trigger = graph.trigger(trigger_node_id)?;
    let identity = RunIdentity {
        run_id: create_run_id(script_id, &trigger.id),
        script_id: script_id.to_owned(),
        trigger_node_id: trigger.id.clone(),
    };
    let mut executor = RuntimeExecutor::new(graph, identity, trigger_payload, resources)?;
    match executor.run_from_trigger() {
        Ok(report) => Ok(executor.redact_report(report)),
        Err(RuntimeError::Cancelled) => Err(RuntimeError::Cancelled),
        Err(error) if executor.has_secrets() => Err(RuntimeError::Redacted(
            executor.redact_text(&error.to_string()),
        )),
        Err(error) => Err(error),
    }
}

fn create_run_id(script_id: &str, trigger_node_id: &str) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{script_id}:{trigger_node_id}:{timestamp}")
}
