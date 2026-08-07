use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use serde_json::{Map, Value};

use crate::value_kind;

pub(crate) fn convert_value_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let input = request.config.get("value").cloned().unwrap_or(Value::Null);
    // No default target. The old default named a type that no longer exists,
    // so falling back to it would report an unsupported target rather than the
    // real problem, which is that the node did not say what to convert to.
    let target = request
        .config
        .get("targetType")
        .and_then(Value::as_str)
        .ok_or_else(|| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: "conversion requires a target type".to_owned(),
        })?;
    let source_type = value_kind(&input).to_owned();

    let target = target
        .parse::<baudbound_runtime::ValueType>()
        .map_err(|_| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("unsupported conversion target {target}"),
        })?;
    let value = baudbound_runtime::cast_value(&input, target).map_err(|message| {
        RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message,
        }
    })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("value".to_owned(), value),
            ("source_type".to_owned(), Value::String(source_type)),
            (
                "target_type".to_owned(),
                Value::String(baudbound_runtime::value_type_name(target).to_owned()),
            ),
        ]),
        sensitive_output_keys: Default::default(),
    })
}
