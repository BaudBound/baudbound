use crate::runtime::{RuntimeFrame, required_config_string, validate_variable_name};
use serde_json::{Number, Value};

use super::{RunVariableScope, RuntimeError, RuntimeExecutor};

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
        self.set_variable(
            item_variable,
            items[index].clone(),
            RunVariableScope::Runtime,
        );
        self.set_variable(
            index_variable,
            Value::Number(Number::from(u64::try_from(index).unwrap_or(u64::MAX))),
            RunVariableScope::Runtime,
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
}
