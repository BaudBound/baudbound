use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use serde::Deserialize;
use serde_json::Value;

use super::PackageLoadError;

const PORT_CONTRACT_JSON: &str = include_str!("../../../../contracts/runner/node-ports.json");
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
        default_output: String,
        input: String,
        output_prefix: String,
    },
    RouterPorts {
        inputs_key: String,
        outputs_key: String,
        input_prefix: String,
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

    for node in nodes.values() {
        let action_type = node
            .get("action_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if let Some(PortPolicy::RouterPorts {
            inputs_key,
            outputs_key,
            ..
        }) = contract.nodes.get(action_type)
        {
            validate_router_config(node, inputs_key, outputs_key)?;
        }
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
            default_output,
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
            outputs.insert(default_output.clone());
            Ok((BTreeSet::from([input.clone()]), outputs))
        }
        PortPolicy::RouterPorts {
            inputs_key,
            outputs_key,
            input_prefix,
            output_prefix,
        } => {
            let inputs = router_port_ids(node, inputs_key)?;
            let outputs = router_port_ids(node, outputs_key)?;
            Ok((
                inputs
                    .iter()
                    .map(|id| format!("{input_prefix}{id}"))
                    .collect(),
                outputs
                    .iter()
                    .map(|id| format!("{output_prefix}{id}"))
                    .collect(),
            ))
        }
    }
}

fn router_port_ids(node: &Value, key: &str) -> Result<Vec<String>, PackageLoadError> {
    let id = node_id(node).unwrap_or("unknown");
    let ports = node
        .get("config")
        .and_then(|config| config.get(key))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            PackageLoadError::ProgramGraph(format!(
                "router node {id:?} config.{key} must be an array"
            ))
        })?;
    let mut ids = Vec::with_capacity(ports.len());
    for port in ports {
        let port_id = port
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                PackageLoadError::ProgramGraph(format!(
                    "router node {id:?} config.{key} entry is missing a non-empty id"
                ))
            })?;
        if ids.iter().any(|existing| existing == port_id) {
            return Err(PackageLoadError::ProgramGraph(format!(
                "router node {id:?} has a duplicate {} id {port_id:?}",
                key.trim_end_matches('s')
            )));
        }
        let label = port
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if label.trim().is_empty() {
            return Err(PackageLoadError::ProgramGraph(format!(
                "router node {id:?} {} {port_id:?} must have a non-empty label",
                key.trim_end_matches('s')
            )));
        }
        ids.push(port_id.to_owned());
    }
    Ok(ids)
}

fn validate_router_config(
    node: &Value,
    inputs_key: &str,
    outputs_key: &str,
) -> Result<(), PackageLoadError> {
    let id = node_id(node).unwrap_or("unknown");
    let config = node
        .get("config")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            PackageLoadError::ProgramGraph(format!("router node {id:?} config must be an object"))
        })?;
    if !config.get(inputs_key).is_some_and(Value::is_array) {
        return Err(PackageLoadError::ProgramGraph(format!(
            "router node {id:?} config.{inputs_key}: inputs must be an array"
        )));
    }
    if !config.get(outputs_key).is_some_and(Value::is_array) {
        return Err(PackageLoadError::ProgramGraph(format!(
            "router node {id:?} config.{outputs_key}: outputs must be an array"
        )));
    }
    let inputs = router_port_ids(node, inputs_key)?;
    let outputs = router_port_ids(node, outputs_key)?;
    if inputs.is_empty() {
        return Err(PackageLoadError::ProgramGraph(format!(
            "router node {id:?} must define at least one input"
        )));
    }
    if outputs.is_empty() {
        return Err(PackageLoadError::ProgramGraph(format!(
            "router node {id:?} must define at least one output"
        )));
    }
    let routes = config
        .get("routes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            PackageLoadError::ProgramGraph(format!(
                "router node {id:?} config.routes: routes must be an array"
            ))
        })?;

    let mut route_ids = BTreeSet::new();
    let mut pairs = BTreeSet::new();
    let mut orders_by_input = BTreeMap::<&str, Vec<u32>>::new();
    let mut routed_outputs = BTreeSet::new();
    for route in routes {
        let route_id = route
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                PackageLoadError::ProgramGraph(format!(
                    "router node {id:?} route is missing a non-empty id"
                ))
            })?;
        if !route_ids.insert(route_id) {
            return Err(PackageLoadError::ProgramGraph(format!(
                "router node {id:?} has a duplicate route id {route_id:?}"
            )));
        }
        let input_id = route
            .get("inputId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let output_id = route
            .get("outputId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !inputs.iter().any(|candidate| candidate == input_id) {
            return Err(PackageLoadError::ProgramGraph(format!(
                "router node {id:?} route {route_id:?} references missing input {input_id:?}"
            )));
        }
        if !outputs.iter().any(|candidate| candidate == output_id) {
            return Err(PackageLoadError::ProgramGraph(format!(
                "router node {id:?} route {route_id:?} references missing output {output_id:?}"
            )));
        }
        if !pairs.insert((input_id, output_id)) {
            return Err(PackageLoadError::ProgramGraph(format!(
                "router node {id:?} has a duplicate route from {input_id:?} to {output_id:?}"
            )));
        }
        let order = route
            .get("order")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| {
                PackageLoadError::ProgramGraph(format!(
                    "router node {id:?} route {route_id:?} order must be a non-negative 32-bit integer"
                ))
            })?;
        orders_by_input.entry(input_id).or_default().push(order);
        routed_outputs.insert(output_id);
    }

    for input_id in &inputs {
        let Some(orders) = orders_by_input.get(input_id.as_str()) else {
            return Err(PackageLoadError::ProgramGraph(format!(
                "router node {id:?} input {input_id:?} has no routes"
            )));
        };
        let mut sorted = orders.clone();
        sorted.sort_unstable();
        if sorted
            .iter()
            .enumerate()
            .any(|(index, order)| usize::try_from(*order).ok() != Some(index))
        {
            return Err(PackageLoadError::ProgramGraph(format!(
                "router node {id:?} input {input_id:?} routes must use unique consecutive order values starting at 0"
            )));
        }
    }
    for output_id in &outputs {
        if !routed_outputs.contains(output_id.as_str()) {
            return Err(PackageLoadError::ProgramGraph(format!(
                "router node {id:?} output {output_id:?} has no incoming routes"
            )));
        }
    }
    Ok(())
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

        let mut default = valid.clone();
        default["entry"]["program"]["edges"][0]["source_handle"] = json!("default");
        validate_program_graph(&default).expect("switch default port should validate");

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

    fn router_step(config: Value) -> Value {
        json!({"id":"n-router","action_type":"control.router","config":config})
    }

    fn valid_router_config() -> Value {
        json!({
            "inputs":[{"id":"a","label":"Alpha"},{"id":"b","label":"Beta"}],
            "outputs":[{"id":"x","label":"X"},{"id":"y","label":"Y"}],
            "routes":[
                {"id":"r1","inputId":"a","outputId":"x","order":0},
                {"id":"r2","inputId":"a","outputId":"y","order":1},
                {"id":"r3","inputId":"b","outputId":"y","order":0}
            ]
        })
    }

    fn router_program(config: Value, edges: Vec<Value>) -> Value {
        json!({
            "entry": {
                "trigger": {"id":"n-trigger","action_type":"trigger.manual","config":{}},
                "triggers": [],
                "program": {
                    "steps":[router_step(config), json!({"id":"n-log","action_type":"action.log","config":{}})],
                    "edges":edges
                }
            }
        })
    }

    #[test]
    fn derives_router_handles_from_config_ports() {
        let program = router_program(
            valid_router_config(),
            vec![
                json!({"execution_order":0,"source":"n-trigger","source_handle":"out","target":"n-router","target_handle":"in-a"}),
                json!({"execution_order":0,"source":"n-router","source_handle":"out-x","target":"n-log","target_handle":"input"}),
            ],
        );
        validate_program_graph(&program).expect("router handles should validate");

        let mut bad_input = program.clone();
        bad_input["entry"]["program"]["edges"][0]["target_handle"] = json!("in-missing");
        let error = validate_program_graph(&bad_input).expect_err("unknown router input must fail");
        assert!(
            error.to_string().contains("unknown target_handle"),
            "{error}"
        );

        let mut bad_output = program;
        bad_output["entry"]["program"]["edges"][1]["source_handle"] = json!("out-missing");
        let error =
            validate_program_graph(&bad_output).expect_err("unknown router output must fail");
        assert!(
            error.to_string().contains("unknown source_handle"),
            "{error}"
        );
    }

    #[test]
    fn rejects_malformed_router_configs_even_without_edges() {
        let cases: Vec<(Value, &str)> = vec![
            (
                json!({"inputs":[],"outputs":[{"id":"x","label":"X"}],"routes":[]}),
                "at least one input",
            ),
            (
                json!({"inputs":[{"id":"a","label":"A"}],"outputs":[],"routes":[]}),
                "at least one output",
            ),
            (
                json!({"inputs":"nope","outputs":[],"routes":[]}),
                "inputs must be an array",
            ),
            (
                json!({"inputs":[{"id":"a","label":"A"},{"id":"a","label":"B"}],"outputs":[{"id":"x","label":"X"}],"routes":[{"id":"r","inputId":"a","outputId":"x","order":0}]}),
                "duplicate input id",
            ),
            (
                json!({"inputs":[{"id":"a","label":""}],"outputs":[{"id":"x","label":"X"}],"routes":[{"id":"r","inputId":"a","outputId":"x","order":0}]}),
                "label",
            ),
            (
                json!({"inputs":[{"id":"a","label":"A"}],"outputs":[{"id":"x","label":"X"}],"routes":[]}),
                "has no routes",
            ),
            (
                json!({"inputs":[{"id":"a","label":"A"}],"outputs":[{"id":"x","label":"X"},{"id":"y","label":"Y"}],"routes":[{"id":"r","inputId":"a","outputId":"x","order":0}]}),
                "has no incoming routes",
            ),
            (
                json!({"inputs":[{"id":"a","label":"A"}],"outputs":[{"id":"x","label":"X"}],"routes":[{"id":"r1","inputId":"a","outputId":"x","order":0},{"id":"r2","inputId":"a","outputId":"x","order":1}]}),
                "duplicate route",
            ),
            (
                json!({"inputs":[{"id":"a","label":"A"}],"outputs":[{"id":"x","label":"X"}],"routes":[{"id":"r","inputId":"a","outputId":"zzz","order":0}]}),
                "missing output",
            ),
            (
                json!({"inputs":[{"id":"a","label":"A"}],"outputs":[{"id":"x","label":"X"},{"id":"y","label":"Y"}],"routes":[{"id":"r1","inputId":"a","outputId":"x","order":1},{"id":"r2","inputId":"a","outputId":"y","order":2}]}),
                "unique consecutive",
            ),
        ];
        for (config, expected) in cases {
            let program = router_program(config, vec![]);
            let error = validate_program_graph(&program)
                .err()
                .unwrap_or_else(|| panic!("{expected} must fail"))
                .to_string();
            assert!(error.contains(expected), "expected {expected:?} in {error}");
        }
        validate_program_graph(&router_program(valid_router_config(), vec![]))
            .expect("valid router without edges should pass");
    }
}
