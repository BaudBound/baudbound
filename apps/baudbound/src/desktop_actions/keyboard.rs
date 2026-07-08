use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use enigo::{Direction, Enigo, Key, Keyboard};
use serde_json::{Map, Number, Value};

use super::{
    config::{failed_error, required_string},
    input::native_input,
};

pub(super) fn run_keyboard(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let key = required_string(request, "key")?;
    let mut enigo = native_input(request)?;
    press_key_combo(request, &mut enigo, &key)?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([("key".to_owned(), Value::String(key))]),
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
    combo: &str,
) -> Result<(), RuntimeActionError> {
    let parts = combo
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let Some((key, modifiers)) = parts.split_last() else {
        return Err(failed_error(request, "keyboard key cannot be empty"));
    };

    for modifier in modifiers {
        enigo
            .key(key_token(modifier), Direction::Press)
            .map_err(|source| failed_error(request, format!("modifier press failed: {source}")))?;
    }
    enigo
        .key(key_token(key), Direction::Click)
        .map_err(|source| failed_error(request, format!("key press failed: {source}")))?;
    for modifier in modifiers.iter().rev() {
        enigo
            .key(key_token(modifier), Direction::Release)
            .map_err(|source| {
                failed_error(request, format!("modifier release failed: {source}"))
            })?;
    }
    Ok(())
}

fn key_token(value: &str) -> Key {
    match value.trim() {
        "Alt" | "AltLeft" | "AltRight" => Key::Alt,
        "Backspace" => Key::Backspace,
        "CapsLock" => Key::CapsLock,
        "Control" | "Ctrl" | "ControlLeft" | "ControlRight" => Key::Control,
        "Delete" => Key::Delete,
        "Down" | "ArrowDown" => Key::DownArrow,
        "Enter" | "NumpadEnter" => Key::Return,
        "Escape" => Key::Escape,
        "F1" => Key::F1,
        "F2" => Key::F2,
        "F3" => Key::F3,
        "F4" => Key::F4,
        "F5" => Key::F5,
        "F6" => Key::F6,
        "F7" => Key::F7,
        "F8" => Key::F8,
        "F9" => Key::F9,
        "F10" => Key::F10,
        "F11" => Key::F11,
        "F12" => Key::F12,
        "Home" => Key::Home,
        "Left" | "ArrowLeft" => Key::LeftArrow,
        "Meta" | "Super" | "Windows" | "Command" => Key::Meta,
        "PageDown" => Key::PageDown,
        "PageUp" => Key::PageUp,
        "Right" | "ArrowRight" => Key::RightArrow,
        "Shift" | "ShiftLeft" | "ShiftRight" => Key::Shift,
        "Space" => Key::Space,
        "Tab" => Key::Tab,
        "Up" | "ArrowUp" => Key::UpArrow,
        other => {
            let mut chars = other.chars();
            match (chars.next(), chars.next()) {
                (Some(character), None) => Key::Unicode(character),
                _ => Key::Unicode(other.chars().next().unwrap_or_default()),
            }
        }
    }
}
