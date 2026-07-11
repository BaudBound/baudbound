//! Headless action implementations for the BaudBound runner.

mod actions;

use std::{collections::BTreeMap, io::Write, sync::Arc, time::Duration};

use baudbound_runtime::{
    RuntimeActionError, RuntimeActionHandler, RuntimeActionRequest, RuntimeActionResult,
    RuntimeContext,
};
use serde_json::{Map, Number, Value};

pub use actions::SerialDeviceConfig;
use actions::{
    SerialDeviceSpec, beep_action, copy_file_action, delete_file_action, desktop_only_action,
    download_file_action, http_request_action, kill_process_action, move_file_action,
    open_application_action, process_status_action, read_file_action, run_process_action,
    serial_port_builder, shell_command_action, text_format_action, validate_usb_identity,
    webhook_response_action, write_file_action,
};

pub const SUPPORTED_ACTION_TYPES: &[&str] = &[
    "action.application.open",
    "action.beep",
    "action.clipboard",
    "action.file.copy",
    "action.file.delete",
    "action.file.download",
    "action.file.move",
    "action.file.read",
    "action.file.write",
    "action.http",
    "action.keyboard",
    "action.keyboard.type_text",
    "action.message_box",
    "action.mouse",
    "action.mouse.move",
    "action.notification",
    "action.pixel.get",
    "action.process.kill",
    "action.process.run",
    "action.process.status",
    "action.serial.write",
    "action.shell",
    "action.sound.play",
    "action.text.format",
    "action.webhook_response",
    "action.websocket.write",
    "action.window.active",
    "action.window.focus",
];

pub const DESKTOP_ADAPTER_ACTION_TYPES: &[&str] = &[
    "action.clipboard",
    "action.keyboard",
    "action.keyboard.type_text",
    "action.message_box",
    "action.mouse",
    "action.mouse.move",
    "action.notification",
    "action.pixel.get",
    "action.sound.play",
    "action.window.active",
    "action.window.focus",
];

#[derive(Default)]
pub struct HeadlessActionHandler {
    serial_devices: BTreeMap<String, SerialDeviceSpec>,
    websocket_sink: Option<Arc<dyn WebSocketMessageSink>>,
}

pub trait WebSocketMessageSink: Send + Sync {
    fn send_text(&self, connection_id: &str, message: &str) -> Result<usize, String>;
}

pub trait DesktopActionAdapter: Send + Sync {
    fn clipboard(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;

    fn message_box(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;

    fn notification(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;

    fn sound_play(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;

    fn keyboard(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;

    fn keyboard_type_text(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;

    fn mouse_click(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;

    fn mouse_move(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;

    fn pixel_get(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;

    fn active_window(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;

    fn window_focus(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError>;

    fn process_status_by_window_title(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        desktop_only_action(request, "process window-title queries")
    }

    fn kill_process_by_window_title(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        desktop_only_action(request, "process window-title termination")
    }
}

#[derive(Debug, Default)]
pub struct UnavailableDesktopActionAdapter;

impl DesktopActionAdapter for UnavailableDesktopActionAdapter {
    fn clipboard(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        desktop_only_action(request, "clipboard access")
    }

    fn message_box(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        desktop_only_action(request, "message boxes")
    }

    fn notification(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        desktop_only_action(request, "desktop notifications")
    }

    fn sound_play(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        desktop_only_action(request, "audio playback")
    }

    fn keyboard(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        desktop_only_action(request, "keyboard input")
    }

    fn keyboard_type_text(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        desktop_only_action(request, "keyboard text input")
    }

    fn mouse_click(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        desktop_only_action(request, "mouse clicks")
    }

    fn mouse_move(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        desktop_only_action(request, "mouse movement")
    }

    fn pixel_get(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        desktop_only_action(request, "screen pixel capture")
    }

    fn active_window(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        desktop_only_action(request, "active window query")
    }

    fn window_focus(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        desktop_only_action(request, "window focus")
    }
}

pub struct DesktopActionHandler<A> {
    adapter: A,
    headless: HeadlessActionHandler,
}

impl<A> DesktopActionHandler<A> {
    #[must_use]
    pub fn new(headless: HeadlessActionHandler, adapter: A) -> Self {
        Self { adapter, headless }
    }
}

impl<A> RuntimeActionHandler for DesktopActionHandler<A>
where
    A: DesktopActionAdapter,
{
    fn execute_action(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        match request.action_type.as_str() {
            "action.clipboard" => self.adapter.clipboard(request, context),
            "action.keyboard" => self.adapter.keyboard(request, context),
            "action.keyboard.type_text" => self.adapter.keyboard_type_text(request, context),
            "action.message_box" => self.adapter.message_box(request, context),
            "action.mouse" => self.adapter.mouse_click(request, context),
            "action.mouse.move" => self.adapter.mouse_move(request, context),
            "action.notification" => self.adapter.notification(request, context),
            "action.pixel.get" => self.adapter.pixel_get(request, context),
            "action.process.kill" if uses_window_title_match(request) => {
                self.adapter.kill_process_by_window_title(request, context)
            }
            "action.process.status" if uses_window_title_match(request) => self
                .adapter
                .process_status_by_window_title(request, context),
            "action.sound.play" => self.adapter.sound_play(request, context),
            "action.window.active" => self.adapter.active_window(request, context),
            "action.window.focus" => self.adapter.window_focus(request, context),
            _ => self.headless.execute_action(request, context),
        }
    }
}

fn uses_window_title_match(request: &RuntimeActionRequest) -> bool {
    request
        .config
        .get("matchMode")
        .and_then(Value::as_str)
        .is_some_and(|mode| mode.trim() == "window_title")
}

impl HeadlessActionHandler {
    #[must_use]
    pub fn from_serial_devices(devices: impl IntoIterator<Item = SerialDeviceConfig>) -> Self {
        Self {
            serial_devices: devices
                .into_iter()
                .filter_map(SerialDeviceSpec::from_config)
                .map(|device| (device.device_id.clone(), device))
                .collect(),
            websocket_sink: None,
        }
    }

    #[must_use]
    pub fn with_websocket_sink(mut self, sink: Arc<dyn WebSocketMessageSink>) -> Self {
        self.websocket_sink = Some(sink);
        self
    }
}

impl RuntimeActionHandler for HeadlessActionHandler {
    fn execute_action(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        match request.action_type.as_str() {
            "action.beep" => beep_action(request),
            "action.clipboard" => desktop_only_action(request, "clipboard access"),
            "action.file.copy" => copy_file_action(request),
            "action.file.delete" => delete_file_action(request),
            "action.file.download" => download_file_action(request),
            "action.file.move" => move_file_action(request),
            "action.file.read" => read_file_action(request),
            "action.file.write" => write_file_action(request),
            "action.http" => http_request_action(request),
            "action.message_box" => desktop_only_action(request, "message boxes"),
            "action.notification" => desktop_only_action(request, "desktop notifications"),
            "action.application.open" => open_application_action(request),
            "action.process.kill" => kill_process_action(request),
            "action.process.run" => run_process_action(request),
            "action.process.status" => process_status_action(request),
            "action.serial.write" => self.serial_write_action(request),
            "action.shell" => shell_command_action(request),
            "action.sound.play" => desktop_only_action(request, "audio playback"),
            "action.text.format" => text_format_action(request),
            "action.webhook_response" => webhook_response_action(request, context),
            "action.websocket.write" => self.websocket_write_action(request),
            action_type => Err(RuntimeActionError::Unsupported(action_type.to_owned())),
        }
    }
}

impl HeadlessActionHandler {
    fn serial_write_action(
        &self,
        request: &RuntimeActionRequest,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        let device_id = required_string(request, "deviceId")?;
        let data = required_string(request, "data")?;
        let line_ending =
            config_string(&request.config, "lineEnding").unwrap_or_else(|| "none".to_owned());
        let Some(device) = self.serial_devices.get(&device_id) else {
            return failed(
                request,
                format!("unknown serial device {device_id:?}; add a matching Serial Input Trigger"),
            );
        };

        validate_usb_identity(request, device)?;
        let mut payload = data.into_bytes();
        match line_ending.trim().to_ascii_lowercase().as_str() {
            "none" | "" => {}
            "lf" => payload.push(b'\n'),
            "crlf" => payload.extend_from_slice(b"\r\n"),
            other => return failed(request, format!("unsupported serial line ending {other:?}")),
        }

        let mut port = serial_port_builder(device, Duration::from_secs(5))
            .open()
            .map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("failed to open serial port {}: {source}", device.port),
            })?;
        port.write_all(&payload)
            .and_then(|_| port.flush())
            .map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("failed to write to serial port {}: {source}", device.port),
            })?;

        Ok(RuntimeActionResult {
            output_data: Map::from_iter([
                (
                    "device_id".to_owned(),
                    Value::String(device.device_id.clone()),
                ),
                ("port".to_owned(), Value::String(device.port.clone())),
                (
                    "bytes".to_owned(),
                    Value::Number(Number::from(payload.len())),
                ),
            ]),
        })
    }

    fn websocket_write_action(
        &self,
        request: &RuntimeActionRequest,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        let connection_id = required_string(request, "connectionId")?;
        let message = required_string(request, "message")?;
        let Some(sink) = &self.websocket_sink else {
            return failed(
                request,
                "WebSocket Write requires an active WebSocket trigger connection",
            );
        };

        let bytes = sink
            .send_text(&connection_id, &message)
            .map_err(|message| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message,
            })?;

        Ok(RuntimeActionResult {
            output_data: Map::from_iter([
                ("connection_id".to_owned(), Value::String(connection_id)),
                ("message".to_owned(), Value::String(message)),
                ("bytes".to_owned(), Value::Number(Number::from(bytes))),
            ]),
        })
    }
}

pub(crate) fn required_string(
    request: &RuntimeActionRequest,
    key: &str,
) -> Result<String, RuntimeActionError> {
    config_string(&request.config, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("missing required config field {key}"),
        })
}

pub(crate) fn config_string(config: &Map<String, Value>, key: &str) -> Option<String> {
    config.get(key).map(value_to_string)
}

pub(crate) fn config_usize(config: &Map<String, Value>, key: &str, fallback: usize) -> usize {
    optional_config_usize(config, key).unwrap_or(fallback)
}

pub(crate) fn optional_config_usize(config: &Map<String, Value>, key: &str) -> Option<usize> {
    match config.get(key) {
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|value| usize::try_from(value).ok()),
        Some(Value::String(value)) => value.trim().parse::<usize>().ok(),
        _ => None,
    }
}

pub(crate) fn config_bool(config: &Map<String, Value>, key: &str, fallback: bool) -> bool {
    match config.get(key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => true,
            "false" | "no" | "0" => false,
            _ => fallback,
        },
        _ => fallback,
    }
}

pub(crate) fn timeout_duration(
    request: &RuntimeActionRequest,
) -> Result<Duration, RuntimeActionError> {
    let seconds = number_from_config(&request.config, "timeoutSeconds").unwrap_or(30.0);
    if !seconds.is_finite() || seconds <= 0.0 {
        return failed(request, "timeoutSeconds must be a positive number");
    }
    Ok(Duration::from_secs_f64(seconds))
}

pub(crate) fn number_from_config(config: &Map<String, Value>, key: &str) -> Option<f64> {
    match config.get(key) {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(value)) => value.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub(crate) fn number_json(value: f64) -> Option<Value> {
    Number::from_f64(value).map(Value::Number)
}

pub(crate) fn failed<T>(
    request: &RuntimeActionRequest,
    message: impl Into<String>,
) -> Result<T, RuntimeActionError> {
    Err(RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: message.into(),
    })
}

pub(crate) fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

pub(crate) fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "list",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests;
