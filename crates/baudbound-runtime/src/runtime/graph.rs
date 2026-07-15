use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::{RuntimeEdge, RuntimeError, RuntimeNode};

#[derive(Debug, Clone, Deserialize)]
struct ProgramEnvelope {
    entry: ProgramEntry,
}

#[derive(Debug, Clone, Deserialize)]
struct ProgramEntry {
    trigger: RuntimeNode,
    #[serde(default)]
    triggers: Vec<RuntimeNode>,
    program: ProgramBlock,
}

#[derive(Debug, Clone, Deserialize)]
struct ProgramBlock {
    #[serde(default)]
    steps: Vec<RuntimeNode>,
    #[serde(default)]
    edges: Vec<RuntimeEdge>,
}

pub(crate) struct RuntimeGraph {
    nodes: BTreeMap<String, RuntimeNode>,
    edges_by_source: BTreeMap<String, Vec<RuntimeEdge>>,
    trigger_ids: Vec<String>,
}

impl RuntimeGraph {
    pub(crate) fn from_program_value(value: &Value) -> Result<Self, RuntimeError> {
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
        let mut execution_orders = BTreeMap::<(String, String), Vec<u32>>::new();
        for edge in envelope.entry.program.edges {
            if edge.source == edge.target {
                return Err(RuntimeError::InvalidGraph(format!(
                    "edge cannot connect node {} to itself",
                    edge.source
                )));
            }
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
            execution_orders
                .entry((edge.source.clone(), edge.source_handle.clone()))
                .or_default()
                .push(edge.execution_order);
            edges_by_source
                .entry(edge.source.clone())
                .or_default()
                .push(edge);
        }
        validate_execution_orders(&execution_orders)?;

        Ok(Self {
            nodes,
            edges_by_source,
            trigger_ids,
        })
    }

    pub(crate) fn manual_trigger(&self) -> Result<&RuntimeNode, RuntimeError> {
        self.trigger_ids
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .find(|node| node.action_type == "trigger.manual")
            .ok_or_else(|| RuntimeError::InvalidGraph("no manual trigger exists".to_owned()))
    }

    pub(crate) fn trigger(&self, node_id: &str) -> Result<&RuntimeNode, RuntimeError> {
        if !self.trigger_ids.iter().any(|id| id == node_id) {
            return Err(RuntimeError::InvalidGraph(format!(
                "node {node_id} is not a registered trigger"
            )));
        }
        self.node(node_id)
    }

    pub(crate) fn node(&self, node_id: &str) -> Result<&RuntimeNode, RuntimeError> {
        self.nodes
            .get(node_id)
            .ok_or_else(|| RuntimeError::InvalidGraph(format!("node {node_id} does not exist")))
    }

    pub(crate) fn nodes(&self) -> impl Iterator<Item = &RuntimeNode> {
        self.nodes.values()
    }

    pub(crate) fn target_node_ids_for_handle(&self, node_id: &str, handle: &str) -> Vec<String> {
        let mut matching_edges = self
            .edges_by_source
            .get(node_id)
            .into_iter()
            .flat_map(|edges| edges.iter())
            .filter(|edge| edge.source_handle == handle)
            .collect::<Vec<_>>();
        matching_edges.sort_by_key(|edge| edge.execution_order);
        matching_edges
            .into_iter()
            .map(|edge| edge.target.clone())
            .collect()
    }

    pub(crate) fn first_available_output_handle<'a>(
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

fn validate_execution_orders(
    execution_orders: &BTreeMap<(String, String), Vec<u32>>,
) -> Result<(), RuntimeError> {
    for ((source, source_handle), orders) in execution_orders {
        let mut sorted_orders = orders.clone();
        sorted_orders.sort_unstable();
        if sorted_orders
            .iter()
            .enumerate()
            .any(|(index, order)| usize::try_from(*order).ok() != Some(index))
        {
            return Err(RuntimeError::InvalidGraph(format!(
                "edges from node {source} output {source_handle} must use unique consecutive execution orders starting at 0"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn rejects_self_connections() {
        let program = json!({
            "entry": {
                "trigger": {
                    "id": "n-trigger",
                    "action_type": "trigger.manual",
                    "type": "manual",
                    "config": {}
                },
                "program": {
                    "steps": [{
                        "id": "n-log",
                        "action_type": "action.log",
                        "type": "action",
                        "action": "log",
                        "config": {}
                    }],
                    "edges": [{
                        "execution_order": 0,
                        "source": "n-log",
                        "source_handle": "out",
                        "target": "n-log",
                        "target_handle": "input"
                    }]
                }
            }
        });

        let error = match RuntimeGraph::from_program_value(&program) {
            Ok(_) => panic!("self-connection must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("cannot connect node"), "{error}");
        assert!(error.to_string().contains("to itself"), "{error}");
    }

    #[test]
    fn returns_fan_out_targets_in_explicit_execution_order() {
        let graph = RuntimeGraph::from_program_value(&fan_out_program(json!([
            edge("n-trigger", "n-zulu", 1),
            edge("n-trigger", "n-alpha", 0)
        ])))
        .expect("ordered fan-out should be valid");

        assert_eq!(
            graph.target_node_ids_for_handle("n-trigger", "out"),
            vec!["n-alpha", "n-zulu"]
        );
    }

    #[test]
    fn rejects_duplicate_or_gapped_execution_orders() {
        for (orders, expected) in [([0, 0], "duplicate"), ([0, 2], "gapped")] {
            let program = fan_out_program(json!([
                edge("n-trigger", "n-alpha", orders[0]),
                edge("n-trigger", "n-zulu", orders[1])
            ]));
            let error = RuntimeGraph::from_program_value(&program)
                .err()
                .unwrap_or_else(|| panic!("{expected} execution orders must fail"));
            assert!(error.to_string().contains("unique consecutive"), "{error}");
        }
    }

    fn edge(source: &str, target: &str, execution_order: u32) -> Value {
        json!({
            "execution_order": execution_order,
            "source": source,
            "source_handle": "out",
            "target": target,
            "target_handle": "input"
        })
    }

    fn fan_out_program(edges: Value) -> Value {
        json!({
            "entry": {
                "trigger": {
                    "id": "n-trigger",
                    "action_type": "trigger.manual",
                    "type": "manual",
                    "config": {}
                },
                "program": {
                    "steps": [
                        {
                            "id": "n-alpha",
                            "action_type": "action.log",
                            "type": "action",
                            "action": "log",
                            "config": {}
                        },
                        {
                            "id": "n-zulu",
                            "action_type": "action.log",
                            "type": "action",
                            "action": "log",
                            "config": {}
                        }
                    ],
                    "edges": edges
                }
            }
        })
    }
}
