use serde_json::{Map, Value};

use crate::runtime::value_to_string;
use crate::{RuntimeError, RuntimeNode};
pub(crate) fn config_string(config: &Map<String, Value>, key: &str) -> Option<String> {
    match config.get(key) {
        Some(Value::String(value)) => Some(value.clone()),
        Some(value) => Some(value_to_string(value)),
        None => None,
    }
}

pub(crate) fn required_config_string(
    node: &RuntimeNode,
    key: &str,
) -> Result<String, RuntimeError> {
    config_string(&node.config, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| RuntimeError::VariableOperation {
            node_id: node.id.clone(),
            message: format!("missing required config field {key}"),
        })
}
