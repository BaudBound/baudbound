use arboard::Clipboard;
use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use serde_json::{Map, Number, Value};

use super::config::{failed_error, required_string};

pub(super) fn run_clipboard(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let value = required_string(request, "value")?;
    Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(value.clone()))
        .map_err(|source| failed_error(request, format!("clipboard write failed: {source}")))?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("bytes".to_owned(), Value::Number(Number::from(value.len()))),
            ("value".to_owned(), Value::String(value)),
        ]),
    })
}
