use crate::runtime::{RuntimeFrame, required_config_string, validate_variable_name};
use serde_json::{Number, Value};

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
                self.push_runtime_log(
                    "info",
                    format!("Color Match {} selected \"{}\" output.", node.id, branch),
                    Some(node.id.clone()),
                );
                frames.push(RuntimeFrame::Follow {
                    source_node_id: node.id,
                    handle: branch.to_owned(),
                    stop_at_node_id: None,
                });
            }
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
                let handle = self.evaluate_switch(&node)?;
                frames.push(RuntimeFrame::Follow {
                    source_node_id: node.id,
                    handle,
                    stop_at_node_id: None,
                });
            }
            "control.repeat" => {
                let count = self.repeat_count(&node)?;
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

        let next_index = index.saturating_add(1);
        self.log_loop_iteration(
            node_id,
            index,
            format!("While {node_id} iteration {next_index}; condition passed."),
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
        self.set_variable(
            item_variable,
            items[index].clone(),
            RunVariableScope::Runtime,
        )?;
        self.set_variable(
            index_variable,
            Value::Number(Number::from(u64::try_from(index).unwrap_or(u64::MAX))),
            RunVariableScope::Runtime,
        )?;
        self.log_loop_iteration(
            node_id,
            u64::try_from(index).unwrap_or(u64::MAX),
            format!("For Each {node_id} item {} of {}.", index + 1, items.len()),
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
