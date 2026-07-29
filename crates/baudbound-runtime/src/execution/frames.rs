use crate::runtime::RuntimeFrame;
use serde_json::{Map, Number, Value};

use super::{RunVariableScope, RuntimeError, RuntimeExecutor, RuntimeNode};

impl RuntimeExecutor<'_> {
    pub(super) fn process_frame(
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
            RuntimeFrame::Repeat {
                node_id,
                index,
                count,
            } => self.process_repeat_frame(frames, &node_id, index, count),
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
            "control.color_match" => {
                let branch = if self.evaluate_color_match(&node)? {
                    "match"
                } else {
                    "no_match"
                };
                frames.push(RuntimeFrame::Follow {
                    source_node_id: node.id,
                    handle: branch.to_owned(),
                    stop_at_node_id: None,
                });
            }
            "control.if" => {
                let evaluation = self.evaluate_conditions(&node)?;
                let branch = if evaluation.result { "true" } else { "false" };
                self.push_runtime_log(
                    "info",
                    format!(
                        "Evaluated {} If / Else condition row{}. {}. Selected {branch:?} output.",
                        condition_row_count(&node),
                        if condition_row_count(&node) == 1 {
                            ""
                        } else {
                            "s"
                        },
                        evaluation.summary
                    ),
                    Some(node.id.clone()),
                );
                frames.push(RuntimeFrame::Follow {
                    source_node_id: node.id,
                    handle: branch.to_owned(),
                    stop_at_node_id: None,
                });
            }
            "control.switch" => {
                let handle = self.evaluate_switch(&node)?;
                frames.push(RuntimeFrame::Follow {
                    source_node_id: node.id,
                    handle,
                    stop_at_node_id: None,
                });
            }
            "control.repeat" => {
                let count = self.repeat_count(&node)?;
                self.push_runtime_log(
                    "info",
                    format!("Started Repeat with {count} configured iterations."),
                    Some(node.id.clone()),
                );
                frames.push(RuntimeFrame::Repeat {
                    node_id: node.id,
                    index: 0,
                    count,
                });
            }
            "control.break_loop" => self.process_loop_control(frames, &node, true)?,
            "control.continue_loop" => self.process_loop_control(frames, &node, false)?,
            "control.while" => frames.push(RuntimeFrame::While {
                node_id: node.id,
                index: 0,
            }),
            "control.for_each" => {
                let items = self.for_each_items(&node)?;
                frames.push(RuntimeFrame::ForEach {
                    node_id: node.id,
                    index: 0,
                    items,
                });
            }
            _ => {
                let handle = match self.execute_node(&node) {
                    Ok(()) => self.default_success_handle(&node),
                    Err(error) => {
                        let Some(failure) =
                            ExpectedNodeFailure::from_runtime_error(&error, &node.action_type)
                        else {
                            return Err(error);
                        };
                        self.record_expected_node_failure(&node, failure)?;
                        Some("failed".to_owned())
                    }
                };
                let Some(handle) = handle else {
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

    fn record_expected_node_failure(
        &mut self,
        node: &RuntimeNode,
        failure: ExpectedNodeFailure<'_>,
    ) -> Result<(), RuntimeError> {
        let details = Map::from_iter([
            (
                "action_type".to_owned(),
                Value::String(node.action_type.clone()),
            ),
            ("node_id".to_owned(), Value::String(node.id.clone())),
        ]);
        let error = Value::Object(Map::from_iter([
            (
                "message".to_owned(),
                Value::String(failure.message.to_owned()),
            ),
            ("code".to_owned(), Value::String(failure.code.to_owned())),
            (
                "type".to_owned(),
                Value::String(failure.error_type.to_owned()),
            ),
            ("retryable".to_owned(), Value::Bool(false)),
            ("details".to_owned(), Value::Object(details)),
        ]));
        self.set_variable(
            format!("{}.error", node.id),
            error,
            RunVariableScope::NodeOutput,
        )?;
        self.push_runtime_log(
            "error",
            format!("{} failed: {}", node.action_type, failure.message),
            Some(node.id.clone()),
        );
        Ok(())
    }

    fn process_repeat_frame(
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

        self.log_loop_iteration(
            node_id,
            index,
            format!("Repeat {node_id} iteration {} of {count}.", index + 1),
        );
        frames.push(RuntimeFrame::Repeat {
            node_id: node_id.to_owned(),
            index: index + 1,
            count,
        });
        frames.push(RuntimeFrame::Follow {
            source_node_id: node_id.to_owned(),
            handle: "repeat".to_owned(),
            stop_at_node_id: Some(node_id.to_owned()),
        });
        Ok(())
    }

    fn process_loop_control(
        &mut self,
        frames: &mut Vec<RuntimeFrame>,
        node: &RuntimeNode,
        break_loop: bool,
    ) -> Result<(), RuntimeError> {
        let Some(loop_frame_index) = frames.iter().rposition(is_loop_iteration_frame) else {
            return Err(RuntimeError::ControlFlow {
                node_id: node.id.clone(),
                message: format!(
                    "{} must run inside a Repeat, While, or For Each loop",
                    if break_loop {
                        "Break Loop"
                    } else {
                        "Continue Loop"
                    }
                ),
            });
        };
        let loop_node_id = loop_frame_node_id(&frames[loop_frame_index]).to_owned();

        if break_loop {
            frames.truncate(loop_frame_index);
            frames.push(RuntimeFrame::Follow {
                source_node_id: loop_node_id.clone(),
                handle: "done".to_owned(),
                stop_at_node_id: None,
            });
            self.push_runtime_log(
                "info",
                format!("Break Loop {} exited loop {loop_node_id}.", node.id),
                Some(node.id.clone()),
            );
        } else {
            frames.truncate(loop_frame_index + 1);
            self.push_runtime_log(
                "info",
                format!(
                    "Continue Loop {} advanced loop {loop_node_id} to its next iteration.",
                    node.id
                ),
                Some(node.id.clone()),
            );
        }

        Ok(())
    }

    fn process_while_frame(
        &mut self,
        frames: &mut Vec<RuntimeFrame>,
        node_id: &str,
        index: u64,
    ) -> Result<(), RuntimeError> {
        let node = self.graph.node(node_id)?.clone();
        let evaluation = self.evaluate_conditions(&node)?;
        if !evaluation.result {
            self.push_runtime_log(
                "info",
                format!(
                    "While {node_id} condition failed after {index} iteration{}: {}.",
                    if index == 1 { "" } else { "s" },
                    evaluation.summary
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

        let next_index = index.saturating_add(1);
        self.log_loop_iteration(
            node_id,
            index,
            format!(
                "While {node_id} iteration {next_index}. Condition passed: {}.",
                evaluation.summary
            ),
        );
        frames.push(RuntimeFrame::While {
            node_id: node_id.to_owned(),
            index: next_index,
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
        self.graph.node(node_id)?;
        if index >= items.len() {
            frames.push(RuntimeFrame::Follow {
                source_node_id: node_id.to_owned(),
                handle: "done".to_owned(),
                stop_at_node_id: None,
            });
            return Ok(());
        }

        let item_variable = format!("{node_id}.item");
        let index_variable = format!("{node_id}.index");
        self.set_variable(
            item_variable.clone(),
            items[index].clone(),
            RunVariableScope::NodeOutput,
        )?;
        self.set_variable(
            index_variable.clone(),
            Value::Number(Number::from(u64::try_from(index).unwrap_or(u64::MAX))),
            RunVariableScope::NodeOutput,
        )?;
        self.log_loop_iteration(
            node_id,
            u64::try_from(index).unwrap_or(u64::MAX),
            format!(
                "For Each item {} of {} set node output {:?} to {} and node output {:?} to {index}.",
                index + 1,
                items.len(),
                item_variable,
                serde_json::to_string(&items[index]).unwrap_or_else(|_| items[index].to_string()),
                index_variable
            ),
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

    fn log_loop_iteration(&mut self, node_id: &str, zero_based_index: u64, message: String) {
        const INITIAL_ITERATIONS: u64 = 100;
        const SAMPLE_INTERVAL: u64 = 1_000;

        if zero_based_index < INITIAL_ITERATIONS
            || zero_based_index
                .saturating_add(1)
                .is_multiple_of(SAMPLE_INTERVAL)
        {
            self.push_runtime_log("info", message, Some(node_id.to_owned()));
        } else if zero_based_index == INITIAL_ITERATIONS {
            self.push_runtime_log(
                "info",
                format!(
                    "Loop {node_id} continues. Further iteration diagnostics are sampled every {SAMPLE_INTERVAL} iterations."
                ),
                Some(node_id.to_owned()),
            );
        }
    }
}

fn condition_row_count(node: &RuntimeNode) -> usize {
    node.config
        .get("conditions")
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

#[derive(Debug, Clone, Copy)]
struct ExpectedNodeFailure<'a> {
    code: &'static str,
    error_type: &'static str,
    message: &'a str,
}

impl<'a> ExpectedNodeFailure<'a> {
    fn from_runtime_error(error: &'a RuntimeError, action_type: &str) -> Option<Self> {
        match error {
            RuntimeError::Action { message, .. } => {
                let (code, error_type) = match action_type {
                    "action.delay" => ("DELAY_DURATION_INVALID", "validation"),
                    "action.text.format" => ("TEXT_TRANSFORM_FAILED", "validation"),
                    "action.value.convert" => ("VALUE_CONVERSION_FAILED", "validation"),
                    _ => ("ACTION_FAILED", "runtime"),
                };
                Some(Self {
                    code,
                    error_type,
                    message,
                })
            }
            RuntimeError::Calculation { message, .. } => Some(Self {
                code: "CALCULATION_FAILED",
                error_type: "validation",
                message,
            }),
            RuntimeError::VariableOperation { message, .. } => Some(Self {
                code: "VARIABLE_OPERATION_FAILED",
                error_type: "validation",
                message,
            }),
            RuntimeError::InvalidGraph(_)
            | RuntimeError::ControlFlow { .. }
            | RuntimeError::UnsupportedStep { .. }
            | RuntimeError::State(_)
            | RuntimeError::Redacted(_)
            | RuntimeError::Cancelled => None,
        }
    }
}

fn is_loop_iteration_frame(frame: &RuntimeFrame) -> bool {
    matches!(
        frame,
        RuntimeFrame::Repeat { .. } | RuntimeFrame::While { .. } | RuntimeFrame::ForEach { .. }
    )
}

fn loop_frame_node_id(frame: &RuntimeFrame) -> &str {
    match frame {
        RuntimeFrame::Repeat { node_id, .. }
        | RuntimeFrame::While { node_id, .. }
        | RuntimeFrame::ForEach { node_id, .. } => node_id,
        RuntimeFrame::Follow { .. } | RuntimeFrame::Node { .. } => {
            unreachable!("loop frame lookup returned a non-loop frame")
        }
    }
}
