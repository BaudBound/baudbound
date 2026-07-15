use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use serde::Deserialize;
use serde_json::Value;

use super::PackageLoadError;

const PORT_CONTRACT_JSON: &str = include_str!("../../contracts/node-ports.json");
const PORT_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
struct NodePortContract {
    nodes: BTreeMap<String, PortPolicy>,
    version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PortPolicy {
    Fixed {
        inputs: Vec<String>,
        outputs: Vec<String>,
    },
    SwitchCases {
        config_key: String,
        input: String,
        output_prefix: String,
    },
}

pub(super) fn validate_program_graph(program: &Value) -> Result<(), PackageLoadError> {
    let contract = port_contract().map_err(PackageLoadError::PortContract)?;
    let entry = program
        .get("entry")
        .and_then(Value::as_object)
        .ok_or_else(|| PackageLoadError::ProgramGraph("entry is missing".to_owned()))?;
    let primary_trigger = entry
        .get("trigger")
        .ok_or_else(|| PackageLoadError::ProgramGraph("entry.trigger is missing".to_owned()))?;
    let mut nodes = BTreeMap::<String, &Value>::new();
    insert_node(&mut nodes, primary_trigger)?;

    for trigger in entry
        .get("triggers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = node_id(trigger)?;
        if let Some(existing) = nodes.get(id) {
            if **existing != *trigger {
                return Err(PackageLoadError::ProgramGraph(format!(
                    "node id {id:?} is reused with different trigger definitions"
                )));
            }
            continue;
        }
        insert_node(&mut nodes, trigger)?;
    }

    let block = entry
        .get("program")
        .and_then(Value::as_object)
        .ok_or_else(|| PackageLoadError::ProgramGraph("entry.program is missing".to_owned()))?;
    for step in block
        .get("steps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        insert_node(&mut nodes, step)?;
    }

    let mut execution_orders = BTreeMap::<(String, String), Vec<u32>>::new();
    for (index, edge) in block
        .get("edges")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let validated_edge = validate_edge(index, edge, &nodes, contract)?;
        execution_orders
            .entry((validated_edge.source, validated_edge.source_handle))
            .or_default()
            .push(validated_edge.execution_order);
    }
    validate_execution_orders(&execution_orders)?;
    Ok(())
}

struct ValidatedEdge {
    execution_order: u32,
    source: String,
    source_handle: String,
}

fn validate_edge(
    index: usize,
    edge: &Value,
    nodes: &BTreeMap<String, &Value>,
    contract: &NodePortContract,
) -> Result<ValidatedEdge, PackageLoadError> {
    let source = edge_string(edge, "source")?;
    let target = edge_string(edge, "target")?;
    let source_handle = edge_string(edge, "source_handle")?;
    let target_handle = edge_string(edge, "target_handle")?;
    let execution_order = edge_u32(edge, "execution_order")?;
    if source == target {
        return Err(PackageLoadError::ProgramGraph(format!(
            "edge {} cannot connect node {source:?} to itself",
            index + 1
        )));
    }
    let source_node = nodes.get(source).ok_or_else(|| {
        PackageLoadError::ProgramGraph(format!(
            "edge {} references missing source node {source:?}",
            index + 1
        ))
    })?;
    let target_node = nodes.get(target).ok_or_else(|| {
        PackageLoadError::ProgramGraph(format!(
            "edge {} references missing target node {target:?}",
            index + 1
        ))
    })?;

    let source_ports = node_ports(source_node, contract)?;
    let target_ports = node_ports(target_node, contract)?;
    if !source_ports.1.contains(source_handle) {
        return Err(PackageLoadError::ProgramGraph(format!(
            "edge {} uses unknown source_handle {source_handle:?} on node {source:?}",
            index + 1
        )));
    }
    if !target_ports.0.contains(target_handle) {
        return Err(PackageLoadError::ProgramGraph(format!(
            "edge {} uses unknown target_handle {target_handle:?} on node {target:?}",
            index + 1
        )));
    }
    Ok(ValidatedEdge {
        execution_order,
        source: source.to_owned(),
        source_handle: source_handle.to_owned(),
    })
}

fn validate_execution_orders(
    execution_orders: &BTreeMap<(String, String), Vec<u32>>,
) -> Result<(), PackageLoadError> {
    for ((source, source_handle), orders) in execution_orders {
        let mut sorted_orders = orders.clone();
        sorted_orders.sort_unstable();
        if sorted_orders
            .iter()
            .enumerate()
            .any(|(index, order)| usize::try_from(*order).ok() != Some(index))
        {
            return Err(PackageLoadError::ProgramGraph(format!(
                "edges from node {source:?} output {source_handle:?} must use unique consecutive execution_order values starting at 0"
            )));
        }
    }
    Ok(())
}

fn node_ports(
    node: &Value,
    contract: &NodePortContract,
) -> Result<(BTreeSet<String>, BTreeSet<String>), PackageLoadError> {
    let action_type = node
        .get("action_type")
        .and_then(Value::as_str)
        .ok_or_else(|| PackageLoadError::ProgramGraph("node action_type is missing".to_owned()))?;
    let policy = contract.nodes.get(action_type).ok_or_else(|| {
        PackageLoadError::ProgramGraph(format!(
            "node action type {action_type:?} has no port contract"
        ))
    })?;
    match policy {
        PortPolicy::Fixed { inputs, outputs } => Ok((
            inputs.iter().cloned().collect(),
            outputs.iter().cloned().collect(),
        )),
        PortPolicy::SwitchCases {
            config_key,
            input,
            output_prefix,
        } => {
            let cases = node
                .get("config")
                .and_then(|config| config.get(config_key))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    PackageLoadError::ProgramGraph(format!(
                        "switch node {:?} is missing config.{config_key}",
                        node_id(node).unwrap_or("unknown")
                    ))
                })?;
            let mut outputs = BTreeSet::new();
            for case in cases {
                let id = case
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| {
                        PackageLoadError::ProgramGraph(
                            "switch case is missing a non-empty id".to_owned(),
                        )
                    })?;
                if !outputs.insert(format!("{output_prefix}{id}")) {
                    return Err(PackageLoadError::ProgramGraph(format!(
                        "switch node {:?} contains duplicate case id {id:?}",
                        node_id(node).unwrap_or("unknown")
                    )));
                }
            }
            Ok((BTreeSet::from([input.clone()]), outputs))
        }
    }
}

fn insert_node<'a>(
    nodes: &mut BTreeMap<String, &'a Value>,
    node: &'a Value,
) -> Result<(), PackageLoadError> {
    let id = node_id(node)?;
    if nodes.insert(id.to_owned(), node).is_some() {
        return Err(PackageLoadError::ProgramGraph(format!(
            "duplicate node id {id:?}"
        )));
    }
    Ok(())
}

fn node_id(node: &Value) -> Result<&str, PackageLoadError> {
    node.get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| PackageLoadError::ProgramGraph("node id is missing".to_owned()))
}

fn edge_string<'a>(edge: &'a Value, key: &str) -> Result<&'a str, PackageLoadError> {
    edge.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| PackageLoadError::ProgramGraph(format!("edge {key} is missing")))
}

fn edge_u32(edge: &Value, key: &str) -> Result<u32, PackageLoadError> {
    edge.get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            PackageLoadError::ProgramGraph(format!(
                "edge {key} must be a non-negative 32-bit integer"
            ))
        })
}

fn port_contract() -> Result<&'static NodePortContract, String> {
    static CONTRACT: OnceLock<Result<NodePortContract, String>> = OnceLock::new();
    match CONTRACT.get_or_init(parse_port_contract) {
        Ok(contract) => Ok(contract),
        Err(message) => Err(message.clone()),
    }
}

fn parse_port_contract() -> Result<NodePortContract, String> {
    let contract = serde_json::from_str::<NodePortContract>(PORT_CONTRACT_JSON)
        .map_err(|error| error.to_string())?;
    if contract.version != PORT_CONTRACT_VERSION {
        return Err(format!(
            "unsupported version {}; expected {PORT_CONTRACT_VERSION}",
            contract.version
        ));
    }
    if contract.nodes.is_empty() {
        return Err("node mapping is empty".to_owned());
    }
    Ok(contract)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn validates_fixed_node_ports() {
        let program = program_with_step(
            json!({"id":"n-log","action_type":"action.log","config":{}}),
            json!({"execution_order":0,"source":"n-trigger","source_handle":"out","target":"n-log","target_handle":"input"}),
        );
        validate_program_graph(&program).expect("known fixed ports should validate");
    }

    #[test]
    fn rejects_unknown_source_and_target_handles() {
        for (source_handle, target_handle, expected) in [
            ("unknown", "input", "unknown source_handle"),
            ("out", "unknown", "unknown target_handle"),
        ] {
            let program = program_with_step(
                json!({"id":"n-log","action_type":"action.log","config":{}}),
                json!({
                    "execution_order":0,
                    "source":"n-trigger",
                    "source_handle":source_handle,
                    "target":"n-log",
                    "target_handle":target_handle
                }),
            );
            let error = validate_program_graph(&program).expect_err("unknown handle must fail");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn rejects_self_connections() {
        let program = program_with_step(
            json!({"id":"n-log","action_type":"action.log","config":{}}),
            json!({"execution_order":0,"source":"n-log","source_handle":"out","target":"n-log","target_handle":"input"}),
        );

        let error = validate_program_graph(&program).expect_err("self-connection must fail");
        assert!(error.to_string().contains("cannot connect node"), "{error}");
        assert!(error.to_string().contains("to itself"), "{error}");
    }

    #[test]
    fn derives_switch_outputs_from_case_ids() {
        let valid = program_with_step(
            json!({
                "id":"n-switch",
                "action_type":"control.switch",
                "config":{"cases":[{"id":"ready","name":"Ready","value":"ready"}]}
            }),
            json!({"execution_order":0,"source":"n-switch","source_handle":"case-ready","target":"n-log","target_handle":"input"}),
        );
        let mut valid = valid;
        valid["entry"]["program"]["steps"]
            .as_array_mut()
            .expect("steps array")
            .push(json!({"id":"n-log","action_type":"action.log","config":{}}));
        validate_program_graph(&valid).expect("switch case port should validate");

        let mut invalid = valid;
        invalid["entry"]["program"]["edges"][0]["source_handle"] = json!("case-missing");
        let error = validate_program_graph(&invalid).expect_err("unknown switch case must fail");
        assert!(
            error.to_string().contains("unknown source_handle"),
            "{error}"
        );
    }

    #[test]
    fn rejects_conflicting_primary_trigger_duplicates() {
        let mut program = program_with_step(
            json!({"id":"n-log","action_type":"action.log","config":{}}),
            json!({"execution_order":0,"source":"n-trigger","source_handle":"out","target":"n-log","target_handle":"input"}),
        );
        program["entry"]["triggers"] = json!([{
            "id":"n-trigger",
            "action_type":"trigger.hotkey",
            "config":{"key":"Ctrl+Alt+B"}
        }]);

        let error =
            validate_program_graph(&program).expect_err("conflicting duplicate trigger must fail");
        assert!(
            error.to_string().contains("reused with different"),
            "{error}"
        );
    }

    fn program_with_step(step: Value, edge: Value) -> Value {
        json!({
            "entry": {
                "trigger": {"id":"n-trigger","action_type":"trigger.manual","config":{}},
                "triggers": [],
                "program": {"steps":[step],"edges":[edge]}
            }
        })
    }

    #[test]
    fn validates_and_rejects_fan_out_execution_orders() {
        let mut valid = program_with_step(
            json!({"id":"n-first","action_type":"action.log","config":{}}),
            json!({"execution_order":1,"source":"n-trigger","source_handle":"out","target":"n-first","target_handle":"input"}),
        );
        valid["entry"]["program"]["steps"]
            .as_array_mut()
            .expect("steps array")
            .push(json!({"id":"n-second","action_type":"action.log","config":{}}));
        valid["entry"]["program"]["edges"]
            .as_array_mut()
            .expect("edges array")
            .push(json!({"execution_order":0,"source":"n-trigger","source_handle":"out","target":"n-second","target_handle":"input"}));
        validate_program_graph(&valid).expect("consecutive fan-out order should validate");

        for orders in [[0, 0], [0, 2]] {
            let mut invalid = valid.clone();
            invalid["entry"]["program"]["edges"][0]["execution_order"] = json!(orders[0]);
            invalid["entry"]["program"]["edges"][1]["execution_order"] = json!(orders[1]);
            let error = validate_program_graph(&invalid)
                .expect_err("duplicate or gapped execution orders must fail");
            assert!(error.to_string().contains("unique consecutive"), "{error}");
        }
    }
}
