//! Runtime primitives for executing BaudBound script graphs.

use std::{collections::BTreeMap, path::PathBuf, thread, time::Duration};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use thiserror::Error;

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

#[derive(Debug, Clone, Deserialize)]
pub struct ProgramEnvelope {
    pub entry: ProgramEntry,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProgramEntry {
    pub trigger: RuntimeNode,
    #[serde(default)]
    pub triggers: Vec<RuntimeNode>,
    pub program: ProgramBlock,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProgramBlock {
    #[serde(default)]
    pub steps: Vec<RuntimeNode>,
    #[serde(default)]
    pub edges: Vec<RuntimeEdge>,
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

#[derive(Debug, Clone, Deserialize)]
struct RuntimeConditionRow {
    #[serde(default)]
    invert: bool,
    left: String,
    #[serde(default)]
    combinator: Option<String>,
    operator: String,
    right: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RuntimeSwitchCaseRow {
    id: String,
    name: String,
    #[serde(default)]
    value: Option<String>,
    #[serde(default, alias = "expectedValue")]
    expected_value: Option<String>,
}

enum RuntimeFrame {
    Follow {
        source_node_id: String,
        handle: String,
        stop_at_node_id: Option<String>,
    },
    ForEach {
        node_id: String,
        index: usize,
        items: Vec<Value>,
    },
    Loop {
        node_id: String,
        index: u64,
        count: u64,
    },
    Node {
        node_id: String,
        stop_at_node_id: Option<String>,
    },
    While {
        node_id: String,
        index: u64,
    },
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
    let graph = RuntimeGraph::from_program_value(program)?;
    let trigger_node_id = graph.manual_trigger()?.id.clone();
    execute_graph_from_trigger(
        graph,
        script_id,
        &trigger_node_id,
        package_path,
        Value::Null,
        action_handler,
    )
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
    let graph = RuntimeGraph::from_program_value(program)?;
    execute_graph_from_trigger(
        graph,
        script_id,
        trigger_node_id,
        package_path,
        trigger_payload,
        action_handler,
    )
}

fn execute_graph_from_trigger(
    graph: RuntimeGraph,
    script_id: &str,
    trigger_node_id: &str,
    package_path: Option<PathBuf>,
    trigger_payload: Value,
    action_handler: &dyn RuntimeActionHandler,
) -> Result<RunReport, RuntimeError> {
    let trigger = graph.trigger(trigger_node_id)?;
    let identity = RunIdentity {
        run_id: create_run_id(script_id, &trigger.id),
        script_id: script_id.to_owned(),
        trigger_node_id: trigger.id.clone(),
    };
    let mut executor = RuntimeExecutor::new(
        graph,
        identity,
        package_path,
        trigger_payload,
        action_handler,
    );
    executor.run_from_trigger()
}

struct RuntimeGraph {
    nodes: BTreeMap<String, RuntimeNode>,
    edges_by_source: BTreeMap<String, Vec<RuntimeEdge>>,
    trigger_ids: Vec<String>,
}

impl RuntimeGraph {
    fn from_program_value(value: &Value) -> Result<Self, RuntimeError> {
        let envelope = serde_json::from_value::<ProgramEnvelope>(value.clone())
            .map_err(|source| RuntimeError::InvalidGraph(source.to_string()))?;

        let mut nodes = BTreeMap::new();
        let trigger_ids = if envelope.entry.triggers.is_empty() {
            vec![envelope.entry.trigger.id.clone()]
        } else {
            envelope
                .entry
                .triggers
                .iter()
                .map(|trigger| trigger.id.clone())
                .collect()
        };

        insert_node(&mut nodes, envelope.entry.trigger.clone())?;
        for trigger in envelope.entry.triggers {
            if trigger.id == envelope.entry.trigger.id {
                continue;
            }
            insert_node(&mut nodes, trigger)?;
        }
        for step in envelope.entry.program.steps {
            insert_node(&mut nodes, step)?;
        }

        let mut edges_by_source = BTreeMap::<String, Vec<RuntimeEdge>>::new();
        for edge in envelope.entry.program.edges {
            if !nodes.contains_key(&edge.source) {
                return Err(RuntimeError::InvalidGraph(format!(
                    "edge source {} does not exist",
                    edge.source
                )));
            }
            if !nodes.contains_key(&edge.target) {
                return Err(RuntimeError::InvalidGraph(format!(
                    "edge target {} does not exist",
                    edge.target
                )));
            }
            edges_by_source
                .entry(edge.source.clone())
                .or_default()
                .push(edge);
        }

        Ok(Self {
            nodes,
            edges_by_source,
            trigger_ids,
        })
    }

    fn manual_trigger(&self) -> Result<&RuntimeNode, RuntimeError> {
        self.trigger_ids
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .find(|node| node.action_type == "trigger.manual")
            .ok_or_else(|| RuntimeError::InvalidGraph("no manual trigger exists".to_owned()))
    }

    fn trigger(&self, node_id: &str) -> Result<&RuntimeNode, RuntimeError> {
        if !self.trigger_ids.iter().any(|id| id == node_id) {
            return Err(RuntimeError::InvalidGraph(format!(
                "node {node_id} is not a registered trigger"
            )));
        }
        self.node(node_id)
    }

    fn node(&self, node_id: &str) -> Result<&RuntimeNode, RuntimeError> {
        self.nodes
            .get(node_id)
            .ok_or_else(|| RuntimeError::InvalidGraph(format!("node {node_id} does not exist")))
    }

    fn target_node_ids_for_handle(&self, node_id: &str, handle: &str) -> Vec<String> {
        let mut targets = self
            .edges_by_source
            .get(node_id)
            .into_iter()
            .flat_map(|edges| edges.iter())
            .filter(|edge| edge.source_handle == handle)
            .map(|edge| edge.target.clone())
            .collect::<Vec<_>>();
        targets.sort();
        targets
    }

    fn first_available_output_handle<'a>(
        &'a self,
        node_id: &str,
        preferred_handles: &[&'a str],
    ) -> Option<String> {
        let edges = self.edges_by_source.get(node_id)?;
        for handle in preferred_handles {
            if edges.iter().any(|edge| edge.source_handle == *handle) {
                return Some((*handle).to_owned());
            }
        }
        edges.first().map(|edge| edge.source_handle.clone())
    }
}

struct RuntimeExecutor<'a> {
    graph: RuntimeGraph,
    context: RuntimeContext,
    logs: Vec<RuntimeLogEntry>,
    action_handler: &'a dyn RuntimeActionHandler,
}

impl<'a> RuntimeExecutor<'a> {
    fn new(
        graph: RuntimeGraph,
        identity: RunIdentity,
        package_path: Option<PathBuf>,
        trigger_payload: Value,
        action_handler: &'a dyn RuntimeActionHandler,
    ) -> Self {
        Self {
            graph,
            context: RuntimeContext {
                identity,
                package_path,
                trigger_payload,
                variables: BTreeMap::new(),
            },
            logs: Vec::new(),
            action_handler,
        }
    }

    fn run_from_trigger(&mut self) -> Result<RunReport, RuntimeError> {
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

        let operation =
            config_string(&node.config, "operation").unwrap_or_else(|| "set".to_owned());
        let value_type =
            config_string(&node.config, "valueType").unwrap_or_else(|| "string".to_owned());

        match operation.as_str() {
            "set" => {
                let raw_value = node.config.get("value").cloned().unwrap_or(Value::Null);
                let value = coerce_variable_value(node, raw_value, &value_type)?;
                self.set_variable(name, value);
            }
            "increment" => {
                let increment = number_from_value(node.config.get("value")).unwrap_or(1.0);
                let current = number_from_value(self.context.variables.get(&name)).unwrap_or(0.0);
                self.set_variable(name, number_value(node, current + increment)?);
            }
            "append_list" => {
                let mut list = match self.context.variables.remove(&name) {
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
                list.push(node.config.get("value").cloned().unwrap_or(Value::Null));
                self.set_variable(name, Value::Array(list));
            }
            "set_object_field" => {
                let field_path = required_config_string(node, "fieldPath")?;
                let raw_value = node.config.get("value").cloned().unwrap_or(Value::Null);
                let value = coerce_variable_value(node, raw_value, &value_type)?;
                let current = self
                    .context
                    .variables
                    .entry(name.clone())
                    .or_insert_with(|| Value::Object(Map::new()));
                set_object_field(node, current, &field_path, value)?;
                refresh_derived_variable_metadata(&mut self.context.variables, &name);
            }
            "clear" => {
                self.set_variable(name, empty_value_for_type(&value_type));
            }
            _ => {
                return Err(RuntimeError::VariableOperation {
                    node_id: node.id.clone(),
                    message: format!("unsupported variable operation {operation}"),
                });
            }
        }

        self.push_runtime_log(
            "debug",
            format!("Variable operation {operation} completed."),
            Some(node.id.clone()),
        );
        Ok(())
    }

    fn execute_external_action(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
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
        thread::sleep(duration);
        self.push_runtime_log(
            "info",
            format!("Delay completed after {} ms.", duration.as_millis()),
            Some(node.id.clone()),
        );
        Ok(())
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

        if let Value::Object(fields) = value {
            return Ok(fields.into_values().collect());
        }

        let text = value_to_string(&value);
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(text.trim()) {
            return Ok(items);
        }

        Ok(text
            .split([',', '\n', '\r'])
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| Value::String(item.to_owned()))
            .collect())
    }

    fn default_success_handle(&self, node: &RuntimeNode) -> Option<String> {
        self.graph
            .first_available_output_handle(&node.id, &["success", "out"])
    }

    fn set_variable(&mut self, name: String, value: Value) {
        self.context.variables.insert(name.clone(), value);
        refresh_derived_variable_metadata(&mut self.context.variables, &name);
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

fn insert_node(
    nodes: &mut BTreeMap<String, RuntimeNode>,
    node: RuntimeNode,
) -> Result<(), RuntimeError> {
    if node.id.is_empty() {
        return Err(RuntimeError::InvalidGraph(
            "node id must not be empty".to_owned(),
        ));
    }
    if nodes.insert(node.id.clone(), node).is_some() {
        return Err(RuntimeError::InvalidGraph(
            "duplicate node id exists in graph".to_owned(),
        ));
    }
    Ok(())
}

fn config_string(config: &Map<String, Value>, key: &str) -> Option<String> {
    match config.get(key) {
        Some(Value::String(value)) => Some(value.clone()),
        Some(value) => Some(value_to_string(value)),
        None => None,
    }
}

fn required_config_string(node: &RuntimeNode, key: &str) -> Result<String, RuntimeError> {
    config_string(&node.config, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!("missing required config field {key}"),
        })
}

fn validate_variable_name(node: &RuntimeNode, name: &str) -> Result<(), RuntimeError> {
    if name.starts_with("system_")
        || name.starts_with("manifest_")
        || name.ends_with(".$length")
        || name.ends_with(".$count")
    {
        return Err(RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!("{name} is read-only or reserved"),
        });
    }
    Ok(())
}

fn coerce_variable_value(
    node: &RuntimeNode,
    value: Value,
    value_type: &str,
) -> Result<Value, RuntimeError> {
    match value_type {
        "string" | "file_content" | "file_path" | "datetime" | "duration" => {
            Ok(Value::String(value_to_string(&value)))
        }
        "number" | "http_status_code" | "duration_ms" | "process_id" | "exit_code" => {
            number_from_value(Some(&value))
                .and_then(Number::from_f64)
                .map(Value::Number)
                .ok_or_else(|| RuntimeError::VariableOperation {
                    node_id: node.id.clone(),
                    message: format!("expected number, found {}", value_kind(&value)),
                })
        }
        "boolean" => match value {
            Value::Bool(value) => Ok(Value::Bool(value)),
            Value::String(value) if value.eq_ignore_ascii_case("true") => Ok(Value::Bool(true)),
            Value::String(value) if value.eq_ignore_ascii_case("false") => Ok(Value::Bool(false)),
            other => Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("expected boolean, found {}", value_kind(&other)),
            }),
        },
        "object" | "http_response" | "http_headers" => match value {
            Value::Object(_) => Ok(value),
            other => Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("expected object, found {}", value_kind(&other)),
            }),
        },
        "list" => match value {
            Value::Array(_) => Ok(value),
            other => Err(RuntimeError::VariableOperation {
                node_id: node.id.clone(),
                message: format!("expected list, found {}", value_kind(&other)),
            }),
        },
        "keyboard_key" => Ok(Value::String(value_to_string(&value))),
        _ => Err(RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!("unsupported variable type {value_type}"),
        }),
    }
}

fn set_object_field(
    node: &RuntimeNode,
    target: &mut Value,
    field_path: &str,
    value: Value,
) -> Result<(), RuntimeError> {
    let segments = field_path
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return Err(RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: "fieldPath must contain at least one segment".to_owned(),
        });
    }

    let mut current = target;
    for segment in &segments[..segments.len() - 1] {
        if !current.is_object() {
            *current = Value::Object(Map::new());
        }
        current = current
            .as_object_mut()
            .expect("current value was just converted to object")
            .entry((*segment).to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
    }

    if !current.is_object() {
        *current = Value::Object(Map::new());
    }
    current
        .as_object_mut()
        .expect("current value was just converted to object")
        .insert(
            segments
                .last()
                .expect("segments is known to be non-empty")
                .to_string(),
            value,
        );
    Ok(())
}

fn refresh_derived_variable_metadata(variables: &mut BTreeMap<String, Value>, name: &str) {
    let length_key = format!("{name}.$length");
    let count_key = format!("{name}.$count");
    variables.remove(&length_key);
    variables.remove(&count_key);

    let derived = variables.get(name).and_then(|value| match value {
        Value::String(value) => Some((Some(value.len()), None)),
        Value::Array(values) => Some((Some(values.len()), Some(values.len()))),
        Value::Object(fields) => Some((Some(fields.len()), Some(fields.len()))),
        _ => None,
    });

    if let Some((length, count)) = derived {
        if let Some(length) = length {
            variables.insert(length_key, Value::Number(length.into()));
        }
        if let Some(count) = count {
            variables.insert(count_key, Value::Number(count.into()));
        }
    }
}

fn empty_value_for_type(value_type: &str) -> Value {
    match value_type {
        "number" | "http_status_code" | "duration_ms" | "process_id" | "exit_code" => {
            Value::Number(0.into())
        }
        "boolean" => Value::Bool(false),
        "object" | "http_response" | "http_headers" => Value::Object(Map::new()),
        "list" => Value::Array(Vec::new()),
        _ => Value::String(String::new()),
    }
}

fn render_template(template: &str, variables: &BTreeMap<String, Value>) -> String {
    let mut output = String::new();
    let mut remaining = template;

    while let Some(start_index) = remaining.find("{{") {
        let (before, after_start) = remaining.split_at(start_index);
        output.push_str(before);
        let after_start = &after_start[2..];

        let Some(end_index) = after_start.find("}}") else {
            output.push_str("{{");
            output.push_str(after_start);
            return output;
        };

        let expression = after_start[..end_index].trim();
        if let Some(value) = resolve_variable_expression(expression, variables) {
            output.push_str(&value_to_string(value));
        } else {
            output.push_str("{{");
            output.push_str(expression);
            output.push_str("}}");
        }

        remaining = &after_start[end_index + 2..];
    }

    output.push_str(remaining);
    output
}

fn resolve_template_value(template: &str, variables: &BTreeMap<String, Value>) -> Value {
    let trimmed = template.trim();
    if let Some(expression) = trimmed
        .strip_prefix("{{")
        .and_then(|value| value.strip_suffix("}}"))
        .filter(|expression| !expression.contains("{{") && !expression.contains("}}"))
    {
        return resolve_variable_expression(expression.trim(), variables)
            .cloned()
            .unwrap_or_else(|| Value::String(trimmed.to_owned()));
    }

    Value::String(render_template(template, variables))
}

fn resolve_config_map(
    config: &Map<String, Value>,
    variables: &BTreeMap<String, Value>,
) -> Map<String, Value> {
    config
        .iter()
        .map(|(key, value)| (key.clone(), resolve_config_value(value, variables)))
        .collect()
}

fn resolve_config_value(value: &Value, variables: &BTreeMap<String, Value>) -> Value {
    match value {
        Value::String(value) => resolve_template_value(value, variables),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| resolve_config_value(value, variables))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(resolve_config_map(fields, variables)),
        other => other.clone(),
    }
}

fn compare_condition_values(left: &Value, operator: &str, right: &Value) -> Result<bool, String> {
    let left_text = value_to_string(left);
    let right_text = value_to_string(right);
    let left_number = number_from_value(Some(left));
    let right_number = number_from_value(Some(right));

    match operator {
        "==" => Ok(values_equal_for_condition(left, right)),
        "!=" => Ok(!values_equal_for_condition(left, right)),
        ">" => compare_numbers(left_number, right_number, |left, right| left > right),
        ">=" => compare_numbers(left_number, right_number, |left, right| left >= right),
        "<" => compare_numbers(left_number, right_number, |left, right| left < right),
        "<=" => compare_numbers(left_number, right_number, |left, right| left <= right),
        "contains" => Ok(left_text.contains(&right_text)),
        "starts_with" => Ok(left_text.starts_with(&right_text)),
        "ends_with" => Ok(left_text.ends_with(&right_text)),
        "regex_match" => safe_regex_match(&left_text, &right_text),
        "is_empty" => Ok(is_value_empty(left)),
        "is_null" => Ok(left.is_null()),
        other => Err(format!("unsupported comparison operator {other}")),
    }
}

fn values_equal_for_condition(left: &Value, right: &Value) -> bool {
    if left == right {
        return true;
    }

    value_to_string(left) == value_to_string(right)
}

fn compare_numbers(
    left: Option<f64>,
    right: Option<f64>,
    compare: impl FnOnce(f64, f64) -> bool,
) -> Result<bool, String> {
    match (left, right) {
        (Some(left), Some(right)) => Ok(compare(left, right)),
        _ => Err("numeric comparison requires numeric values".to_owned()),
    }
}

fn safe_regex_match(value: &str, pattern: &str) -> Result<bool, String> {
    const MAX_REGEX_PATTERN_LENGTH: usize = 256;
    if pattern.len() > MAX_REGEX_PATTERN_LENGTH {
        return Err(format!(
            "regex pattern exceeds {MAX_REGEX_PATTERN_LENGTH} characters"
        ));
    }

    Regex::new(pattern)
        .map(|regex| regex.is_match(value))
        .map_err(|source| format!("invalid regex pattern: {source}"))
}

fn is_value_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(value) => value.is_empty(),
        Value::Array(values) => values.is_empty(),
        Value::Object(fields) => fields.is_empty(),
        Value::Bool(_) | Value::Number(_) => false,
    }
}

fn resolve_variable_expression<'a>(
    expression: &str,
    variables: &'a BTreeMap<String, Value>,
) -> Option<&'a Value> {
    let mut best_name = None;
    for name in variables.keys() {
        if (expression == name || expression.starts_with(&format!("{name}.")))
            && best_name.is_none_or(|best: &String| name.len() > best.len())
        {
            best_name = Some(name);
        }
    }

    let name = best_name?;
    let value = variables.get(name)?;
    let suffix = expression.strip_prefix(name.as_str()).unwrap_or_default();
    if suffix == "." {
        return None;
    }
    let path = suffix.strip_prefix('.').unwrap_or_default();
    resolve_value_path(value, path)
}

fn resolve_value_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }

    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }
        current = match current {
            Value::Object(fields) => fields.get(segment)?,
            Value::Array(values) => values.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}

#[derive(Debug, Clone, PartialEq)]
enum CalculationToken {
    Comma,
    Identifier(String),
    Number(f64),
    Operator(char),
    Paren(char),
}

fn evaluate_calculation_expression(expression: &str) -> Result<f64, String> {
    let tokens = tokenize_calculation_expression(expression)?;
    let mut parser = CalculationParser { index: 0, tokens };
    let value = parser.parse_expression()?;
    if !parser.is_complete() {
        return Err("expression contains trailing tokens".to_owned());
    }
    if value.is_finite() {
        Ok(value)
    } else {
        Err("expression result must be finite".to_owned())
    }
}

fn tokenize_calculation_expression(expression: &str) -> Result<Vec<CalculationToken>, String> {
    let chars = expression.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < chars.len() {
        let character = chars[index];
        if character.is_whitespace() {
            index += 1;
            continue;
        }

        if character == '(' || character == ')' {
            tokens.push(CalculationToken::Paren(character));
            index += 1;
            continue;
        }

        if character == ',' {
            tokens.push(CalculationToken::Comma);
            index += 1;
            continue;
        }

        if matches!(character, '+' | '-' | '*' | '/' | '%' | '^') {
            tokens.push(CalculationToken::Operator(character));
            index += 1;
            continue;
        }

        if character.is_ascii_digit() || character == '.' {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_digit()
                    || chars[index] == '.'
                    || chars[index] == 'e'
                    || chars[index] == 'E'
                    || ((chars[index] == '+' || chars[index] == '-')
                        && matches!(chars.get(index.wrapping_sub(1)), Some('e' | 'E'))))
            {
                index += 1;
            }
            let raw = chars[start..index].iter().collect::<String>();
            let value = raw
                .parse::<f64>()
                .map_err(|_| format!("invalid number \"{raw}\""))?;
            if !value.is_finite() {
                return Err(format!("invalid number \"{raw}\""));
            }
            tokens.push(CalculationToken::Number(value));
            continue;
        }

        if character.is_ascii_alphabetic() || character == '_' {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
            {
                index += 1;
            }
            tokens.push(CalculationToken::Identifier(
                chars[start..index]
                    .iter()
                    .collect::<String>()
                    .to_lowercase(),
            ));
            continue;
        }

        return Err(format!("unexpected token \"{character}\""));
    }

    if tokens.is_empty() {
        Err("expression is required".to_owned())
    } else {
        Ok(tokens)
    }
}

struct CalculationParser {
    index: usize,
    tokens: Vec<CalculationToken>,
}

impl CalculationParser {
    fn is_complete(&self) -> bool {
        self.index >= self.tokens.len()
    }

    fn parse_expression(&mut self) -> Result<f64, String> {
        let mut left = self.parse_term()?;
        loop {
            if self.match_operator('+') {
                left += self.parse_term()?;
            } else if self.match_operator('-') {
                left -= self.parse_term()?;
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_term(&mut self) -> Result<f64, String> {
        let mut left = self.parse_unary()?;
        loop {
            if self.match_operator('*') {
                left *= self.parse_unary()?;
            } else if self.match_operator('/') {
                let right = self.parse_unary()?;
                if right == 0.0 {
                    return Err("division by zero is not allowed".to_owned());
                }
                left /= right;
            } else if self.match_operator('%') {
                let right = self.parse_unary()?;
                if right == 0.0 {
                    return Err("division by zero is not allowed".to_owned());
                }
                left %= right;
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_unary(&mut self) -> Result<f64, String> {
        if self.match_operator('-') {
            return Ok(-self.parse_unary()?);
        }
        if self.match_operator('+') {
            return self.parse_unary();
        }
        self.parse_power()
    }

    fn parse_power(&mut self) -> Result<f64, String> {
        let left = self.parse_primary()?;
        if !self.match_operator('^') {
            return Ok(left);
        }

        let right = self.parse_unary()?;
        let value = left.powf(right);
        if value.is_finite() {
            Ok(value)
        } else {
            Err("exponent result must be finite".to_owned())
        }
    }

    fn parse_primary(&mut self) -> Result<f64, String> {
        match self.advance() {
            Some(CalculationToken::Number(value)) => Ok(value),
            Some(CalculationToken::Paren('(')) => {
                let value = self.parse_expression()?;
                if !self.match_paren(')') {
                    return Err("missing closing parenthesis".to_owned());
                }
                Ok(value)
            }
            Some(CalculationToken::Identifier(name)) => self.parse_function_call(name),
            Some(token) => Err(format!("unexpected token {token:?}")),
            None => Err("expression ended unexpectedly".to_owned()),
        }
    }

    fn parse_function_call(&mut self, name: String) -> Result<f64, String> {
        if !self.match_paren('(') {
            return Err(format!(
                "function \"{name}\" must be called with parentheses"
            ));
        }

        let mut args = Vec::new();
        if !self.match_paren(')') {
            loop {
                args.push(self.parse_expression()?);
                if !self.match_comma() {
                    break;
                }
            }

            if !self.match_paren(')') {
                return Err(format!(
                    "function \"{name}\" is missing a closing parenthesis"
                ));
            }
        }

        evaluate_calculation_function(&name, &args)
    }

    fn match_operator(&mut self, operator: char) -> bool {
        if matches!(self.peek(), Some(CalculationToken::Operator(value)) if *value == operator) {
            self.index += 1;
            return true;
        }
        false
    }

    fn match_paren(&mut self, paren: char) -> bool {
        if matches!(self.peek(), Some(CalculationToken::Paren(value)) if *value == paren) {
            self.index += 1;
            return true;
        }
        false
    }

    fn match_comma(&mut self) -> bool {
        if matches!(self.peek(), Some(CalculationToken::Comma)) {
            self.index += 1;
            return true;
        }
        false
    }

    fn advance(&mut self) -> Option<CalculationToken> {
        let token = self.peek().cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    fn peek(&self) -> Option<&CalculationToken> {
        self.tokens.get(self.index)
    }
}

fn evaluate_calculation_function(name: &str, args: &[f64]) -> Result<f64, String> {
    match name {
        "round" | "floor" | "ceil" => {
            if args.len() != 1 {
                return Err(format!("{name}() expects exactly one argument"));
            }
            let value = args[0];
            Ok(match name {
                "round" => value.round(),
                "floor" => value.floor(),
                _ => value.ceil(),
            })
        }
        "min" | "max" => {
            if args.is_empty() {
                return Err(format!("{name}() expects at least one argument"));
            }
            let mut values = args.iter().copied();
            let first = values.next().expect("args is known to be non-empty");
            Ok(values.fold(first, |current, value| {
                if name == "min" {
                    current.min(value)
                } else {
                    current.max(value)
                }
            }))
        }
        "random" => {
            if args.len() > 2 {
                return Err("random() expects zero, one, or two arguments".to_owned());
            }
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.subsec_nanos() as f64 / 1_000_000_000.0)
                .unwrap_or(0.0);
            Ok(match args {
                [] => seed,
                [max] => seed * max,
                [min, max] => min + seed * (max - min),
                _ => unreachable!("args length is checked above"),
            })
        }
        _ => Err(format!("unknown function \"{name}\"")),
    }
}

fn number_from_value(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(value)) => value.parse::<f64>().ok(),
        _ => None,
    }
}

fn number_value(node: &RuntimeNode, value: f64) -> Result<Value, RuntimeError> {
    Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(|| RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!("{value} cannot be represented as a JSON number"),
        })
}

fn duration_from_amount(amount: f64, unit: &str) -> Duration {
    let milliseconds = match unit {
        "millisecond" | "milliseconds" | "ms" => amount,
        "minute" | "minutes" => amount * 60_000.0,
        "hour" | "hours" => amount * 3_600_000.0,
        _ => amount * 1_000.0,
    };

    Duration::from_millis(milliseconds.max(0.0).round() as u64)
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "list",
        Value::Object(_) => "object",
    }
}

fn create_run_id(script_id: &str, trigger_node_id: &str) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{script_id}:{trigger_node_id}:{timestamp}")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
}
