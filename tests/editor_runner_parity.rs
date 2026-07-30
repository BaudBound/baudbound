use std::collections::BTreeSet;

use baudbound_actions::SUPPORTED_ACTION_TYPES;
use baudbound_core::{SUPPORTED_CORE_ACTION_TYPES, SUPPORTED_CORE_TRIGGER_ACTION_TYPES};
use baudbound_runtime::{SUPPORTED_CONTROL_ACTION_TYPES, SUPPORTED_INTERNAL_ACTION_TYPES};
use baudbound_triggers::SUPPORTED_SERVICE_TRIGGER_ACTION_TYPES;
use serde_json::Value;

#[test]
fn runner_implements_exactly_the_editor_generated_node_catalog() {
    let contract: Value =
        serde_json::from_str(include_str!("../contracts/runner/node-capabilities.json"))
            .expect("editor-generated node capability contract should be valid JSON");
    let editor_nodes = contract
        .get("nodes")
        .and_then(Value::as_object)
        .expect("node capability contract should contain a nodes object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    let runner_nodes = SUPPORTED_ACTION_TYPES
        .iter()
        .chain(SUPPORTED_CORE_ACTION_TYPES)
        .chain(SUPPORTED_INTERNAL_ACTION_TYPES)
        .chain(SUPPORTED_CONTROL_ACTION_TYPES)
        .chain(SUPPORTED_SERVICE_TRIGGER_ACTION_TYPES)
        .chain(SUPPORTED_CORE_TRIGGER_ACTION_TYPES)
        .copied()
        .collect::<BTreeSet<_>>();

    assert_eq!(
        runner_nodes, editor_nodes,
        "runner implementation and editor-generated node catalog have drifted"
    );
}
