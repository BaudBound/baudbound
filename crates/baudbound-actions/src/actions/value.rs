use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use serde_json::{Map, Number, Value};

use crate::{failed, value_kind};

const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

pub(crate) fn convert_value_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let input = request.config.get("value").cloned().unwrap_or(Value::Null);
    let target = request
        .config
        .get("targetType")
        .and_then(Value::as_str)
        .unwrap_or("text");
    let source_type = value_kind(&input).to_owned();
    let value = match target {
        "text" => Value::String(match &input {
            Value::String(value) => value.clone(),
            value => serde_json::to_string(value).map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("failed to serialize value as text: {source}"),
            })?,
        }),
        "number" => Value::Number(convert_number(request, &input, false)?),
        "integer" => Value::Number(convert_number(request, &input, true)?),
        "boolean" => Value::Bool(match &input {
            Value::Bool(value) => *value,
            Value::String(value) if value.trim().eq_ignore_ascii_case("true") => true,
            Value::String(value) if value.trim().eq_ignore_ascii_case("false") => false,
            _ => return failed(request, "boolean conversion expects true or false"),
        }),
        "list" => match parse_json_string(&input) {
            Value::Array(items) => Value::Array(items),
            other => {
                return failed(
                    request,
                    format!(
                        "list conversion expects a JSON list, found {}",
                        value_kind(&other)
                    ),
                );
            }
        },
        "object" => match parse_json_string(&input) {
            Value::Object(object) => Value::Object(object),
            other => {
                return failed(
                    request,
                    format!(
                        "object conversion expects a JSON object, found {}",
                        value_kind(&other)
                    ),
                );
            }
        },
        _ => return failed(request, format!("unsupported conversion target {target:?}")),
    };

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("value".to_owned(), value),
            ("source_type".to_owned(), Value::String(source_type)),
            ("target_type".to_owned(), Value::String(target.to_owned())),
        ]),
    })
}

fn convert_number(
    request: &RuntimeActionRequest,
    input: &Value,
    integer: bool,
) -> Result<Number, RuntimeActionError> {
    let value = match input {
        Value::Number(value) => value.as_f64(),
        Value::String(value) if !value.trim().is_empty() => value.trim().parse::<f64>().ok(),
        _ => None,
    }
    .filter(|value| value.is_finite())
    .ok_or_else(|| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: "number conversion expects a finite numeric value".to_owned(),
    })?;

    if integer {
        if value.fract() != 0.0 || value.abs() > MAX_SAFE_INTEGER as f64 {
            return failed(
                request,
                format!(
                    "integer conversion expects a whole number between -{MAX_SAFE_INTEGER} and {MAX_SAFE_INTEGER}"
                ),
            );
        }
        return Ok(Number::from(value as i64));
    }

    Number::from_f64(value).ok_or_else(|| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: "number conversion produced a non-finite number".to_owned(),
    })
}

fn parse_json_string(input: &Value) -> Value {
    match input {
        Value::String(value) => serde_json::from_str(value).unwrap_or_else(|_| input.clone()),
        _ => input.clone(),
    }
}
