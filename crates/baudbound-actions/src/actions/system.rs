use std::{
    io::{self, Write},
    thread,
    time::Duration,
};

use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use serde_json::{Map, Value};

use crate::{failed, number_from_config, number_json};
pub(crate) fn beep_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let frequency_hz = number_from_config(&request.config, "frequencyHz").unwrap_or(800.0);
    let duration_ms = number_from_config(&request.config, "durationMs").unwrap_or(200.0);
    if !frequency_hz.is_finite() || frequency_hz <= 0.0 {
        return failed(request, "frequencyHz must be a positive number");
    }
    if !duration_ms.is_finite() || duration_ms <= 0.0 {
        return failed(request, "durationMs must be a positive number");
    }

    let duration = Duration::from_secs_f64(duration_ms / 1000.0);
    let mut stdout = io::stdout();
    stdout
        .write_all(b"\x07")
        .and_then(|_| stdout.flush())
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to emit terminal bell: {source}"),
        })?;
    thread::sleep(duration);

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            (
                "frequency_hz".to_owned(),
                number_json(frequency_hz).unwrap_or(Value::Null),
            ),
            (
                "duration_ms".to_owned(),
                number_json(duration_ms).unwrap_or(Value::Null),
            ),
        ]),
    })
}

pub(crate) fn desktop_only_action(
    request: &RuntimeActionRequest,
    capability: &str,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    failed(
        request,
        format!("{capability} requires the desktop runner action adapter"),
    )
}
