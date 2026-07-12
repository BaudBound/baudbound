use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest};
use serde_json::Value;

pub(super) fn required_string(
    request: &RuntimeActionRequest,
    key: &str,
) -> Result<String, RuntimeActionError> {
    config_string(request, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| failed_error(request, format!("missing required config field {key}")))
}

pub(super) fn config_string(request: &RuntimeActionRequest, key: &str) -> Option<String> {
    request.config.get(key).map(value_to_string)
}

#[cfg(windows)]
pub(super) fn required_i32(
    request: &RuntimeActionRequest,
    key: &str,
) -> Result<i32, RuntimeActionError> {
    let raw = required_string(request, key)?;
    raw.trim().parse::<i32>().map_err(|source| {
        failed_error(
            request,
            format!("invalid integer config field {key}: {source}"),
        )
    })
}

pub(super) fn required_u32(
    request: &RuntimeActionRequest,
    key: &str,
) -> Result<u32, RuntimeActionError> {
    let raw = required_string(request, key)?;
    raw.trim().parse::<u32>().map_err(|source| {
        failed_error(
            request,
            format!("invalid non-negative integer config field {key}: {source}"),
        )
    })
}

#[cfg(windows)]
pub(super) fn config_bool(request: &RuntimeActionRequest, key: &str) -> bool {
    match request.config.get(key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => matches!(value.trim(), "true" | "1" | "yes" | "on"),
        Some(Value::Number(value)) => value.as_i64().is_some_and(|value| value != 0),
        _ => false,
    }
}

pub(super) fn failed_error(
    request: &RuntimeActionRequest,
    message: impl Into<String>,
) -> RuntimeActionError {
    RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: message.into(),
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}
