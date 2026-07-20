use baudbound_runtime::{
    RuntimeActionError, RuntimeActionRequest, RuntimeActionResult, RuntimeContext,
};
use enigo::Button;
use serde_json::{Map, Number, Value};

use super::{
    config::{config_bool, config_string, failed_error, required_i32},
    input::{InputAction, NativeInputState},
};

pub(super) fn run_mouse_click(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
    input_state: &NativeInputState,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let button = normalize_mouse_button(
        request,
        &config_string(request, "button").unwrap_or_else(|| "left".to_owned()),
    )?;
    let input_action = InputAction::from_request(request)?;
    let click_type = if input_action == InputAction::Press {
        normalize_mouse_click_type(
            &config_string(request, "clickType").unwrap_or_else(|| "single".to_owned()),
        )
        .ok_or_else(|| failed_error(request, "mouse click type must be single or double"))?
    } else {
        "single".to_owned()
    };
    let click_count = usize::from(click_type == "double") + 1;
    input_state.mouse(
        request,
        &context.identity.run_id,
        button,
        input_action,
        click_count,
    )?;

    let mut output_data = Map::from_iter([
        ("button".to_owned(), Value::String(button.name.to_owned())),
        (
            "input_action".to_owned(),
            Value::String(input_action.as_str().to_owned()),
        ),
    ]);
    if input_action == InputAction::Press {
        output_data.insert("click_type".to_owned(), Value::String(click_type));
    }
    Ok(RuntimeActionResult { output_data })
}

pub(super) fn run_mouse_move(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let x = required_i32(request, "x")?;
    let y = required_i32(request, "y")?;
    let relative = config_bool(request, "relative");
    if relative {
        super::screen_tools::move_cursor_relative(x, y)
            .map_err(|error| failed_error(request, error))?;
    } else {
        super::screen_tools::validate_coordinate(x, y)
            .map_err(|error| failed_error(request, error))?;
        super::screen_tools::move_cursor_absolute(x, y)
            .map_err(|error| failed_error(request, error))?;
    }

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("relative".to_owned(), Value::Bool(relative)),
            ("x".to_owned(), Value::Number(Number::from(x))),
            ("y".to_owned(), Value::Number(Number::from(y))),
        ]),
    })
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NormalizedMouseButton {
    pub(super) name: &'static str,
    pub(super) token: Button,
}

pub(super) fn normalize_mouse_button(
    request: &RuntimeActionRequest,
    value: &str,
) -> Result<NormalizedMouseButton, RuntimeActionError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "left" => Ok(NormalizedMouseButton {
            name: "left",
            token: Button::Left,
        }),
        "middle" => Ok(NormalizedMouseButton {
            name: "middle",
            token: Button::Middle,
        }),
        "right" => Ok(NormalizedMouseButton {
            name: "right",
            token: Button::Right,
        }),
        "back" => native_extended_mouse_button(request, "back"),
        "forward" => native_extended_mouse_button(request, "forward"),
        other => Err(failed_error(
            request,
            format!("unsupported mouse button {other:?}"),
        )),
    }
}

#[cfg(any(windows, unix))]
fn native_extended_mouse_button(
    request: &RuntimeActionRequest,
    value: &str,
) -> Result<NormalizedMouseButton, RuntimeActionError> {
    match value {
        "back" => Ok(NormalizedMouseButton {
            name: "back",
            token: Button::Back,
        }),
        "forward" => Ok(NormalizedMouseButton {
            name: "forward",
            token: Button::Forward,
        }),
        other => Err(failed_error(
            request,
            format!("unsupported mouse button {other:?}"),
        )),
    }
}

#[cfg(not(any(windows, unix)))]
fn native_extended_mouse_button(
    request: &RuntimeActionRequest,
    value: &str,
) -> Result<NormalizedMouseButton, RuntimeActionError> {
    Err(failed_error(
        request,
        format!("{value} mouse button does not have a native backend for this platform"),
    ))
}

pub(super) fn normalize_mouse_click_type(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "single" => Some("single"),
        "double" => Some("double"),
        _ => None,
    }
    .map(str::to_owned)
}
