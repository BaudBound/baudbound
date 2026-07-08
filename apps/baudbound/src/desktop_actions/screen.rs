use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use serde_json::{Map, Number, Value};

#[cfg(not(windows))]
use super::config::failed_error;
use super::config::{config_string, required_string, required_u32};

pub(super) fn run_pixel_get(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let x = required_u32(request, "x")?;
    let y = required_u32(request, "y")?;
    let color = native_pixel_color(request, x, y)?;

    Ok(RuntimeActionResult { output_data: color })
}

pub(super) fn run_active_window(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    Ok(RuntimeActionResult {
        output_data: native_active_window(request)?,
    })
}

pub(super) fn run_window_focus(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let match_mode =
        config_string(request, "matchMode").unwrap_or_else(|| "window_title".to_owned());
    let target = required_string(request, "target")?;
    native_window_focus(request, &match_mode, &target)?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("focused".to_owned(), Value::Bool(true)),
            ("match_mode".to_owned(), Value::String(match_mode)),
            ("target".to_owned(), Value::String(target)),
        ]),
    })
}

#[cfg(windows)]
pub(super) fn native_pixel_color(
    request: &RuntimeActionRequest,
    x: u32,
    y: u32,
) -> Result<Map<String, Value>, RuntimeActionError> {
    super::windows_desktop::pixel_color(request, x, y)
}

#[cfg(not(windows))]
pub(super) fn native_pixel_color(
    request: &RuntimeActionRequest,
    _x: u32,
    _y: u32,
) -> Result<Map<String, Value>, RuntimeActionError> {
    unsupported_native(request, "screen pixel capture")
}

#[cfg(windows)]
pub(super) fn native_active_window(
    request: &RuntimeActionRequest,
) -> Result<Map<String, Value>, RuntimeActionError> {
    super::windows_desktop::active_window(request)
}

#[cfg(not(windows))]
pub(super) fn native_active_window(
    request: &RuntimeActionRequest,
) -> Result<Map<String, Value>, RuntimeActionError> {
    unsupported_native(request, "active window query")
}

#[cfg(windows)]
pub(super) fn native_window_focus(
    request: &RuntimeActionRequest,
    match_mode: &str,
    target: &str,
) -> Result<(), RuntimeActionError> {
    super::windows_desktop::window_focus(request, match_mode, target)
}

#[cfg(not(windows))]
pub(super) fn native_window_focus(
    request: &RuntimeActionRequest,
    _match_mode: &str,
    _target: &str,
) -> Result<(), RuntimeActionError> {
    unsupported_native(request, "window focus")
}

pub(in crate::desktop_actions) fn pixel_color_map(
    x: u32,
    y: u32,
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
) -> Map<String, Value> {
    let integer = (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue);
    Map::from_iter([
        ("x".to_owned(), Value::Number(Number::from(x))),
        ("y".to_owned(), Value::Number(Number::from(y))),
        (
            "hex".to_owned(),
            Value::String(format!("#{red:02X}{green:02X}{blue:02X}")),
        ),
        (
            "rgb".to_owned(),
            Value::Object(Map::from_iter([
                ("r".to_owned(), Value::Number(Number::from(red))),
                ("g".to_owned(), Value::Number(Number::from(green))),
                ("b".to_owned(), Value::Number(Number::from(blue))),
            ])),
        ),
        (
            "rgba".to_owned(),
            Value::Object(Map::from_iter([
                ("r".to_owned(), Value::Number(Number::from(red))),
                ("g".to_owned(), Value::Number(Number::from(green))),
                ("b".to_owned(), Value::Number(Number::from(blue))),
                ("a".to_owned(), Value::Number(Number::from(alpha))),
            ])),
        ),
        ("red".to_owned(), Value::Number(Number::from(red))),
        ("green".to_owned(), Value::Number(Number::from(green))),
        ("blue".to_owned(), Value::Number(Number::from(blue))),
        ("alpha".to_owned(), Value::Number(Number::from(alpha))),
        ("integer".to_owned(), Value::Number(Number::from(integer))),
    ])
}

#[cfg(not(windows))]
fn unsupported_native<T>(
    request: &RuntimeActionRequest,
    feature: &str,
) -> Result<T, RuntimeActionError> {
    Err(failed_error(
        request,
        format!("{feature} does not have a native backend for this platform yet"),
    ))
}
