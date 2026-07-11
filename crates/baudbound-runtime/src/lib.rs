//! Runtime primitives for executing BaudBound script graphs.

mod runtime;

use std::{collections::BTreeMap, path::PathBuf};

pub use runtime::{
    RuntimeCancellationToken, RuntimeSecretDeclaration, RuntimeStateStore, RuntimeVariableScope,
    VersionedRuntimeVariable,
};
use runtime::{
    RuntimeConditionRow, RuntimeFrame, RuntimeGraph, RuntimeSwitchCaseRow, coerce_variable_value,
    compare_condition_values, config_string, duration_from_amount, empty_value_for_type,
    evaluate_calculation_expression, number_from_value, number_value,
    refresh_derived_variable_metadata, render_template, required_config_string, resolve_config_map,
    resolve_template_value, set_object_field, validate_variable_name, value_kind, value_to_string,
    values_equal_for_condition,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use thiserror::Error;

pub const SUPPORTED_CONTROL_ACTION_TYPES: &[&str] = &[
    "control.for_each",
    "control.if",
    "control.loop",
    "control.switch",
    "control.while",
];

pub const SUPPORTED_INTERNAL_ACTION_TYPES: &[&str] = &[
    "action.calculate",
    "action.delay",
    "action.log",
    "runtime.set_variable",
];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum RunnerMode {
    DesktopAgent,
    Headless,
    Service,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RunIdentity {
    pub run_id: String,
    pub script_id: String,
    pub trigger_node_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeContext {
    pub identity: RunIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_path: Option<PathBuf>,
    pub trigger_payload: Value,
    pub variables: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeLogEntry {
    pub level: String,
    pub message: String,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunReport {
    pub identity: RunIdentity,
    pub logs: Vec<RuntimeLogEntry>,
    pub variables: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct RuntimeActionRequest {
    pub action: Option<String>,
    pub action_type: String,
    pub config: Map<String, Value>,
    pub node_id: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeActionResult {
    pub output_data: Map<String, Value>,
}

#[derive(Debug, Error)]
pub enum RuntimeActionError {
    #[error("action {0} is not supported by this runner")]
    Unsupported(String),
    #[error("action {action_type} failed: {message}")]
    Failed {
        action_type: String,
        message: String,
    },
}

pub trait RuntimeActionHandler: Send + Sync {
    fn execute_action(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;
}

#[derive(Debug, Default)]
pub struct UnsupportedActionHandler;

impl RuntimeActionHandler for UnsupportedActionHandler {
    fn execute_action(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        Err(RuntimeActionError::Unsupported(request.action_type.clone()))
    }
}

pub struct RuntimeExecutionResources<'a> {
    package_path: Option<PathBuf>,
    action_handler: &'a dyn RuntimeActionHandler,
    cancellation: RuntimeCancellationToken,
    state_store: Option<&'a dyn RuntimeStateStore>,
    secrets: &'a [RuntimeSecretDeclaration],
}

impl<'a> RuntimeExecutionResources<'a> {
    #[must_use]
    pub fn new(action_handler: &'a dyn RuntimeActionHandler) -> Self {
        Self {
            package_path: None,
            action_handler,
            cancellation: RuntimeCancellationToken::new(),
            state_store: None,
            secrets: &[],
        }
    }

    #[must_use]
    pub fn with_package_path(mut self, package_path: PathBuf) -> Self {
        self.package_path = Some(package_path);
        self
    }

    #[must_use]
    pub fn with_cancellation(mut self, cancellation: RuntimeCancellationToken) -> Self {
        self.cancellation = cancellation;
        self
    }

    #[must_use]
    pub fn with_state(
        mut self,
        state_store: &'a dyn RuntimeStateStore,
        secrets: &'a [RuntimeSecretDeclaration],
    ) -> Self {
        self.state_store = Some(state_store);
        self.secrets = secrets;
        self
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeNode {
    pub id: String,
    pub action_type: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub config: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeEdge {
    pub source: String,
    pub source_handle: String,
    pub target: String,
    pub target_handle: String,
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("program graph is invalid: {0}")]
    InvalidGraph(String),
    #[error("action failed for node {node_id}: {message}")]
    Action { node_id: String, message: String },
    #[error("calculation failed for node {node_id}: {message}")]
    Calculation { node_id: String, message: String },
    #[error("control flow failed for node {node_id}: {message}")]
    ControlFlow { node_id: String, message: String },
    #[error("runtime execution is not implemented for {action_type} node {node_id}")]
    UnsupportedStep {
        action_type: String,
        node_id: String,
    },
    #[error("runtime variable operation failed for node {node_id}: {message}")]
    VariableOperation { node_id: String, message: String },
    #[error("runtime state failed: {0}")]
    State(String),
    #[error("runtime execution failed: {0}")]
    Redacted(String),
    #[error("runtime was cancelled")]
    Cancelled,
}

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

struct RuntimeExecutor<'a> {
    graph: RuntimeGraph,
    context: RuntimeContext,
    logs: Vec<RuntimeLogEntry>,
    action_handler: &'a dyn RuntimeActionHandler,
    cancellation: RuntimeCancellationToken,
    state_store: Option<&'a dyn RuntimeStateStore>,
    secret_names: Vec<String>,
    secret_values: Vec<Value>,
}

struct InitialRuntimeState {
    secret_names: Vec<String>,
    secret_values: Vec<Value>,
    variables: BTreeMap<String, Value>,
}

impl<'a> RuntimeExecutor<'a> {
    fn new(
        graph: RuntimeGraph,
        identity: RunIdentity,
        trigger_payload: Value,
        resources: RuntimeExecutionResources<'a>,
    ) -> Result<Self, RuntimeError> {
        let initial_state = load_initial_state(
            &graph,
            &identity.script_id,
            resources.state_store,
            resources.secrets,
        )?;
        Ok(Self {
            graph,
            context: RuntimeContext {
                identity,
                package_path: resources.package_path,
                trigger_payload,
                variables: initial_state.variables,
            },
            logs: Vec::new(),
            action_handler: resources.action_handler,
            cancellation: resources.cancellation,
            state_store: resources.state_store,
            secret_names: initial_state.secret_names,
            secret_values: initial_state.secret_values,
        })
    }

    fn run_from_trigger(&mut self) -> Result<RunReport, RuntimeError> {
        self.ensure_not_cancelled()?;
        let trigger_node_id = self.context.identity.trigger_node_id.clone();
        self.push_runtime_log(
            "info",
            format!("Trigger {} started.", trigger_node_id),
            Some(trigger_node_id.clone()),
        );
        self.seed_trigger_payload_outputs(&trigger_node_id);

        let mut frames = vec![RuntimeFrame::Follow {
            source_node_id: trigger_node_id,
            handle: "out".to_owned(),
            stop_at_node_id: None,
        }];

        while let Some(frame) = frames.pop() {
            self.ensure_not_cancelled()?;
            self.process_frame(frame, &mut frames)?;
        }

        self.push_runtime_log("info", "Run completed.", None);
        Ok(RunReport {
            identity: self.context.identity.clone(),
            logs: self.logs.clone(),
            variables: self.context.variables.clone(),
        })
    }

    fn seed_trigger_payload_outputs(&mut self, trigger_node_id: &str) {
        match self.context.trigger_payload.clone() {
            Value::Object(payload) => {
                for (key, value) in payload {
                    self.set_variable(format!("{trigger_node_id}.{key}"), value);
                }
            }
            Value::Null => {}
            value => {
                self.set_variable(format!("{trigger_node_id}.payload"), value);
            }
        }
    }

    fn process_frame(
        &mut self,
        frame: RuntimeFrame,
        frames: &mut Vec<RuntimeFrame>,
    ) -> Result<(), RuntimeError> {
        match frame {
            RuntimeFrame::Follow {
                source_node_id,
                handle,
                stop_at_node_id,
            } => self.enqueue_follow_frames(frames, &source_node_id, &handle, stop_at_node_id),
            RuntimeFrame::ForEach {
                node_id,
                index,
                items,
            } => self.process_for_each_frame(frames, &node_id, index, items),
            RuntimeFrame::Loop {
                node_id,
                index,
                count,
            } => self.process_loop_frame(frames, &node_id, index, count),
            RuntimeFrame::Node {
                node_id,
                stop_at_node_id,
            } => self.execute_node_frame(frames, &node_id, stop_at_node_id),
            RuntimeFrame::While { node_id, index } => {
                self.process_while_frame(frames, &node_id, index)
            }
        }
    }

    fn enqueue_follow_frames(
        &mut self,
        frames: &mut Vec<RuntimeFrame>,
        source_node_id: &str,
        handle: &str,
        stop_at_node_id: Option<String>,
    ) -> Result<(), RuntimeError> {
        self.graph.node(source_node_id)?;
        let targets = self
            .graph
            .target_node_ids_for_handle(source_node_id, handle);
        if targets.is_empty() {
            self.push_runtime_log(
                "info",
                format!("No connection from {source_node_id} output \"{handle}\". Branch ended."),
                Some(source_node_id.to_owned()),
            );
            return Ok(());
        }

        for target_node_id in targets.into_iter().rev() {
            frames.push(RuntimeFrame::Node {
                node_id: target_node_id,
                stop_at_node_id: stop_at_node_id.clone(),
            });
        }

        Ok(())
    }

    fn execute_node_frame(
        &mut self,
        frames: &mut Vec<RuntimeFrame>,
        node_id: &str,
        stop_at_node_id: Option<String>,
    ) -> Result<(), RuntimeError> {
        if stop_at_node_id.as_deref() == Some(node_id) {
            return Ok(());
        }

        let node = self.graph.node(node_id)?.clone();
        match node.action_type.as_str() {
            "control.if" => {
                let branch = if self.evaluate_conditions(&node)? {
                    "true"
                } else {
                    "false"
                };
                self.push_runtime_log(
                    "info",
                    format!("If / Else {} selected \"{}\" output.", node.id, branch),
                    Some(node.id.clone()),
                );
                frames.push(RuntimeFrame::Follow {
                    source_node_id: node.id,
                    handle: branch.to_owned(),
                    stop_at_node_id: None,
                });
            }
            "control.switch" => {
                let Some(handle) = self.evaluate_switch(&node)? else {
                    self.push_runtime_log(
                        "warn",
                        format!("Switch {} matched no case. Branch ended.", node.id),
                        Some(node.id.clone()),
                    );
                    return Ok(());
                };
                frames.push(RuntimeFrame::Follow {
                    source_node_id: node.id,
                    handle,
                    stop_at_node_id: None,
                });
            }
            "control.loop" => {
                let count = self.loop_count(&node)?;
                frames.push(RuntimeFrame::Loop {
                    node_id: node.id,
                    index: 0,
                    count,
                });
            }
            "control.while" => {
                frames.push(RuntimeFrame::While {
                    node_id: node.id,
                    index: 0,
                });
            }
            "control.for_each" => {
                let items = self.for_each_items(&node)?;
                frames.push(RuntimeFrame::ForEach {
                    node_id: node.id,
                    index: 0,
                    items,
                });
            }
            _ => {
                self.execute_node(&node)?;
                let Some(handle) = self.default_success_handle(&node) else {
                    self.push_runtime_log(
                        "info",
                        format!("{} has no outgoing edge. Branch ended.", node.id),
                        Some(node.id.clone()),
                    );
                    return Ok(());
                };
                frames.push(RuntimeFrame::Follow {
                    source_node_id: node.id,
                    handle,
                    stop_at_node_id: None,
                });
            }
        }

        Ok(())
    }

    fn process_loop_frame(
        &mut self,
        frames: &mut Vec<RuntimeFrame>,
        node_id: &str,
        index: u64,
        count: u64,
    ) -> Result<(), RuntimeError> {
        self.graph.node(node_id)?;
        if index >= count {
            frames.push(RuntimeFrame::Follow {
                source_node_id: node_id.to_owned(),
                handle: "done".to_owned(),
                stop_at_node_id: None,
            });
            return Ok(());
        }

        self.push_runtime_log(
            "info",
            format!("Loop {node_id} iteration {} of {count}.", index + 1),
            Some(node_id.to_owned()),
        );
        frames.push(RuntimeFrame::Loop {
            node_id: node_id.to_owned(),
            index: index + 1,
            count,
        });
        frames.push(RuntimeFrame::Follow {
            source_node_id: node_id.to_owned(),
            handle: "loop".to_owned(),
            stop_at_node_id: Some(node_id.to_owned()),
        });
        Ok(())
    }

    fn process_while_frame(
        &mut self,
        frames: &mut Vec<RuntimeFrame>,
        node_id: &str,
        index: u64,
    ) -> Result<(), RuntimeError> {
        let node = self.graph.node(node_id)?.clone();
        if !self.evaluate_conditions(&node)? {
            self.push_runtime_log(
                "info",
                format!(
                    "While {node_id} condition failed after {index} iteration{}.",
                    if index == 1 { "" } else { "s" }
                ),
                Some(node_id.to_owned()),
            );
            frames.push(RuntimeFrame::Follow {
                source_node_id: node_id.to_owned(),
                handle: "done".to_owned(),
                stop_at_node_id: None,
            });
            return Ok(());
        }

        self.push_runtime_log(
            "info",
            format!("While {node_id} iteration {}; condition passed.", index + 1),
            Some(node_id.to_owned()),
        );
        frames.push(RuntimeFrame::While {
            node_id: node_id.to_owned(),
            index: index + 1,
        });
        frames.push(RuntimeFrame::Follow {
            source_node_id: node_id.to_owned(),
            handle: "loop".to_owned(),
            stop_at_node_id: Some(node_id.to_owned()),
        });
        Ok(())
    }

    fn process_for_each_frame(
        &mut self,
        frames: &mut Vec<RuntimeFrame>,
        node_id: &str,
        index: usize,
        items: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        let node = self.graph.node(node_id)?.clone();
        if index >= items.len() {
            frames.push(RuntimeFrame::Follow {
                source_node_id: node_id.to_owned(),
                handle: "done".to_owned(),
                stop_at_node_id: None,
            });
            return Ok(());
        }

        let item_variable = required_config_string(&node, "itemVariable")?;
        let index_variable = required_config_string(&node, "indexVariable")?;
        validate_variable_name(&node, &item_variable)?;
        validate_variable_name(&node, &index_variable)?;

        self.set_variable(item_variable, items[index].clone());
        self.set_variable(
            index_variable,
            Value::Number(Number::from(u64::try_from(index).unwrap_or(u64::MAX))),
        );
        self.push_runtime_log(
            "info",
            format!("For Each {node_id} item {} of {}.", index + 1, items.len()),
            Some(node_id.to_owned()),
        );

        frames.push(RuntimeFrame::ForEach {
            node_id: node_id.to_owned(),
            index: index + 1,
            items,
        });
        frames.push(RuntimeFrame::Follow {
            source_node_id: node_id.to_owned(),
            handle: "loop".to_owned(),
            stop_at_node_id: Some(node_id.to_owned()),
        });
        Ok(())
    }

    fn execute_node(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        match node.action_type.as_str() {
            "action.log" => self.execute_log(node),
            "runtime.set_variable" => self.execute_variable_operation(node),
            "action.delay" => self.execute_delay(node),
            "action.calculate" => self.execute_calculate(node),
            action_type if action_type.starts_with("action.") => self.execute_external_action(node),
            action_type => Err(RuntimeError::UnsupportedStep {
                action_type: action_type.to_owned(),
                node_id: node.id.clone(),
            }),
        }
    }

    fn execute_log(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        let level = config_string(&node.config, "level").unwrap_or_else(|| "info".to_owned());
        let message_template = config_string(&node.config, "message").unwrap_or_default();
        let message = render_template(&message_template, &self.context.variables);
        self.logs.push(RuntimeLogEntry {
            level,
            message,
            node_id: Some(node.id.clone()),
        });
        Ok(())
    }

    fn execute_variable_operation(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        let name = required_config_string(node, "name")?;
        validate_variable_name(node, &name)?;
        if self.secret_names.iter().any(|secret| secret == &name) {
            return Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("secret {name:?} is read-only"),
            });
        }

        let operation =
            config_string(&node.config, "operation").unwrap_or_else(|| "set".to_owned());
        let value_type =
            config_string(&node.config, "valueType").unwrap_or_else(|| "string".to_owned());
        let scope = match required_config_string(node, "scope")?.as_str() {
            "runtime" => None,
            "persistent" => Some(RuntimeVariableScope::Persistent),
            "global" => Some(RuntimeVariableScope::Global),
            invalid => {
                return Err(RuntimeError::VariableOperation {
                    node_id: node.id.clone(),
                    message: format!("unsupported variable scope {invalid}"),
                });
            }
        };

        if let Some(scope) = scope {
            self.execute_stored_variable_operation(node, scope, &name, &operation, &value_type)?;
        } else {
            let current = self.context.variables.get(&name).cloned();
            let value = self.calculate_variable_operation_value(
                node,
                &name,
                &operation,
                &value_type,
                current,
            )?;
            self.set_variable(name.clone(), value);
        }

        self.push_runtime_log(
            "debug",
            format!("Variable operation {operation} completed."),
            Some(node.id.clone()),
        );
        Ok(())
    }

    fn execute_stored_variable_operation(
        &mut self,
        node: &RuntimeNode,
        scope: RuntimeVariableScope,
        name: &str,
        operation: &str,
        value_type: &str,
    ) -> Result<(), RuntimeError> {
        const MAX_COMPARE_AND_SET_ATTEMPTS: usize = 32;
        let store = self.state_store.ok_or_else(|| {
            RuntimeError::State(
                "stored variable operation requires a runner state store".to_owned(),
            )
        })?;

        for _ in 0..MAX_COMPARE_AND_SET_ATTEMPTS {
            self.ensure_not_cancelled()?;
            let stored = store
                .load_variable(scope, &self.context.identity.script_id, name)
                .map_err(RuntimeError::State)?;
            let expected_version = stored.as_ref().map(|variable| variable.version);
            let current = stored.map(|variable| variable.value);
            match &current {
                Some(value) => self.set_variable(name.to_owned(), value.clone()),
                None => {
                    self.context.variables.remove(name);
                    refresh_derived_variable_metadata(&mut self.context.variables, name);
                }
            }
            let next = self
                .calculate_variable_operation_value(node, name, operation, value_type, current)?;
            if store
                .compare_and_set_variable(
                    scope,
                    &self.context.identity.script_id,
                    name,
                    expected_version,
                    &next,
                )
                .map_err(RuntimeError::State)?
            {
                self.set_variable(name.to_owned(), next);
                return Ok(());
            }
        }

        Err(RuntimeError::State(format!(
            "variable {name:?} changed too frequently to update safely"
        )))
    }

    fn calculate_variable_operation_value(
        &self,
        node: &RuntimeNode,
        name: &str,
        operation: &str,
        value_type: &str,
        current: Option<Value>,
    ) -> Result<Value, RuntimeError> {
        match operation {
            "set" => {
                let raw_value = self.resolve_variable_input(node.config.get("value"));
                coerce_variable_value(node, raw_value, value_type)
            }
            "increment" => {
                let increment_value = self.resolve_variable_input(node.config.get("value"));
                let increment = number_from_value(Some(&increment_value)).ok_or_else(|| {
                    RuntimeError::VariableOperation {
                        node_id: node.id.clone(),
                        message: "increment value must resolve to a finite number".to_owned(),
                    }
                })?;
                let current = number_from_value(current.as_ref()).unwrap_or(0.0);
                number_value(node, current + increment)
            }
            "append_list" => {
                let mut list = match current {
                    Some(Value::Array(values)) => values,
                    Some(other) => {
                        return Err(RuntimeError::VariableOperation {
                            node_id: node.id.clone(),
                            message: format!(
                                "append_list requires existing variable {name} to be a list, found {}",
                                value_kind(&other)
                            ),
                        });
                    }
                    None => Vec::new(),
                };
                list.push(self.resolve_json_compatible_input(node.config.get("value"))?);
                Ok(Value::Array(list))
            }
            "set_object_field" => {
                let field_path = required_config_string(node, "fieldPath")?;
                let value = self.resolve_json_compatible_input(node.config.get("value"))?;
                let mut current = current.unwrap_or_else(|| Value::Object(Map::new()));
                set_object_field(node, &mut current, &field_path, value)?;
                Ok(current)
            }
            "clear" => Ok(empty_value_for_type(value_type)),
            _ => Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("unsupported variable operation {operation}"),
            }),
        }
    }

    fn resolve_variable_input(&self, value: Option<&Value>) -> Value {
        match value.cloned().unwrap_or(Value::Null) {
            Value::String(template) => resolve_template_value(&template, &self.context.variables),
            value => value,
        }
    }

    fn resolve_json_compatible_input(&self, value: Option<&Value>) -> Result<Value, RuntimeError> {
        let resolved = self.resolve_variable_input(value);
        match resolved {
            Value::String(text) => match serde_json::from_str(text.trim()) {
                Ok(value) => Ok(value),
                Err(_) => Ok(Value::String(text)),
            },
            value => Ok(value),
        }
    }

    fn execute_external_action(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        self.ensure_not_cancelled()?;
        let request = RuntimeActionRequest {
            action: node.action.clone(),
            action_type: node.action_type.clone(),
            config: resolve_config_map(&node.config, &self.context.variables),
            node_id: node.id.clone(),
        };

        let result = self
            .action_handler
            .execute_action(&request, &self.context)
            .map_err(|source| RuntimeError::Action {
                node_id: node.id.clone(),
                message: source.to_string(),
            })?;
        self.ensure_not_cancelled()?;

        for (key, value) in result.output_data {
            self.set_variable(format!("{}.{}", node.id, key), value);
        }

        self.push_runtime_log(
            "info",
            format!("Action {} completed.", node.action_type),
            Some(node.id.clone()),
        );
        Ok(())
    }

    fn execute_delay(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        let amount = number_from_value(node.config.get("amount"))
            .or_else(|| number_from_value(node.config.get("every")))
            .unwrap_or(0.0);
        let unit = config_string(&node.config, "unit").unwrap_or_else(|| "seconds".to_owned());
        let duration = duration_from_amount(amount, &unit);
        self.push_runtime_log(
            "info",
            format!("Delay started for {} ms.", duration.as_millis()),
            Some(node.id.clone()),
        );
        if self.cancellation.wait_for(duration) {
            return Err(RuntimeError::Cancelled);
        }
        self.push_runtime_log(
            "info",
            format!("Delay completed after {} ms.", duration.as_millis()),
            Some(node.id.clone()),
        );
        Ok(())
    }

    fn ensure_not_cancelled(&self) -> Result<(), RuntimeError> {
        if self.cancellation.is_cancelled() {
            Err(RuntimeError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn execute_calculate(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        let expression = config_string(&node.config, "expression").unwrap_or_default();
        let rendered = render_template(&expression, &self.context.variables);
        let result = evaluate_calculation_expression(&rendered).map_err(|message| {
            RuntimeError::Calculation {
                node_id: node.id.clone(),
                message,
            }
        })?;
        let value = number_value(node, result)?;
        self.set_variable(format!("{}.result", node.id), value.clone());
        self.push_runtime_log(
            "info",
            format!(
                "Calculation {} completed with result {}.",
                node.id,
                value_to_string(&value)
            ),
            Some(node.id.clone()),
        );
        Ok(())
    }

    fn evaluate_conditions(&self, node: &RuntimeNode) -> Result<bool, RuntimeError> {
        let conditions = node
            .config
            .get("conditions")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        let rows =
            serde_json::from_value::<Vec<RuntimeConditionRow>>(conditions).map_err(|source| {
                RuntimeError::ControlFlow {
                    node_id: node.id.clone(),
                    message: format!("condition rows are malformed: {source}"),
                }
            })?;

        if rows.is_empty() {
            return Ok(false);
        }

        let mut result = false;
        for (index, row) in rows.iter().enumerate() {
            let compared = compare_condition_values(
                &resolve_template_value(&row.left, &self.context.variables),
                &row.operator,
                &resolve_template_value(&row.right, &self.context.variables),
            )
            .map_err(|message| RuntimeError::ControlFlow {
                node_id: node.id.clone(),
                message,
            })?;
            let row_result = if row.invert { !compared } else { compared };

            if index == 0 {
                result = row_result;
                continue;
            }

            result = match row.combinator.as_deref() {
                Some("or") => result || row_result,
                Some("and") | None => result && row_result,
                Some(other) => {
                    return Err(RuntimeError::ControlFlow {
                        node_id: node.id.clone(),
                        message: format!("unsupported condition combinator {other}"),
                    });
                }
            };
        }

        Ok(result)
    }

    fn evaluate_switch(&mut self, node: &RuntimeNode) -> Result<Option<String>, RuntimeError> {
        let switch_value = resolve_template_value(
            &config_string(&node.config, "value").unwrap_or_default(),
            &self.context.variables,
        );
        let cases = node
            .config
            .get("cases")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        let cases =
            serde_json::from_value::<Vec<RuntimeSwitchCaseRow>>(cases).map_err(|source| {
                RuntimeError::ControlFlow {
                    node_id: node.id.clone(),
                    message: format!("switch cases are malformed: {source}"),
                }
            })?;

        for switch_case in cases {
            let raw_case_value = switch_case
                .value
                .or(switch_case.expected_value)
                .unwrap_or_default();
            let case_value = resolve_template_value(&raw_case_value, &self.context.variables);
            if values_equal_for_condition(&switch_value, &case_value) {
                self.push_runtime_log(
                    "info",
                    format!(
                        "Switch {} matched case \"{}\".",
                        node.id,
                        if switch_case.name.trim().is_empty() {
                            switch_case.id.as_str()
                        } else {
                            switch_case.name.as_str()
                        }
                    ),
                    Some(node.id.clone()),
                );
                return Ok(Some(format!("case-{}", switch_case.id)));
            }
        }

        Ok(None)
    }

    fn loop_count(&self, node: &RuntimeNode) -> Result<u64, RuntimeError> {
        let raw_count = node.config.get("count").cloned().unwrap_or(Value::Null);
        let value = match raw_count {
            Value::String(template) => resolve_template_value(&template, &self.context.variables),
            other => other,
        };
        let count = number_from_value(Some(&value)).unwrap_or(0.0);
        Ok(count.max(0.0).trunc() as u64)
    }

    fn for_each_items(&self, node: &RuntimeNode) -> Result<Vec<Value>, RuntimeError> {
        let raw_items = required_config_string(node, "items")?;
        let value = resolve_template_value(&raw_items, &self.context.variables);
        if let Value::Array(items) = value {
            return Ok(items);
        }

        if let Value::String(text) = &value
            && let Ok(Value::Array(items)) = serde_json::from_str::<Value>(text.trim())
        {
            return Ok(items);
        }

        Err(RuntimeError::ControlFlow {
            node_id: node.id.clone(),
            message: format!(
                "for-each items must resolve to a list, found {}",
                value_kind(&value)
            ),
        })
    }

    fn default_success_handle(&self, node: &RuntimeNode) -> Option<String> {
        self.graph
            .first_available_output_handle(&node.id, &["success", "out"])
    }

    fn set_variable(&mut self, name: String, value: Value) {
        self.context.variables.insert(name.clone(), value);
        refresh_derived_variable_metadata(&mut self.context.variables, &name);
    }

    fn has_secrets(&self) -> bool {
        !self.secret_values.is_empty()
    }

    fn redact_report(&self, mut report: RunReport) -> RunReport {
        for name in &self.secret_names {
            report.variables.remove(name);
            for suffix in [".$length", ".$count", ".$type", ".$is_empty"] {
                report.variables.remove(&format!("{name}{suffix}"));
            }
        }
        for value in report.variables.values_mut() {
            self.redact_value(value);
        }
        for log in &mut report.logs {
            log.message = self.redact_text(&log.message);
        }
        report
    }

    fn redact_value(&self, value: &mut Value) {
        if self.secret_values.iter().any(|secret| secret == value) {
            *value = Value::String("[REDACTED]".to_owned());
            return;
        }
        match value {
            Value::String(text) => *text = self.redact_text(text),
            Value::Array(values) => {
                for value in values {
                    self.redact_value(value);
                }
            }
            Value::Object(values) => {
                for value in values.values_mut() {
                    self.redact_value(value);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }

    fn redact_text(&self, text: &str) -> String {
        self.secret_values
            .iter()
            .fold(text.to_owned(), |redacted, value| {
                let sensitive = value_to_string(value);
                if sensitive.is_empty() {
                    redacted
                } else {
                    redacted.replace(&sensitive, "[REDACTED]")
                }
            })
    }

    fn push_runtime_log(
        &mut self,
        level: &str,
        message: impl Into<String>,
        node_id: Option<String>,
    ) {
        self.logs.push(RuntimeLogEntry {
            level: level.to_owned(),
            message: message.into(),
            node_id,
        });
    }
}

fn create_run_id(script_id: &str, trigger_node_id: &str) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{script_id}:{trigger_node_id}:{timestamp}")
}

fn load_initial_state(
    graph: &RuntimeGraph,
    script_id: &str,
    state_store: Option<&dyn RuntimeStateStore>,
    secrets: &[RuntimeSecretDeclaration],
) -> Result<InitialRuntimeState, RuntimeError> {
    let mut variables = BTreeMap::new();
    let mut declarations = BTreeMap::<String, RuntimeVariableScope>::new();
    let mut declared_scopes = BTreeMap::<String, String>::new();

    for node in graph
        .nodes()
        .filter(|node| node.action_type == "runtime.set_variable")
    {
        let name = required_config_string(node, "name")?;
        validate_variable_name(node, &name)?;
        let scope_name = required_config_string(node, "scope")?;
        let scope = match scope_name.as_str() {
            "runtime" => None,
            "persistent" => Some(RuntimeVariableScope::Persistent),
            "global" => Some(RuntimeVariableScope::Global),
            invalid => {
                return Err(RuntimeError::VariableOperation {
                    node_id: node.id.clone(),
                    message: format!("unsupported variable scope {invalid}"),
                });
            }
        };
        if let Some(existing) = declared_scopes.insert(name.clone(), scope_name.clone())
            && existing != scope_name
        {
            return Err(RuntimeError::InvalidGraph(format!(
                "variable {name:?} is declared with conflicting scopes {existing:?} and {scope_name:?}"
            )));
        }
        if let Some(scope) = scope {
            declarations.insert(name.clone(), scope);
        }
    }

    let secret_names = secrets
        .iter()
        .map(|secret| secret.name.clone())
        .collect::<Vec<_>>();
    let unique_secret_names = secret_names
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    if unique_secret_names.len() != secret_names.len() {
        return Err(RuntimeError::InvalidGraph(
            "manifest contains duplicate secret declarations".to_owned(),
        ));
    }
    if let Some(collision) = secret_names
        .iter()
        .find(|name| declared_scopes.contains_key(name.as_str()))
    {
        return Err(RuntimeError::InvalidGraph(format!(
            "secret {collision:?} conflicts with a writable variable"
        )));
    }

    if (!declarations.is_empty() || !secrets.is_empty()) && state_store.is_none() {
        return Err(RuntimeError::State(
            "persistent, global, and secret variables require a runner state store".to_owned(),
        ));
    }
    let mut secret_values = Vec::new();
    if let Some(store) = state_store {
        for (name, scope) in declarations {
            if let Some(stored) = store
                .load_variable(scope, script_id, &name)
                .map_err(RuntimeError::State)?
            {
                variables.insert(name.clone(), stored.value);
                refresh_derived_variable_metadata(&mut variables, &name);
            }
        }
        for secret in secrets {
            match store
                .read_secret(script_id, &secret.name)
                .map_err(RuntimeError::State)?
            {
                Some(value) => {
                    validate_secret_value(secret, &value)?;
                    variables.insert(secret.name.clone(), value.clone());
                    secret_values.push(value);
                }
                None if secret.required => {
                    return Err(RuntimeError::State(format!(
                        "required secret {:?} is not configured",
                        secret.name
                    )));
                }
                None => {}
            }
        }
    }

    Ok(InitialRuntimeState {
        secret_names,
        secret_values,
        variables,
    })
}

fn validate_secret_value(
    declaration: &RuntimeSecretDeclaration,
    value: &Value,
) -> Result<(), RuntimeError> {
    let valid = match declaration.value_type.as_str() {
        "string" | "file_path" => value.is_string(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "list" => value.is_array(),
        "object" | "http_response" | "datetime" | "duration" => value.is_object(),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(RuntimeError::State(format!(
            "secret {:?} does not match declared type {}",
            declaration.name, declaration.value_type
        )))
    }
}

#[cfg(test)]
mod tests;
