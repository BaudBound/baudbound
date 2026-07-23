use baudbound_runtime::{
    RuntimeActionError, RuntimeActionRequest, RuntimeActionResult, RuntimeContext,
};
use enigo::Keyboard;
use serde_json::{Map, Number, Value};

mod keymap;

use keymap::parse_key_combo;

use super::{
    config::{failed_error, required_string},
    input::{InputAction, NativeInputState, native_input},
};

pub(super) fn run_keyboard(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
    input_state: &NativeInputState,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let key = required_string(request, "key")?;
    let combo = parse_key_combo(&key).map_err(|message| {
        failed_error(
            request,
            format!("invalid keyboard key expression: {message}"),
        )
    })?;
    let input_action = InputAction::from_request(request)?;
    input_state.keyboard(
        request,
        &context.identity.run_id,
        &combo.canonical_keys,
        &combo.keys,
        input_action,
    )?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            (
                "input_action".to_owned(),
                Value::String(input_action.as_str().to_owned()),
            ),
            ("key".to_owned(), Value::String(combo.expression.clone())),
        ]),
    })
}

pub(super) fn run_keyboard_type_text(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let text = required_string(request, "text")?;
    let mut enigo = native_input(request)?;
    enigo
        .text(&text)
        .map_err(|source| failed_error(request, format!("typing text failed: {source}")))?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            (
                "chars".to_owned(),
                Value::Number(Number::from(text.chars().count())),
            ),
            ("text".to_owned(), Value::String(text)),
        ]),
    })
}
