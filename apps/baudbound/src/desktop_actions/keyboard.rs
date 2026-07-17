use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use enigo::{Direction, Enigo, Key, Keyboard};
use serde_json::{Map, Number, Value};

mod keymap;

use keymap::{ParsedKeyCombo, parse_key_combo};

use super::{
    config::{failed_error, required_string},
    input::native_input,
};

pub(super) fn run_keyboard(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let key = required_string(request, "key")?;
    let combo = parse_key_combo(&key).map_err(|message| {
        failed_error(
            request,
            format!("invalid keyboard key expression: {message}"),
        )
    })?;
    let mut enigo = native_input(request)?;
    press_key_combo(request, &mut enigo, &combo)?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([("key".to_owned(), Value::String(combo.expression.clone()))]),
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

fn press_key_combo(
    request: &RuntimeActionRequest,
    enigo: &mut Enigo,
    combo: &ParsedKeyCombo,
) -> Result<(), RuntimeActionError> {
    let Some((last_key, held_keys)) = combo.keys.split_last() else {
        return Err(failed_error(request, "key chord is empty"));
    };
    let mut pressed_keys = Vec::new();
    for key in held_keys {
        if let Err(source) = enigo.key(*key, Direction::Press) {
            let release_error = release_keys(enigo, &pressed_keys);
            let detail = release_error.map_or_else(
                || source.to_string(),
                |release| format!("{source}; key cleanup also failed: {release}"),
            );
            return Err(failed_error(request, format!("key press failed: {detail}")));
        }
        pressed_keys.push(*key);
    }

    let key_error = enigo
        .key(*last_key, Direction::Click)
        .err()
        .map(|source| source.to_string());
    let release_error = release_keys(enigo, &pressed_keys);

    if let Some(source) = key_error {
        let detail = release_error.map_or(source.clone(), |release| {
            format!("{source}; key cleanup also failed: {release}")
        });
        return Err(failed_error(request, format!("key press failed: {detail}")));
    }
    if let Some(source) = release_error {
        return Err(failed_error(
            request,
            format!("key release failed: {source}"),
        ));
    }

    Ok(())
}

fn release_keys(enigo: &mut Enigo, keys: &[Key]) -> Option<String> {
    let mut first_error = None;
    for key in keys.iter().rev() {
        if let Err(source) = enigo.key(*key, Direction::Release)
            && first_error.is_none()
        {
            first_error = Some(source.to_string());
        }
    }
    first_error
}
