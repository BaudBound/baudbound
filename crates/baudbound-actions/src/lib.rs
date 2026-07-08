//! Headless action implementations for the BaudBound runner.

use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose};
use baudbound_runtime::{
    RuntimeActionError, RuntimeActionHandler, RuntimeActionRequest, RuntimeActionResult,
    RuntimeContext,
};
use regex::Regex;
use reqwest::{
    Method, StatusCode,
    blocking::Client,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde_json::{Map, Number, Value};
use serialport::{
    DataBits, FlowControl, Parity, SerialPortBuilder, SerialPortType, StopBits, available_ports,
};
use sysinfo::{Pid, ProcessesToUpdate, Signal, System};

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
            "action.sound.play" => self.adapter.sound_play(request, context),
            "action.window.active" => self.adapter.active_window(request, context),
            "action.window.focus" => self.adapter.window_focus(request, context),
            _ => self.headless.execute_action(request, context),
        }
    }
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

fn beep_action(request: &RuntimeActionRequest) -> Result<RuntimeActionResult, RuntimeActionError> {
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

fn webhook_response_action(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let status_code = http_status_config(request, "statusCode", 200)?;
    let content_type =
        config_string(&request.config, "contentType").unwrap_or_else(|| "text/plain".to_owned());
    let body = config_string(&request.config, "body").unwrap_or_default();
    let headers = request_headers(request)?;
    let trigger_id = context
        .trigger_payload
        .get("trigger_id")
        .and_then(Value::as_str)
        .unwrap_or(&context.identity.trigger_node_id)
        .to_owned();

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("sent".to_owned(), Value::Bool(true)),
            (
                "status_code".to_owned(),
                Value::Number(Number::from(status_code)),
            ),
            ("content_type".to_owned(), Value::String(content_type)),
            (
                "headers".to_owned(),
                Value::Object(response_headers(&headers)),
            ),
            ("body".to_owned(), Value::String(body)),
            ("trigger_id".to_owned(), Value::String(trigger_id)),
        ]),
    })
}

fn desktop_only_action(
    request: &RuntimeActionRequest,
    capability: &str,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    failed(
        request,
        format!("{capability} requires the desktop runner action adapter"),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialDeviceConfig {
    pub auto_reconnect: bool,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub device_id: String,
    pub flow_control: String,
    pub parity: String,
    pub port: String,
    pub product_id: Option<String>,
    pub read_mode: String,
    pub stop_bits: String,
    pub validate_usb_identity: bool,
    pub vendor_id: Option<String>,
}

#[derive(Debug, Clone)]
struct SerialDeviceSpec {
    baud_rate: u32,
    data_bits: DataBits,
    device_id: String,
    flow_control: FlowControl,
    parity: Parity,
    port: String,
    product_id: Option<u16>,
    stop_bits: StopBits,
    validate_usb_identity: bool,
    vendor_id: Option<u16>,
}

impl SerialDeviceSpec {
    fn from_config(config: SerialDeviceConfig) -> Option<Self> {
        let device_id = config.device_id.trim().to_owned();
        let port = config.port.trim().to_owned();
        if device_id.is_empty() || port.is_empty() {
            return None;
        }

        Some(Self {
            baud_rate: config.baud_rate,
            data_bits: parse_data_bits(&config.data_bits.to_string()),
            device_id,
            flow_control: parse_flow_control(&config.flow_control),
            parity: parse_parity(&config.parity),
            port,
            product_id: config.product_id.and_then(|value| parse_usb_hex_id(&value)),
            stop_bits: parse_stop_bits(&config.stop_bits),
            validate_usb_identity: config.validate_usb_identity,
            vendor_id: config.vendor_id.and_then(|value| parse_usb_hex_id(&value)),
        })
    }
}

fn serial_port_builder(device: &SerialDeviceSpec, timeout: Duration) -> SerialPortBuilder {
    serialport::new(&device.port, device.baud_rate)
        .data_bits(device.data_bits)
        .flow_control(device.flow_control)
        .parity(device.parity)
        .stop_bits(device.stop_bits)
        .timeout(timeout)
}

fn validate_usb_identity(
    request: &RuntimeActionRequest,
    device: &SerialDeviceSpec,
) -> Result<(), RuntimeActionError> {
    if !device.validate_usb_identity {
        return Ok(());
    }

    let ports = available_ports().map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to list serial ports for USB identity validation: {source}"),
    })?;
    let Some(port) = ports.into_iter().find(|port| port.port_name == device.port) else {
        return failed(
            request,
            format!(
                "serial port {} was not found while validating USB identity",
                device.port
            ),
        );
    };
    let SerialPortType::UsbPort(info) = port.port_type else {
        return failed(
            request,
            format!(
                "serial port {} is not reported as a USB serial device",
                device.port
            ),
        );
    };

    if let Some(vendor_id) = device.vendor_id
        && info.vid != vendor_id
    {
        return failed(
            request,
            format!(
                "serial port {} vendor id mismatch: expected {:04X}, got {:04X}",
                device.port, vendor_id, info.vid
            ),
        );
    }
    if let Some(product_id) = device.product_id
        && info.pid != product_id
    {
        return failed(
            request,
            format!(
                "serial port {} product id mismatch: expected {:04X}, got {:04X}",
                device.port, product_id, info.pid
            ),
        );
    }

    Ok(())
}

fn read_file_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let path = required_string(request, "path")?;
    let encoding = config_string(&request.config, "encoding").unwrap_or_else(|| "utf-8".to_owned());
    if encoding != "utf-8" {
        return failed(request, format!("unsupported file encoding {encoding}"));
    }

    let bytes = fs::read(Path::new(&path)).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to read {path}: {source}"),
    })?;
    let content =
        String::from_utf8(bytes.clone()).map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("{path} is not valid UTF-8: {source}"),
        })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("path".to_owned(), Value::String(path)),
            ("content".to_owned(), Value::String(content)),
            ("bytes".to_owned(), Value::Number(Number::from(bytes.len()))),
        ]),
    })
}

fn http_request_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let method = request_method(request)?;
    let url = required_string(request, "url")?;
    let timeout = timeout_duration(request)?;
    let headers = request_headers(request)?;
    let user_agent = config_string(&request.config, "userAgent");
    let body = config_string(&request.config, "body").unwrap_or_default();

    let client = Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to build HTTP client: {source}"),
        })?;

    let started_at = Instant::now();
    let mut builder = client.request(method.clone(), &url).headers(headers);
    if let Some(user_agent) = user_agent.filter(|value| !value.trim().is_empty()) {
        builder = builder.header(reqwest::header::USER_AGENT, user_agent);
    }
    if method_allows_body(&method) && !body.is_empty() {
        builder = builder.body(body);
    }

    let response = builder
        .send()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("HTTP request {method} {url} failed: {source}"),
        })?;
    let duration_ms = elapsed_millis(started_at);
    let status = response.status();
    let headers = response_headers(response.headers());
    let body = response
        .text()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to read HTTP response body: {source}"),
        })?;
    let json_body = serde_json::from_str::<Value>(&body).ok();

    let mut output_data = Map::from_iter([
        (
            "status_code".to_owned(),
            Value::Number(Number::from(status.as_u16())),
        ),
        (
            "status_text".to_owned(),
            Value::String(status_text(status).to_owned()),
        ),
        ("headers".to_owned(), Value::Object(headers)),
        ("body".to_owned(), Value::String(body)),
        (
            "duration_ms".to_owned(),
            Value::Number(Number::from(duration_ms)),
        ),
    ]);
    if let Some(json_body) = json_body {
        output_data.insert("json".to_owned(), json_body);
    }

    Ok(RuntimeActionResult { output_data })
}

fn download_file_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let url = required_string(request, "url")?;
    let destination_path = required_string(request, "destinationPath")?;
    let overwrite = config_bool(&request.config, "overwrite", false);
    let destination = PathBuf::from(&destination_path);
    ensure_destination_available(request, &destination, overwrite)?;
    ensure_parent_directory(request, &destination)?;

    let client = Client::builder()
        .timeout(timeout_duration(request)?)
        .build()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to build HTTP client: {source}"),
        })?;
    let response = client
        .get(&url)
        .send()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("download request {url} failed: {source}"),
        })?;
    let status = response.status();
    if !status.is_success() {
        return failed(
            request,
            format!("download request {url} returned {}", status.as_u16()),
        );
    }

    let bytes = response
        .bytes()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to read download response body: {source}"),
        })?;
    fs::write(&destination, &bytes).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to write download to {destination_path}: {source}"),
    })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("path".to_owned(), Value::String(destination_path)),
            ("url".to_owned(), Value::String(url)),
            ("bytes".to_owned(), Value::Number(Number::from(bytes.len()))),
        ]),
    })
}

fn write_file_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let path = required_string(request, "path")?;
    let content = config_string(&request.config, "content").unwrap_or_default();
    let mode = config_string(&request.config, "mode").unwrap_or_else(|| "overwrite".to_owned());
    let path_buf = PathBuf::from(&path);

    if let Some(parent) = path_buf
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to create parent directory for {path}: {source}"),
        })?;
    }

    let mut options = fs::OpenOptions::new();
    options.create(true).write(true);
    match mode.as_str() {
        "append" => {
            options.append(true);
        }
        "overwrite" => {
            options.truncate(true);
        }
        other => return failed(request, format!("unsupported file write mode {other}")),
    }

    let mut file = options
        .open(&path_buf)
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to open {path} for writing: {source}"),
        })?;
    file.write_all(content.as_bytes())
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to write {path}: {source}"),
        })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("path".to_owned(), Value::String(path)),
            ("mode".to_owned(), Value::String(mode)),
            (
                "bytes".to_owned(),
                Value::Number(Number::from(content.len())),
            ),
        ]),
    })
}

fn copy_file_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let source_path = required_string(request, "sourcePath")?;
    let destination_path = required_string(request, "destinationPath")?;
    let overwrite = config_bool(&request.config, "overwrite", false);
    let source = PathBuf::from(&source_path);
    let destination = PathBuf::from(&destination_path);

    ensure_destination_available(request, &destination, overwrite)?;
    ensure_parent_directory(request, &destination)?;

    let bytes = fs::copy(&source, &destination).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to copy {source_path} to {destination_path}: {source}"),
    })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("source_path".to_owned(), Value::String(source_path)),
            (
                "destination_path".to_owned(),
                Value::String(destination_path),
            ),
            ("bytes".to_owned(), Value::Number(Number::from(bytes))),
        ]),
    })
}

fn move_file_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let source_path = required_string(request, "sourcePath")?;
    let destination_path = required_string(request, "destinationPath")?;
    let overwrite = config_bool(&request.config, "overwrite", false);
    let source = PathBuf::from(&source_path);
    let destination = PathBuf::from(&destination_path);

    ensure_destination_available(request, &destination, overwrite)?;
    ensure_parent_directory(request, &destination)?;
    if overwrite && destination.exists() {
        fs::remove_file(&destination).map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to remove existing {destination_path}: {source}"),
        })?;
    }

    fs::rename(&source, &destination).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to move {source_path} to {destination_path}: {source}"),
    })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("source_path".to_owned(), Value::String(source_path)),
            (
                "destination_path".to_owned(),
                Value::String(destination_path),
            ),
        ]),
    })
}

fn delete_file_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let path = required_string(request, "path")?;
    let path_buf = PathBuf::from(&path);
    let metadata = fs::metadata(&path_buf).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to inspect {path}: {source}"),
    })?;
    if !metadata.is_file() {
        return failed(request, format!("{path} is not a regular file"));
    }

    fs::remove_file(&path_buf).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to delete {path}: {source}"),
    })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([("path".to_owned(), Value::String(path))]),
    })
}

fn process_status_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let target = required_string(request, "target")?;
    let match_mode =
        config_string(&request.config, "matchMode").unwrap_or_else(|| "process_name".to_owned());
    let mut system = process_system();
    let process = find_process(request, &system, &match_mode, &target)?;

    let output_data = match process {
        Some(process) => process_status_output(process, true),
        None => Map::from_iter([
            ("running".to_owned(), Value::Bool(false)),
            ("state".to_owned(), Value::String("not_found".to_owned())),
            ("process_id".to_owned(), Value::Null),
            ("process_name".to_owned(), Value::Null),
            ("executable_path".to_owned(), Value::Null),
        ]),
    };

    system.refresh_processes(ProcessesToUpdate::All, true);
    Ok(RuntimeActionResult { output_data })
}

fn kill_process_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let target = required_string(request, "target")?;
    let match_mode =
        config_string(&request.config, "matchMode").unwrap_or_else(|| "process_name".to_owned());
    let system = process_system();
    let Some(process) = find_process(request, &system, &match_mode, &target)? else {
        return failed(
            request,
            format!("no process matched {match_mode} target {target}"),
        );
    };

    let mut output_data = process_status_output(process, true);
    let killed = process
        .kill_with(Signal::Kill)
        .unwrap_or_else(|| process.kill());
    output_data.insert("killed".to_owned(), Value::Bool(killed));
    if !killed {
        return failed(
            request,
            format!(
                "failed to terminate process {} ({})",
                process.pid(),
                process.name().to_string_lossy()
            ),
        );
    }

    Ok(RuntimeActionResult { output_data })
}

fn open_application_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let application = required_string(request, "application")?;
    let arguments = config_string(&request.config, "arguments").unwrap_or_default();
    let parsed_arguments = parse_command_arguments(request, &arguments)?;

    let mut command = Command::new(&application);
    command
        .args(&parsed_arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command
        .spawn()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to open application {application}: {source}"),
        })?;
    let process_id = child.id();

    thread::spawn(move || {
        let _ = child.wait();
    });

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("application_id".to_owned(), Value::String(application)),
            (
                "process_id".to_owned(),
                Value::Number(Number::from(process_id)),
            ),
            (
                "arguments".to_owned(),
                Value::Array(parsed_arguments.into_iter().map(Value::String).collect()),
            ),
        ]),
    })
}

fn run_process_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let executable = required_string(request, "executable")?;
    let arguments = config_string(&request.config, "arguments").unwrap_or_default();
    let working_directory = config_string(&request.config, "workingDirectory").unwrap_or_default();
    let parsed_arguments = parse_command_arguments(request, &arguments)?;

    let mut command = Command::new(&executable);
    command.args(&parsed_arguments);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if !working_directory.trim().is_empty() {
        command.current_dir(&working_directory);
    }

    let child = command
        .spawn()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to start process {executable}: {source}"),
        })?;
    let process_id = child.id();
    let output = child
        .wait_with_output()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed while waiting for process {executable}: {source}"),
        })?;

    Ok(process_result(
        process_id,
        output.status.code(),
        output.stdout,
        output.stderr,
    ))
}

fn shell_command_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let command = required_string(request, "command")?;
    let mut shell = platform_shell_command(&command);
    shell.stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = shell.spawn().map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to start shell command: {source}"),
    })?;
    let process_id = child.id();
    let output = child
        .wait_with_output()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed while waiting for shell command: {source}"),
        })?;

    Ok(process_result(
        process_id,
        output.status.code(),
        output.stdout,
        output.stderr,
    ))
}

fn text_format_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let operation =
        config_string(&request.config, "operation").unwrap_or_else(|| "template".to_owned());
    let input = config_string(&request.config, "input").unwrap_or_default();
    let template = config_string(&request.config, "template").unwrap_or_default();
    let search = config_string(&request.config, "search").unwrap_or_default();
    let replacement = config_string(&request.config, "replacement").unwrap_or_default();
    let delimiter = config_string(&request.config, "delimiter").unwrap_or_else(|| ",".to_owned());
    let pad = config_string(&request.config, "pad").unwrap_or_else(|| " ".to_owned());

    let (text, items) = match operation.as_str() {
        "template" => (template, Vec::new()),
        "trim" => (input.trim().to_owned(), Vec::new()),
        "uppercase" => (input.to_uppercase(), Vec::new()),
        "lowercase" => (input.to_lowercase(), Vec::new()),
        "replace" => (input.replace(&search, &replacement), Vec::new()),
        "regex_replace" => {
            let regex = Regex::new(&search).map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("invalid regex pattern: {source}"),
            })?;
            (
                regex.replace_all(&input, replacement.as_str()).to_string(),
                Vec::new(),
            )
        }
        "split" => (
            String::new(),
            input
                .split(&delimiter)
                .map(|item| Value::String(item.to_owned()))
                .collect(),
        ),
        "join" => {
            let items = parse_items(request)?;
            let text = items
                .iter()
                .map(value_to_string)
                .collect::<Vec<_>>()
                .join(&delimiter);
            (text, items)
        }
        "substring" => {
            let start = config_usize(&request.config, "start", 0);
            let length = optional_config_usize(&request.config, "length");
            (substring_by_chars(&input, start, length), Vec::new())
        }
        "pad_start" => (
            pad_text(
                &input,
                config_usize(&request.config, "targetLength", input.chars().count()),
                &pad,
                true,
            ),
            Vec::new(),
        ),
        "pad_end" => (
            pad_text(
                &input,
                config_usize(&request.config, "targetLength", input.chars().count()),
                &pad,
                false,
            ),
            Vec::new(),
        ),
        "url_encode" => (urlencoding::encode(&input).into_owned(), Vec::new()),
        "url_decode" => (
            urlencoding::decode(&input)
                .map_err(|source| RuntimeActionError::Failed {
                    action_type: request.action_type.clone(),
                    message: format!("invalid URL encoded input: {source}"),
                })?
                .into_owned(),
            Vec::new(),
        ),
        "base64_encode" => (
            general_purpose::STANDARD.encode(input.as_bytes()),
            Vec::new(),
        ),
        "base64_decode" => {
            let bytes = general_purpose::STANDARD
                .decode(input.trim())
                .map_err(|source| RuntimeActionError::Failed {
                    action_type: request.action_type.clone(),
                    message: format!("invalid base64 input: {source}"),
                })?;
            let text = String::from_utf8(bytes).map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("decoded base64 is not valid UTF-8: {source}"),
            })?;
            (text, Vec::new())
        }
        "json_escape" => (
            serde_json::to_string(&input).map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("failed to JSON escape input: {source}"),
            })?,
            Vec::new(),
        ),
        "json_unescape" => {
            let value = serde_json::from_str::<Value>(&input).map_err(|source| {
                RuntimeActionError::Failed {
                    action_type: request.action_type.clone(),
                    message: format!("failed to JSON unescape input: {source}"),
                }
            })?;
            (value_to_string(&value), Vec::new())
        }
        _ => {
            return failed(
                request,
                format!("unsupported text transform operation {operation}"),
            );
        }
    };

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("text".to_owned(), Value::String(text)),
            ("items".to_owned(), Value::Array(items)),
        ]),
    })
}

fn parse_items(request: &RuntimeActionRequest) -> Result<Vec<Value>, RuntimeActionError> {
    match request.config.get("items") {
        Some(Value::Array(items)) => Ok(items.clone()),
        Some(Value::String(items)) => {
            let parsed = serde_json::from_str::<Value>(items).map_err(|source| {
                RuntimeActionError::Failed {
                    action_type: request.action_type.clone(),
                    message: format!("join items must be a JSON array: {source}"),
                }
            })?;
            match parsed {
                Value::Array(items) => Ok(items),
                _ => failed(request, "join items must be a JSON array"),
            }
        }
        Some(other) => failed(
            request,
            format!("join items must be a list, found {}", value_kind(other)),
        ),
        None => Ok(Vec::new()),
    }
}

fn required_string(
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

fn config_string(config: &Map<String, Value>, key: &str) -> Option<String> {
    config.get(key).map(value_to_string)
}

fn config_usize(config: &Map<String, Value>, key: &str, fallback: usize) -> usize {
    optional_config_usize(config, key).unwrap_or(fallback)
}

fn optional_config_usize(config: &Map<String, Value>, key: &str) -> Option<usize> {
    match config.get(key) {
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|value| usize::try_from(value).ok()),
        Some(Value::String(value)) => value.trim().parse::<usize>().ok(),
        _ => None,
    }
}

fn parse_data_bits(value: &str) -> DataBits {
    match value.trim() {
        "5" => DataBits::Five,
        "6" => DataBits::Six,
        "7" => DataBits::Seven,
        _ => DataBits::Eight,
    }
}

fn parse_stop_bits(value: &str) -> StopBits {
    match value.trim() {
        "2" => StopBits::Two,
        _ => StopBits::One,
    }
}

fn parse_parity(value: &str) -> Parity {
    match value.trim().to_ascii_lowercase().as_str() {
        "even" => Parity::Even,
        "odd" => Parity::Odd,
        _ => Parity::None,
    }
}

fn parse_flow_control(value: &str) -> FlowControl {
    match value.trim().to_ascii_lowercase().as_str() {
        "hardware" => FlowControl::Hardware,
        "software" => FlowControl::Software,
        _ => FlowControl::None,
    }
}

fn parse_usb_hex_id(value: &str) -> Option<u16> {
    let trimmed = value
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if trimmed.is_empty()
        || trimmed.len() > 4
        || !trimmed.chars().all(|value| value.is_ascii_hexdigit())
    {
        return None;
    }
    u16::from_str_radix(trimmed, 16).ok()
}

fn config_bool(config: &Map<String, Value>, key: &str, fallback: bool) -> bool {
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

fn timeout_duration(request: &RuntimeActionRequest) -> Result<Duration, RuntimeActionError> {
    let seconds = number_from_config(&request.config, "timeoutSeconds").unwrap_or(30.0);
    if !seconds.is_finite() || seconds <= 0.0 {
        return failed(request, "timeoutSeconds must be a positive number");
    }
    Ok(Duration::from_secs_f64(seconds))
}

fn http_status_config(
    request: &RuntimeActionRequest,
    key: &str,
    fallback: u16,
) -> Result<u16, RuntimeActionError> {
    let status = number_from_config(&request.config, key).unwrap_or(f64::from(fallback));
    if !status.is_finite() || status.fract() != 0.0 || !(100.0..=599.0).contains(&status) {
        return failed(
            request,
            format!("{key} must be an HTTP status code 100-599"),
        );
    }
    Ok(status as u16)
}

fn request_method(request: &RuntimeActionRequest) -> Result<Method, RuntimeActionError> {
    let method = config_string(&request.config, "method").unwrap_or_else(|| "GET".to_owned());
    Method::from_bytes(method.trim().as_bytes()).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("invalid HTTP method {method}: {source}"),
    })
}

fn request_headers(request: &RuntimeActionRequest) -> Result<HeaderMap, RuntimeActionError> {
    let mut headers = HeaderMap::new();
    match request.config.get("headers") {
        Some(Value::Array(rows)) => {
            for row in rows {
                let Some(row) = row.as_object() else {
                    continue;
                };
                let name = row.get("name").map(value_to_string).unwrap_or_default();
                let value = row.get("value").map(value_to_string).unwrap_or_default();
                insert_header(request, &mut headers, name, value)?;
            }
        }
        Some(Value::Object(values)) => {
            for (name, value) in values {
                insert_header(request, &mut headers, name.clone(), value_to_string(value))?;
            }
        }
        Some(Value::Null) | None => {}
        Some(other) => {
            return failed(
                request,
                format!(
                    "headers must be a list or object, found {}",
                    value_kind(other)
                ),
            );
        }
    }
    Ok(headers)
}

fn insert_header(
    request: &RuntimeActionRequest,
    headers: &mut HeaderMap,
    name: String,
    value: String,
) -> Result<(), RuntimeActionError> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(());
    }
    let header_name =
        HeaderName::from_bytes(name.as_bytes()).map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("invalid HTTP header name {name}: {source}"),
        })?;
    let header_value =
        HeaderValue::from_str(&value).map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("invalid HTTP header value for {name}: {source}"),
        })?;
    headers.insert(header_name, header_value);
    Ok(())
}

fn response_headers(headers: &HeaderMap) -> Map<String, Value> {
    let mut values = Map::new();
    for (name, value) in headers {
        if let Ok(value) = value.to_str() {
            values.insert(name.as_str().to_owned(), Value::String(value.to_owned()));
        }
    }
    values
}

fn status_text(status: StatusCode) -> &'static str {
    status.canonical_reason().unwrap_or("")
}

fn method_allows_body(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD)
}

fn elapsed_millis(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn number_from_config(config: &Map<String, Value>, key: &str) -> Option<f64> {
    match config.get(key) {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(value)) => value.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn number_json(value: f64) -> Option<Value> {
    Number::from_f64(value).map(Value::Number)
}

fn ensure_destination_available(
    request: &RuntimeActionRequest,
    destination: &Path,
    overwrite: bool,
) -> Result<(), RuntimeActionError> {
    if destination.exists() && !overwrite {
        return failed(
            request,
            format!(
                "destination {} already exists and overwrite is disabled",
                destination.display()
            ),
        );
    }
    if destination.exists() && !destination.is_file() {
        return failed(
            request,
            format!(
                "destination {} is not a regular file",
                destination.display()
            ),
        );
    }
    Ok(())
}

fn ensure_parent_directory(
    request: &RuntimeActionRequest,
    destination: &Path,
) -> Result<(), RuntimeActionError> {
    let Some(parent) = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    fs::create_dir_all(parent).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!(
            "failed to create parent directory for {}: {source}",
            destination.display()
        ),
    })
}

fn process_result(
    process_id: u32,
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
) -> RuntimeActionResult {
    RuntimeActionResult {
        output_data: Map::from_iter([
            (
                "process_id".to_owned(),
                Value::Number(Number::from(process_id)),
            ),
            (
                "exit_code".to_owned(),
                exit_code.map_or(Value::Null, |code| Value::Number(Number::from(code))),
            ),
            (
                "success".to_owned(),
                Value::Bool(exit_code.is_some_and(|code| code == 0)),
            ),
            (
                "stdout".to_owned(),
                Value::String(String::from_utf8_lossy(&stdout).to_string()),
            ),
            (
                "stderr".to_owned(),
                Value::String(String::from_utf8_lossy(&stderr).to_string()),
            ),
        ]),
    }
}

fn platform_shell_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut shell = Command::new("cmd");
        shell.args(["/C", command]);
        shell
    }

    #[cfg(not(windows))]
    {
        let mut shell = Command::new("sh");
        shell.args(["-c", command]);
        shell
    }
}

fn process_system() -> System {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system
}

fn find_process<'a>(
    request: &RuntimeActionRequest,
    system: &'a System,
    match_mode: &str,
    target: &str,
) -> Result<Option<&'a sysinfo::Process>, RuntimeActionError> {
    match match_mode {
        "pid" => {
            let process_id =
                target
                    .trim()
                    .parse::<usize>()
                    .map_err(|source| RuntimeActionError::Failed {
                        action_type: request.action_type.clone(),
                        message: format!("invalid process id {target}: {source}"),
                    })?;
            Ok(system.process(Pid::from(process_id)))
        }
        "process_name" => Ok(system.processes().values().find(|process| {
            process
                .name()
                .to_string_lossy()
                .eq_ignore_ascii_case(target.trim())
        })),
        "executable_path" => {
            let normalized_target = normalize_path_string(target);
            Ok(system.processes().values().find(|process| {
                process
                    .exe()
                    .map(|path| normalize_path_string(&path.display().to_string()))
                    .is_some_and(|path| path == normalized_target)
            }))
        }
        "window_title" => Err(RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: "window title matching is only available in the desktop runner".to_owned(),
        }),
        other => Err(RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("unsupported process match mode {other}"),
        }),
    }
}

fn process_status_output(process: &sysinfo::Process, running: bool) -> Map<String, Value> {
    Map::from_iter([
        ("running".to_owned(), Value::Bool(running)),
        (
            "state".to_owned(),
            Value::String(if running { "running" } else { "not_found" }.to_owned()),
        ),
        (
            "process_id".to_owned(),
            Value::Number(Number::from(process.pid().as_u32())),
        ),
        (
            "process_name".to_owned(),
            Value::String(process.name().to_string_lossy().to_string()),
        ),
        (
            "executable_path".to_owned(),
            process.exe().map_or(Value::Null, |path| {
                Value::String(path.display().to_string())
            }),
        ),
    ])
}

fn normalize_path_string(path: &str) -> String {
    path.trim().replace('\\', "/").to_ascii_lowercase()
}

fn parse_command_arguments(
    request: &RuntimeActionRequest,
    input: &str,
) -> Result<Vec<String>, RuntimeActionError> {
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut quote = None::<char>;
    let mut escaped = false;

    while let Some(character) = chars.next() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        match quote {
            Some(active_quote) if character == active_quote => quote = None,
            Some(_) => current.push(character),
            None if matches!(character, '"' | '\'') => quote = Some(character),
            None if character.is_whitespace() => {
                if !current.is_empty() {
                    arguments.push(std::mem::take(&mut current));
                }
                while matches!(chars.peek(), Some(next) if next.is_whitespace()) {
                    chars.next();
                }
            }
            None => current.push(character),
        }
    }

    if escaped {
        current.push('\\');
    }
    if quote.is_some() {
        return failed(
            request,
            "process arguments contain an unterminated quoted string",
        );
    }
    if !current.is_empty() {
        arguments.push(current);
    }

    Ok(arguments)
}

fn substring_by_chars(input: &str, start: usize, length: Option<usize>) -> String {
    let chars = input.chars().skip(start);
    match length {
        Some(length) => chars.take(length).collect(),
        None => chars.collect(),
    }
}

fn pad_text(input: &str, target_length: usize, pad: &str, start: bool) -> String {
    let current_length = input.chars().count();
    if current_length >= target_length || pad.is_empty() {
        return input.to_owned();
    }

    let mut fill = String::new();
    while fill.chars().count() < target_length - current_length {
        fill.push_str(pad);
    }
    let fill = substring_by_chars(&fill, 0, Some(target_length - current_length));

    if start {
        format!("{fill}{input}")
    } else {
        format!("{input}{fill}")
    }
}

fn failed<T>(
    request: &RuntimeActionRequest,
    message: impl Into<String>,
) -> Result<T, RuntimeActionError> {
    Err(RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: message.into(),
    })
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

fn value_kind(value: &Value) -> &'static str {
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
mod tests {
    use std::{
        fs,
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
        thread,
    };

    use baudbound_runtime::{
        RunIdentity, RuntimeActionHandler, RuntimeActionRequest, RuntimeContext,
    };
    use serde_json::{Map, Value, json};

    use super::{
        DesktopActionAdapter, DesktopActionHandler, HeadlessActionHandler, SerialDeviceConfig,
        WebSocketMessageSink,
    };

    #[derive(Default)]
    struct FakeWebSocketSink {
        sent: Mutex<Vec<(String, String)>>,
    }

    impl WebSocketMessageSink for FakeWebSocketSink {
        fn send_text(&self, connection_id: &str, message: &str) -> Result<usize, String> {
            self.sent
                .lock()
                .expect("fake sink lock should not be poisoned")
                .push((connection_id.to_owned(), message.to_owned()));
            Ok(message.as_bytes().len())
        }
    }

    #[derive(Default)]
    struct FakeDesktopAdapter {
        called: Mutex<Vec<String>>,
    }

    impl DesktopActionAdapter for FakeDesktopAdapter {
        fn clipboard(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError>
        {
            self.record(request);
            Ok(baudbound_runtime::RuntimeActionResult {
                output_data: Map::from_iter([("handled".to_owned(), json!("clipboard"))]),
            })
        }

        fn message_box(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError>
        {
            self.record(request);
            Ok(baudbound_runtime::RuntimeActionResult {
                output_data: Map::from_iter([("handled".to_owned(), json!("message_box"))]),
            })
        }

        fn notification(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError>
        {
            self.record(request);
            Ok(baudbound_runtime::RuntimeActionResult {
                output_data: Map::from_iter([("handled".to_owned(), json!("notification"))]),
            })
        }

        fn sound_play(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError>
        {
            self.record(request);
            Ok(baudbound_runtime::RuntimeActionResult {
                output_data: Map::from_iter([("handled".to_owned(), json!("sound_play"))]),
            })
        }

        fn keyboard(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError>
        {
            self.record(request);
            Ok(baudbound_runtime::RuntimeActionResult {
                output_data: Map::from_iter([("handled".to_owned(), json!("keyboard"))]),
            })
        }

        fn keyboard_type_text(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError>
        {
            self.record(request);
            Ok(baudbound_runtime::RuntimeActionResult {
                output_data: Map::from_iter([("handled".to_owned(), json!("keyboard_type_text"))]),
            })
        }

        fn mouse_click(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError>
        {
            self.record(request);
            Ok(baudbound_runtime::RuntimeActionResult {
                output_data: Map::from_iter([("handled".to_owned(), json!("mouse_click"))]),
            })
        }

        fn mouse_move(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError>
        {
            self.record(request);
            Ok(baudbound_runtime::RuntimeActionResult {
                output_data: Map::from_iter([("handled".to_owned(), json!("mouse_move"))]),
            })
        }

        fn pixel_get(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError>
        {
            self.record(request);
            Ok(baudbound_runtime::RuntimeActionResult {
                output_data: Map::from_iter([("handled".to_owned(), json!("pixel_get"))]),
            })
        }

        fn active_window(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError>
        {
            self.record(request);
            Ok(baudbound_runtime::RuntimeActionResult {
                output_data: Map::from_iter([("handled".to_owned(), json!("active_window"))]),
            })
        }

        fn window_focus(
            &self,
            request: &RuntimeActionRequest,
            _context: &RuntimeContext,
        ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError>
        {
            self.record(request);
            Ok(baudbound_runtime::RuntimeActionResult {
                output_data: Map::from_iter([("handled".to_owned(), json!("window_focus"))]),
            })
        }
    }

    impl FakeDesktopAdapter {
        fn record(&self, request: &RuntimeActionRequest) {
            self.called
                .lock()
                .expect("fake adapter lock should not be poisoned")
                .push(request.action_type.clone());
        }
    }

    #[test]
    fn reads_utf8_file() {
        let directory = tempfile::tempdir().expect("tempdir should be created");
        let path = directory.path().join("input.txt");
        fs::write(&path, "hello").expect("file should be written");

        let result = execute(
            "action.file.read",
            json!({ "path": path.display().to_string(), "encoding": "utf-8" }),
        )
        .expect("file read should succeed");

        assert_eq!(result.output_data.get("content"), Some(&json!("hello")));
        assert_eq!(result.output_data.get("bytes"), Some(&json!(5)));
    }

    #[test]
    fn writes_and_appends_file_content() {
        let directory = tempfile::tempdir().expect("tempdir should be created");
        let path = directory.path().join("nested").join("output.txt");

        let overwrite = execute(
            "action.file.write",
            json!({
                "mode": "overwrite",
                "path": path.display().to_string(),
                "content": "hello"
            }),
        )
        .expect("file write should succeed");
        assert_eq!(overwrite.output_data.get("bytes"), Some(&json!(5)));

        execute(
            "action.file.write",
            json!({
                "mode": "append",
                "path": path.display().to_string(),
                "content": " world"
            }),
        )
        .expect("file append should succeed");

        assert_eq!(
            fs::read_to_string(&path).expect("file should read"),
            "hello world"
        );
    }

    #[test]
    fn copies_file_and_respects_overwrite_flag() {
        let directory = tempfile::tempdir().expect("tempdir should be created");
        let source = directory.path().join("input.txt");
        let destination = directory.path().join("out").join("input.txt");
        fs::write(&source, "one").expect("source should write");
        fs::write(directory.path().join("existing.txt"), "old").expect("destination should write");

        let result = execute(
            "action.file.copy",
            json!({
                "sourcePath": source.display().to_string(),
                "destinationPath": destination.display().to_string(),
                "overwrite": "false"
            }),
        )
        .expect("file copy should succeed");
        assert_eq!(result.output_data.get("bytes"), Some(&json!(3)));
        assert_eq!(
            fs::read_to_string(&destination).expect("destination should read"),
            "one"
        );

        let error = execute(
            "action.file.copy",
            json!({
                "sourcePath": source.display().to_string(),
                "destinationPath": destination.display().to_string(),
                "overwrite": "false"
            }),
        )
        .expect_err("copy should reject existing destination");
        assert!(error.to_string().contains("overwrite is disabled"));
    }

    #[test]
    fn moves_file_and_removes_source() {
        let directory = tempfile::tempdir().expect("tempdir should be created");
        let source = directory.path().join("input.txt");
        let destination = directory.path().join("archive").join("input.txt");
        fs::write(&source, "move me").expect("source should write");

        execute(
            "action.file.move",
            json!({
                "sourcePath": source.display().to_string(),
                "destinationPath": destination.display().to_string(),
                "overwrite": false
            }),
        )
        .expect("file move should succeed");

        assert!(!source.exists());
        assert_eq!(
            fs::read_to_string(&destination).expect("destination should read"),
            "move me"
        );
    }

    #[test]
    fn deletes_regular_file_only() {
        let directory = tempfile::tempdir().expect("tempdir should be created");
        let file = directory.path().join("delete-me.txt");
        fs::write(&file, "delete me").expect("file should write");

        execute(
            "action.file.delete",
            json!({ "path": file.display().to_string() }),
        )
        .expect("file delete should succeed");
        assert!(!file.exists());

        let error = execute(
            "action.file.delete",
            json!({ "path": directory.path().display().to_string() }),
        )
        .expect_err("directory delete should fail");
        assert!(error.to_string().contains("not a regular file"));
    }

    #[test]
    fn emits_beep_result_metadata() {
        let result = execute(
            "action.beep",
            json!({
                "frequencyHz": "880",
                "durationMs": "1"
            }),
        )
        .expect("beep should succeed");

        assert_eq!(result.output_data.get("frequency_hz"), Some(&json!(880.0)));
        assert_eq!(result.output_data.get("duration_ms"), Some(&json!(1.0)));
    }

    #[test]
    fn prepares_webhook_response_data() {
        let result = execute_with_trigger_payload(
            "action.webhook_response",
            json!({
                "statusCode": "202",
                "contentType": "application/json",
                "headers": [{ "id": "h-1", "name": "Cache-Control", "value": "no-store" }],
                "body": "{\"queued\":true}"
            }),
            json!({ "trigger_id": "n-webhook" }),
        )
        .expect("webhook response should prepare");

        assert_eq!(result.output_data.get("sent"), Some(&json!(true)));
        assert_eq!(result.output_data.get("status_code"), Some(&json!(202)));
        assert_eq!(
            result.output_data.get("content_type"),
            Some(&json!("application/json"))
        );
        assert_eq!(
            result.output_data.get("trigger_id"),
            Some(&json!("n-webhook"))
        );
        assert_eq!(
            result
                .output_data
                .get("headers")
                .and_then(|headers| headers.get("cache-control")),
            Some(&json!("no-store"))
        );
    }

    #[test]
    fn indexes_serial_devices_from_runner_config() {
        let handler = HeadlessActionHandler::from_serial_devices([serial_device_config()]);

        let device = handler
            .serial_devices
            .get("main-device")
            .expect("serial device should be indexed");

        assert_eq!(device.port, "COM3");
        assert_eq!(device.baud_rate, 115_200);
    }

    #[test]
    fn serial_write_rejects_unknown_device() {
        let error = execute(
            "action.serial.write",
            json!({
                "deviceId": "missing-device",
                "data": "ping",
                "lineEnding": "none"
            }),
        )
        .expect_err("unknown serial device should fail");

        assert!(error.to_string().contains("unknown serial device"));
    }

    #[test]
    fn websocket_write_uses_configured_sink() {
        let sink = Arc::new(FakeWebSocketSink::default());
        let handler_sink: Arc<dyn WebSocketMessageSink> = sink.clone();
        let handler = HeadlessActionHandler::default().with_websocket_sink(handler_sink);
        let result = execute_with_handler(
            &handler,
            "action.websocket.write",
            json!({
                "connectionId": "conn-1",
                "message": "hello"
            }),
            Value::Null,
        )
        .expect("websocket write should use sink");

        assert_eq!(
            result.output_data.get("connection_id"),
            Some(&json!("conn-1"))
        );
        assert_eq!(result.output_data.get("bytes"), Some(&json!(5)));
        assert_eq!(
            sink.sent
                .lock()
                .expect("fake sink lock should not be poisoned")
                .as_slice(),
            &[("conn-1".to_owned(), "hello".to_owned())]
        );
    }

    fn serial_device_config() -> SerialDeviceConfig {
        SerialDeviceConfig {
            auto_reconnect: true,
            baud_rate: 115_200,
            data_bits: 8,
            device_id: "main-device".to_owned(),
            flow_control: "none".to_owned(),
            parity: "none".to_owned(),
            port: "COM3".to_owned(),
            product_id: None,
            read_mode: "line".to_owned(),
            stop_bits: "1".to_owned(),
            validate_usb_identity: false,
            vendor_id: None,
        }
    }

    #[test]
    fn desktop_only_actions_fail_explicitly_in_headless_handler() {
        let error = execute(
            "action.notification",
            json!({ "title": "BaudBound", "message": "hello" }),
        )
        .expect_err("notification should be desktop-only");

        assert!(error.to_string().contains("desktop runner action adapter"));
    }

    #[test]
    fn desktop_action_handler_routes_desktop_actions_to_adapter() {
        let adapter = FakeDesktopAdapter::default();
        let handler = DesktopActionHandler::new(HeadlessActionHandler::default(), adapter);

        let result = execute_with_handler(
            &handler,
            "action.notification",
            json!({ "title": "BaudBound", "message": "hello" }),
            Value::Null,
        )
        .expect("desktop action should route to adapter");

        assert_eq!(
            result.output_data.get("handled"),
            Some(&json!("notification"))
        );
        assert_eq!(
            handler
                .adapter
                .called
                .lock()
                .expect("fake adapter lock should not be poisoned")
                .as_slice(),
            &["action.notification".to_owned()]
        );
    }

    #[test]
    fn desktop_action_handler_routes_input_and_window_actions_to_adapter() {
        let adapter = FakeDesktopAdapter::default();
        let handler = DesktopActionHandler::new(HeadlessActionHandler::default(), adapter);

        for (action_type, expected) in [
            ("action.keyboard", "keyboard"),
            ("action.keyboard.type_text", "keyboard_type_text"),
            ("action.mouse", "mouse_click"),
            ("action.mouse.move", "mouse_move"),
            ("action.pixel.get", "pixel_get"),
            ("action.window.active", "active_window"),
            ("action.window.focus", "window_focus"),
        ] {
            let result = execute_with_handler(&handler, action_type, json!({}), Value::Null)
                .expect("desktop action should route to adapter");
            assert_eq!(result.output_data.get("handled"), Some(&json!(expected)));
        }

        assert_eq!(
            handler
                .adapter
                .called
                .lock()
                .expect("fake adapter lock should not be poisoned")
                .as_slice(),
            &[
                "action.keyboard".to_owned(),
                "action.keyboard.type_text".to_owned(),
                "action.mouse".to_owned(),
                "action.mouse.move".to_owned(),
                "action.pixel.get".to_owned(),
                "action.window.active".to_owned(),
                "action.window.focus".to_owned()
            ]
        );
    }

    #[test]
    fn desktop_action_handler_delegates_headless_actions() {
        let handler = DesktopActionHandler::new(
            HeadlessActionHandler::default(),
            FakeDesktopAdapter::default(),
        );

        let result = execute_with_handler(
            &handler,
            "action.text.format",
            json!({
                "operation": "uppercase",
                "input": "baudbound"
            }),
            Value::Null,
        )
        .expect("headless action should delegate");

        assert_eq!(result.output_data.get("text"), Some(&json!("BAUDBOUND")));
        assert!(
            handler
                .adapter
                .called
                .lock()
                .expect("fake adapter lock should not be poisoned")
                .is_empty()
        );
    }

    #[test]
    fn sends_http_request_and_parses_json_response() {
        let server = TestHttpServer::start(
            "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nX-Test: ok\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
        );

        let result = execute(
            "action.http",
            json!({
                "method": "POST",
                "url": server.url("/submit"),
                "headers": [{ "id": "h-1", "name": "X-Request", "value": "runner" }],
                "userAgent": "BaudBound-Test",
                "timeoutSeconds": "5",
                "body": "{\"name\":\"baudbound\"}"
            }),
        )
        .expect("HTTP request should succeed");

        assert_eq!(result.output_data.get("status_code"), Some(&json!(201)));
        assert_eq!(
            result.output_data.get("status_text"),
            Some(&json!("Created"))
        );
        assert_eq!(result.output_data.get("json"), Some(&json!({ "ok": true })));
        assert_eq!(
            result
                .output_data
                .get("headers")
                .and_then(|headers| headers.get("x-test")),
            Some(&json!("ok"))
        );
        assert!(
            result
                .output_data
                .get("duration_ms")
                .and_then(Value::as_u64)
                .is_some()
        );

        let request = server.join();
        assert!(request.contains("POST /submit HTTP/1.1"));
        assert!(request.contains("x-request: runner"));
        assert!(request.contains("user-agent: BaudBound-Test"));
        assert!(request.contains(r#"{"name":"baudbound"}"#));
    }

    #[test]
    fn downloads_file_to_destination() {
        let server = TestHttpServer::start(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 9\r\nConnection: close\r\n\r\ndownload!",
        );
        let directory = tempfile::tempdir().expect("tempdir should be created");
        let destination = directory.path().join("downloads").join("file.txt");

        let result = execute(
            "action.file.download",
            json!({
                "url": server.url("/file.txt"),
                "destinationPath": destination.display().to_string(),
                "overwrite": "false",
                "timeoutSeconds": 5
            }),
        )
        .expect("download should succeed");

        assert_eq!(
            fs::read_to_string(&destination).expect("download should read"),
            "download!"
        );
        assert_eq!(result.output_data.get("bytes"), Some(&json!(9)));
        assert_eq!(
            result.output_data.get("path"),
            Some(&json!(destination.display().to_string()))
        );

        let request = server.join();
        assert!(request.contains("GET /file.txt HTTP/1.1"));
    }

    #[test]
    fn runs_process_and_captures_output() {
        let (executable, arguments) = platform_echo_process("process-ok");
        let result = execute(
            "action.process.run",
            json!({
                "executable": executable,
                "arguments": arguments,
                "workingDirectory": ""
            }),
        )
        .expect("process should run");

        assert_eq!(result.output_data.get("exit_code"), Some(&json!(0)));
        assert_eq!(result.output_data.get("success"), Some(&json!(true)));
        assert!(
            result
                .output_data
                .get("stdout")
                .and_then(Value::as_str)
                .is_some_and(|stdout| stdout.contains("process-ok"))
        );
    }

    #[test]
    fn opens_application_without_waiting_for_output() {
        let (application, arguments) = platform_echo_process("open-ok");
        let result = execute(
            "action.application.open",
            json!({
                "application": application,
                "arguments": arguments
            }),
        )
        .expect("application should open");

        assert_eq!(
            result.output_data.get("application_id"),
            Some(&json!(application))
        );
        assert!(
            result
                .output_data
                .get("process_id")
                .and_then(Value::as_u64)
                .is_some_and(|process_id| process_id > 0)
        );
        assert!(
            result
                .output_data
                .get("arguments")
                .is_some_and(Value::is_array)
        );
    }

    #[test]
    fn rejects_open_application_with_invalid_arguments() {
        let error = execute(
            "action.application.open",
            json!({
                "application": "example-app",
                "arguments": "\"unterminated"
            }),
        )
        .expect_err("unterminated arguments should fail");

        assert!(
            error.to_string().contains("unterminated quoted string"),
            "{error}"
        );
    }

    #[test]
    fn reads_current_process_status() {
        let current_executable = std::env::current_exe().expect("current exe should resolve");
        let process_name = current_executable
            .file_name()
            .and_then(|name| name.to_str())
            .expect("current exe should have utf-8 file name");

        let result = execute(
            "action.process.status",
            json!({
                "matchMode": "process_name",
                "target": process_name
            }),
        )
        .expect("process status should succeed");

        assert_eq!(result.output_data.get("running"), Some(&json!(true)));
        assert_eq!(result.output_data.get("state"), Some(&json!("running")));
        assert!(
            result
                .output_data
                .get("process_id")
                .and_then(Value::as_u64)
                .is_some_and(|process_id| process_id > 0)
        );
    }

    #[test]
    fn kills_process_by_pid() {
        let mut child = spawn_long_running_process();
        let process_id = child.id();

        let result = execute(
            "action.process.kill",
            json!({
                "matchMode": "pid",
                "target": process_id.to_string()
            }),
        )
        .expect("process kill should succeed");

        assert_eq!(result.output_data.get("killed"), Some(&json!(true)));
        assert_eq!(
            result.output_data.get("process_id"),
            Some(&json!(process_id))
        );
        let _ = child.wait();
    }

    #[test]
    fn runs_shell_command_and_captures_output() {
        let result = execute(
            "action.shell",
            json!({ "command": platform_echo_shell_command("shell-ok") }),
        )
        .expect("shell command should run");

        assert_eq!(result.output_data.get("exit_code"), Some(&json!(0)));
        assert_eq!(result.output_data.get("success"), Some(&json!(true)));
        assert!(
            result
                .output_data
                .get("stdout")
                .and_then(Value::as_str)
                .is_some_and(|stdout| stdout.contains("shell-ok"))
        );
    }

    #[test]
    fn transforms_text_with_regex_replace() {
        let result = execute(
            "action.text.format",
            json!({
                "operation": "regex_replace",
                "input": "server-123",
                "search": "\\d+",
                "replacement": "ok"
            }),
        )
        .expect("text transform should succeed");

        assert_eq!(result.output_data.get("text"), Some(&json!("server-ok")));
    }

    #[test]
    fn joins_json_items() {
        let result = execute(
            "action.text.format",
            json!({
                "operation": "join",
                "items": ["one", "two", 3],
                "delimiter": "|"
            }),
        )
        .expect("join should succeed");

        assert_eq!(result.output_data.get("text"), Some(&json!("one|two|3")));
    }

    fn execute(
        action_type: &str,
        config: Value,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        execute_with_trigger_payload(action_type, config, Value::Null)
    }

    fn execute_with_trigger_payload(
        action_type: &str,
        config: Value,
        trigger_payload: Value,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        let handler = HeadlessActionHandler::default();
        execute_with_handler(&handler, action_type, config, trigger_payload)
    }

    fn execute_with_handler(
        handler: &dyn RuntimeActionHandler,
        action_type: &str,
        config: Value,
        trigger_payload: Value,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        let context = RuntimeContext {
            identity: RunIdentity {
                run_id: "run-1".to_owned(),
                script_id: "script-1".to_owned(),
                trigger_node_id: "trigger-1".to_owned(),
            },
            package_path: None,
            trigger_payload,
            variables: Default::default(),
        };
        let request = RuntimeActionRequest {
            action: None,
            action_type: action_type.to_owned(),
            config: match config {
                Value::Object(config) => config,
                _ => Map::new(),
            },
            node_id: "node-1".to_owned(),
        };

        handler.execute_action(&request, &context)
    }

    fn platform_echo_process(message: &str) -> (&'static str, String) {
        #[cfg(windows)]
        {
            ("cmd", format!("/C echo {message}"))
        }

        #[cfg(not(windows))]
        {
            ("printf", message.to_owned())
        }
    }

    fn platform_echo_shell_command(message: &str) -> String {
        #[cfg(windows)]
        {
            format!("echo {message}")
        }

        #[cfg(not(windows))]
        {
            format!("printf {message}")
        }
    }

    fn spawn_long_running_process() -> std::process::Child {
        #[cfg(windows)]
        {
            std::process::Command::new("cmd")
                .args(["/C", "ping 127.0.0.1 -n 30 > NUL"])
                .spawn()
                .expect("long running process should start")
        }

        #[cfg(not(windows))]
        {
            std::process::Command::new("sh")
                .args(["-c", "sleep 30"])
                .spawn()
                .expect("long running process should start")
        }
    }

    struct TestHttpServer {
        join_handle: thread::JoinHandle<String>,
        url: String,
    }

    impl TestHttpServer {
        fn start(response: &'static str) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
            let address = listener
                .local_addr()
                .expect("test server address should resolve");
            let join_handle = thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("test server should accept");
                let mut buffer = [0_u8; 4096];
                let bytes_read = stream
                    .read(&mut buffer)
                    .expect("test server should read request");
                stream
                    .write_all(response.as_bytes())
                    .expect("test server should write response");
                String::from_utf8_lossy(&buffer[..bytes_read]).to_string()
            });

            Self {
                join_handle,
                url: format!("http://{address}"),
            }
        }

        fn url(&self, path: &str) -> String {
            format!("{}{}", self.url, path)
        }

        fn join(self) -> String {
            self.join_handle
                .join()
                .expect("test server thread should finish")
        }
    }
}
