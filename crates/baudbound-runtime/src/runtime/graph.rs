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
