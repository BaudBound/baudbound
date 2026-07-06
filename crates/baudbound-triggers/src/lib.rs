//! Trigger adapter contracts for runner implementations.

use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use baudbound_runtime::RunReport;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use serialport::{
    DataBits, FlowControl, Parity, SerialPortBuilder, SerialPortType, StopBits, available_ports,
};
use thiserror::Error;

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct TriggerRegistration {
    pub action_type: String,
    pub config: Value,
    pub node_id: String,
    pub runner_type: String,
    pub script_id: String,
    pub script_name: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct TriggerEvent {
    pub node_id: String,
    pub payload: Value,
    pub script_id: String,
}

#[derive(Debug, Error)]
pub enum TriggerError {
    #[error("trigger {0} is not supported by this runner")]
    Unsupported(String),
    #[error("trigger {0} failed: {1}")]
    Failed(String, String),
}

pub trait TriggerHandler: Send + Sync {
    fn register(&self, registration: &TriggerRegistration) -> Result<(), TriggerError>;
}

pub trait TriggerDispatcher: Send + Sync {
    fn dispatch(&self, event: TriggerEvent) -> Result<RunReport, TriggerError>;
}

pub struct FileWatchService {
    watchers: Vec<RecommendedWatcher>,
}

impl FileWatchService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            watchers: Vec::new(),
        }
    }

    pub fn start(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        sender: Sender<TriggerEvent>,
    ) -> Result<Self, TriggerError> {
        let mut watchers = Vec::new();

        for registration in registrations {
            if registration.action_type != "trigger.file_watch" {
                continue;
            }

            let spec = FileWatchSpec::from_registration(&registration)?;
            let callback_registration = registration.clone();
            let callback_watched_path = spec.path.clone();
            let callback_sender = sender.clone();
            let mut watcher = RecommendedWatcher::new(
                move |result: notify::Result<Event>| match result {
                    Ok(event) => {
                        for event in file_watch_events_from_notify_event(
                            &callback_registration,
                            &callback_watched_path,
                            &event,
                        ) {
                            let _ = callback_sender.send(event);
                        }
                    }
                    Err(error) => {
                        tracing::warn!(
                            "file watch trigger {} failed to read event: {}",
                            callback_registration.node_id,
                            error
                        );
                    }
                },
                Config::default(),
            )
            .map_err(|source| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    format!("failed to create file watcher: {source}"),
                )
            })?;
            watcher
                .watch(&spec.path, RecursiveMode::NonRecursive)
                .map_err(|source| {
                    TriggerError::Failed(
                        registration.node_id.clone(),
                        format!("failed to watch {}: {source}", spec.path.display()),
                    )
                })?;
            watchers.push(watcher);
        }

        Ok(Self { watchers })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.watchers.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.watchers.len()
    }
}

pub struct SerialInputService {
    handles: Vec<JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

impl SerialInputService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            handles: Vec::new(),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        devices: impl IntoIterator<Item = SerialDeviceConfig>,
        sender: Sender<TriggerEvent>,
    ) -> Result<Self, TriggerError> {
        let mut handles = Vec::new();
        let running = Arc::new(AtomicBool::new(true));
        let devices = devices
            .into_iter()
            .map(|device| (device.device_id.clone(), device))
            .collect::<BTreeMap<_, _>>();

        for registration in registrations {
            if registration.action_type != "trigger.serial_input" {
                continue;
            }

            let spec = SerialInputSpec::from_registration(&registration, &devices)?;
            let thread_registration = registration.clone();
            let thread_sender = sender.clone();
            let thread_running = Arc::clone(&running);
            handles.push(thread::spawn(move || {
                run_serial_input_reader(thread_registration, spec, thread_sender, thread_running);
            }));
        }

        Ok(Self { handles, running })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.handles.len()
    }
}

impl Drop for SerialInputService {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }
}

#[derive(Debug, Clone)]
struct FileWatchSpec {
    path: PathBuf,
}

impl FileWatchSpec {
    fn from_registration(registration: &TriggerRegistration) -> Result<Self, TriggerError> {
        let path = registration
            .config
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    "file watch trigger must define path".to_owned(),
                )
            })?;
        if path.contains("{{") || path.contains("}}") {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "file watch path cannot use runtime variable templates".to_owned(),
            ));
        }

        Ok(Self {
            path: PathBuf::from(path),
        })
    }
}

fn file_watch_events_from_notify_event(
    registration: &TriggerRegistration,
    watched_path: &Path,
    event: &Event,
) -> Vec<TriggerEvent> {
    let event_name = file_event_name(&event.kind);
    let paths = if event.paths.is_empty() {
        vec![watched_path.to_path_buf()]
    } else {
        event.paths.clone()
    };

    paths
        .into_iter()
        .map(|path| file_watch_event(registration, watched_path, &path, event_name))
        .collect()
}

fn file_watch_event(
    registration: &TriggerRegistration,
    watched_path: &Path,
    event_path: &Path,
    event_name: &str,
) -> TriggerEvent {
    TriggerEvent {
        node_id: registration.node_id.clone(),
        payload: json!({
            "event": event_name,
            "path": event_path.to_string_lossy(),
            "watched_path": watched_path.to_string_lossy(),
        }),
        script_id: registration.script_id.clone(),
    }
}

fn file_event_name(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::Access(_) => "accessed",
        EventKind::Create(_) => "created",
        EventKind::Modify(_) => "modified",
        EventKind::Remove(_) => "removed",
        EventKind::Any | EventKind::Other => "changed",
    }
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
struct SerialInputSpec {
    auto_reconnect: bool,
    baud_rate: u32,
    data_bits: DataBits,
    device_id: String,
    flow_control: FlowControl,
    parity: Parity,
    port: String,
    product_id: Option<u16>,
    read_mode: SerialReadMode,
    stop_bits: StopBits,
    validate_usb_identity: bool,
    vendor_id: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SerialReadMode {
    Line,
    Raw,
}

impl SerialInputSpec {
    fn from_registration(
        registration: &TriggerRegistration,
        devices: &BTreeMap<String, SerialDeviceConfig>,
    ) -> Result<Self, TriggerError> {
        let device_id = required_config_string(registration, "deviceId")?;
        let device = devices.get(&device_id).ok_or_else(|| {
            TriggerError::Failed(
                registration.node_id.clone(),
                format!(
                    "serial input trigger references device id {device_id:?}, but runner config has no matching [serial.devices.{device_id}] entry"
                ),
            )
        })?;
        let validate_usb_identity = device.validate_usb_identity;
        let vendor_id = device
            .vendor_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| parse_usb_hex_id(registration, "vendor_id", value))
            .transpose()?;
        let product_id = device
            .product_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| parse_usb_hex_id(registration, "product_id", value))
            .transpose()?;

        if validate_usb_identity && vendor_id.is_none() {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "serial device config must define vendor_id when USB identity validation is enabled"
                    .to_owned(),
            ));
        }
        if validate_usb_identity && product_id.is_none() {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "serial device config must define product_id when USB identity validation is enabled"
                    .to_owned(),
            ));
        }

        Ok(Self {
            auto_reconnect: device.auto_reconnect,
            baud_rate: device.baud_rate,
            data_bits: parse_data_bits(&device.data_bits.to_string()),
            device_id,
            flow_control: parse_flow_control(&device.flow_control),
            parity: parse_parity(&device.parity),
            port: device.port.clone(),
            product_id,
            read_mode: parse_read_mode(&device.read_mode),
            stop_bits: parse_stop_bits(&device.stop_bits),
            validate_usb_identity,
            vendor_id,
        })
    }
}

fn run_serial_input_reader(
    registration: TriggerRegistration,
    spec: SerialInputSpec,
    sender: Sender<TriggerEvent>,
    running: Arc<AtomicBool>,
) {
    while running.load(Ordering::Relaxed) {
        if let Err(error) = validate_serial_usb_identity(&spec) {
            tracing::warn!(
                "serial input trigger {} USB identity validation failed: {}",
                registration.node_id,
                error
            );
            if !spec.auto_reconnect {
                return;
            }
            sleep_serial_reconnect_delay(&running);
            continue;
        }

        let mut port = match serial_port_builder(&spec, Duration::from_millis(250)).open() {
            Ok(port) => port,
            Err(error) => {
                tracing::warn!(
                    "serial input trigger {} failed to open {}: {}",
                    registration.node_id,
                    spec.port,
                    error
                );
                if !spec.auto_reconnect {
                    return;
                }
                sleep_serial_reconnect_delay(&running);
                continue;
            }
        };

        let result = match spec.read_mode {
            SerialReadMode::Line => {
                read_serial_lines(&registration, &spec, &sender, &running, &mut *port)
            }
            SerialReadMode::Raw => {
                read_serial_raw_chunks(&registration, &spec, &sender, &running, &mut *port)
            }
        };

        if let Err(error) = result {
            tracing::warn!(
                "serial input trigger {} read loop ended for {}: {}",
                registration.node_id,
                spec.port,
                error
            );
            if !spec.auto_reconnect {
                return;
            }
            sleep_serial_reconnect_delay(&running);
        }
    }
}

fn read_serial_lines(
    registration: &TriggerRegistration,
    spec: &SerialInputSpec,
    sender: &Sender<TriggerEvent>,
    running: &AtomicBool,
    port: &mut dyn serialport::SerialPort,
) -> io::Result<()> {
    let mut line = Vec::new();
    let mut byte = [0_u8; 1];
    while running.load(Ordering::Relaxed) {
        match port.read(&mut byte) {
            Ok(0) => {}
            Ok(_) if byte[0] == b'\n' => {
                while matches!(line.last(), Some(b'\r' | b'\n')) {
                    line.pop();
                }
                send_serial_event(registration, spec, sender, &line);
                line.clear();
            }
            Ok(_) => line.push(byte[0]),
            Err(error) if error.kind() == io::ErrorKind::TimedOut => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn read_serial_raw_chunks(
    registration: &TriggerRegistration,
    spec: &SerialInputSpec,
    sender: &Sender<TriggerEvent>,
    running: &AtomicBool,
    port: &mut dyn serialport::SerialPort,
) -> io::Result<()> {
    let mut buffer = [0_u8; 1024];
    while running.load(Ordering::Relaxed) {
        match port.read(&mut buffer) {
            Ok(0) => {}
            Ok(bytes_read) => send_serial_event(registration, spec, sender, &buffer[..bytes_read]),
            Err(error) if error.kind() == io::ErrorKind::TimedOut => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn send_serial_event(
    registration: &TriggerRegistration,
    spec: &SerialInputSpec,
    sender: &Sender<TriggerEvent>,
    bytes: &[u8],
) {
    let data = String::from_utf8_lossy(bytes).into_owned();
    let _ = sender.send(TriggerEvent {
        node_id: registration.node_id.clone(),
        payload: json!({
            "bytes": bytes.len(),
            "data": data,
            "device_id": spec.device_id,
            "timestamp": unix_timestamp_millis(SystemTime::now()).to_string(),
        }),
        script_id: registration.script_id.clone(),
    });
}

fn serial_port_builder(spec: &SerialInputSpec, timeout: Duration) -> SerialPortBuilder {
    serialport::new(&spec.port, spec.baud_rate)
        .data_bits(spec.data_bits)
        .flow_control(spec.flow_control)
        .parity(spec.parity)
        .stop_bits(spec.stop_bits)
        .timeout(timeout)
}

fn validate_serial_usb_identity(spec: &SerialInputSpec) -> Result<(), String> {
    if !spec.validate_usb_identity {
        return Ok(());
    }

    let ports = available_ports().map_err(|source| source.to_string())?;
    let port = ports
        .into_iter()
        .find(|port| port.port_name == spec.port)
        .ok_or_else(|| format!("serial port {} was not found", spec.port))?;
    let SerialPortType::UsbPort(info) = port.port_type else {
        return Err(format!(
            "serial port {} is not a USB serial device",
            spec.port
        ));
    };

    if let Some(vendor_id) = spec.vendor_id
        && info.vid != vendor_id
    {
        return Err(format!(
            "vendor id mismatch: expected {:04X}, got {:04X}",
            vendor_id, info.vid
        ));
    }
    if let Some(product_id) = spec.product_id
        && info.pid != product_id
    {
        return Err(format!(
            "product id mismatch: expected {:04X}, got {:04X}",
            product_id, info.pid
        ));
    }
    Ok(())
}

fn sleep_serial_reconnect_delay(running: &AtomicBool) {
    for _ in 0..10 {
        if !running.load(Ordering::Relaxed) {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[derive(Debug, Clone)]
pub struct WebhookDispatch {
    pub event: TriggerEvent,
    pub fallback_response: WebhookResponse,
    pub wait_for_response: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookRequest {
    pub body: String,
    pub headers: BTreeMap<String, String>,
    pub method: String,
    pub path_and_query: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookResponse {
    pub body: String,
    pub content_type: String,
    pub headers: BTreeMap<String, String>,
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct WebhookService {
    routes: Vec<WebhookRoute>,
}

impl WebhookService {
    pub fn from_registrations(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
    ) -> Result<Self, TriggerError> {
        let mut routes = Vec::new();
        for registration in registrations {
            if registration.action_type != "trigger.webhook" {
                continue;
            }
            routes.push(WebhookRoute::from_registration(registration)?);
        }

        Ok(Self { routes })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    #[must_use]
    pub fn dispatch_for_request(&self, request: &WebhookRequest) -> Option<WebhookDispatch> {
        let method = request.method.to_ascii_uppercase();
        let (path, query) = split_path_and_query(&request.path_and_query);

        self.routes
            .iter()
            .find(|route| route.method == method && route.path == path)
            .map(|route| WebhookDispatch {
                event: TriggerEvent {
                    node_id: route.registration.node_id.clone(),
                    payload: webhook_payload(route, request, &path, &query),
                    script_id: route.registration.script_id.clone(),
                },
                fallback_response: route.fallback_response.clone(),
                wait_for_response: route.wait_for_response,
            })
    }

    #[must_use]
    pub fn response_for_report(
        &self,
        dispatch: &WebhookDispatch,
        report: &RunReport,
    ) -> WebhookResponse {
        if !dispatch.wait_for_response {
            return dispatch.fallback_response.clone();
        }

        response_from_report(&dispatch.event.node_id, report)
            .unwrap_or_else(|| dispatch.fallback_response.clone())
    }
}

#[derive(Debug, Clone)]
struct WebhookRoute {
    fallback_response: WebhookResponse,
    method: String,
    path: String,
    registration: TriggerRegistration,
    wait_for_response: bool,
}

impl WebhookRoute {
    fn from_registration(registration: TriggerRegistration) -> Result<Self, TriggerError> {
        let hook_name = registration
            .config
            .get("hookName")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    "webhook trigger must define hookName".to_owned(),
                )
            })?;
        let method = registration
            .config
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("POST")
            .trim()
            .to_ascii_uppercase();
        if !is_supported_http_method(&method) {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                format!("unsupported webhook method {method:?}"),
            ));
        }

        Ok(Self {
            fallback_response: WebhookResponse {
                body: config_string(&registration.config, "timeoutResponseBody")
                    .unwrap_or_else(|| r#"{ "ok": true }"#.to_owned()),
                content_type: config_string(&registration.config, "timeoutResponseContentType")
                    .unwrap_or_else(|| "application/json".to_owned()),
                headers: BTreeMap::new(),
                status_code: config_u16(&registration.config, "timeoutResponseStatus", 200),
            },
            method,
            path: format!("/events/{hook_name}"),
            wait_for_response: config_bool(&registration.config, "waitForResponse"),
            registration,
        })
    }
}

fn webhook_payload(
    route: &WebhookRoute,
    request: &WebhookRequest,
    path: &str,
    query: &BTreeMap<String, String>,
) -> Value {
    let json_body = serde_json::from_str::<Value>(&request.body).unwrap_or_else(|_| json!({}));
    json!({
        "body": request.body,
        "headers": request.headers,
        "json": json_body,
        "method": request.method.to_ascii_uppercase(),
        "path": path,
        "query": query,
        "response": {
            "waiting": route.wait_for_response,
        },
        "trigger_id": route.registration.node_id,
    })
}

fn response_from_report(trigger_node_id: &str, report: &RunReport) -> Option<WebhookResponse> {
    let mut response_prefixes = report
        .variables
        .iter()
        .filter_map(|(key, value)| {
            let prefix = key.strip_suffix(".trigger_id")?;
            (value.as_str() == Some(trigger_node_id)).then_some(prefix.to_owned())
        })
        .collect::<Vec<_>>();
    response_prefixes.sort();

    for prefix in response_prefixes {
        let sent = report
            .variables
            .get(&format!("{prefix}.sent"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !sent {
            continue;
        }

        return Some(WebhookResponse {
            body: report
                .variables
                .get(&format!("{prefix}.body"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            content_type: report
                .variables
                .get(&format!("{prefix}.content_type"))
                .and_then(Value::as_str)
                .unwrap_or("text/plain")
                .to_owned(),
            headers: value_object_to_string_map(report.variables.get(&format!("{prefix}.headers"))),
            status_code: report
                .variables
                .get(&format!("{prefix}.status_code"))
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .filter(|value| (100..=599).contains(value))
                .unwrap_or(200),
        });
    }

    None
}

fn split_path_and_query(path_and_query: &str) -> (String, BTreeMap<String, String>) {
    let (path, query) = path_and_query
        .split_once('?')
        .unwrap_or((path_and_query, ""));
    (path.to_owned(), parse_query(query))
}

fn parse_query(query: &str) -> BTreeMap<String, String> {
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            (decode_url_component(key), decode_url_component(value))
        })
        .collect()
}

fn decode_url_component(value: &str) -> String {
    let plus_normalized = value.replace('+', " ");
    urlencoding::decode(&plus_normalized)
        .map(|value| value.into_owned())
        .unwrap_or(plus_normalized)
}

fn value_object_to_string_map(value: Option<&Value>) -> BTreeMap<String, String> {
    value
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|fields| fields.iter())
        .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_owned())))
        .collect()
}

fn config_string(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn required_config_string(
    registration: &TriggerRegistration,
    key: &str,
) -> Result<String, TriggerError> {
    config_string(&registration.config, key)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            TriggerError::Failed(
                registration.node_id.clone(),
                format!("trigger must define {key}"),
            )
        })
}

fn config_u16(config: &Value, key: &str, fallback: u16) -> u16 {
    match config.get(key) {
        Some(Value::Number(value)) => value
            .as_u64()
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| (100..=599).contains(value))
            .unwrap_or(fallback),
        Some(Value::String(value)) => value
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|value| (100..=599).contains(value))
            .unwrap_or(fallback),
        _ => fallback,
    }
}

fn config_bool(config: &Value, key: &str) -> bool {
    match config.get(key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => value.eq_ignore_ascii_case("true"),
        _ => false,
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

fn parse_read_mode(value: &str) -> SerialReadMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "raw" => SerialReadMode::Raw,
        _ => SerialReadMode::Line,
    }
}

fn parse_usb_hex_id(
    registration: &TriggerRegistration,
    key: &str,
    value: &str,
) -> Result<u16, TriggerError> {
    let trimmed = value
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if trimmed.is_empty()
        || trimmed.len() > 4
        || !trimmed.chars().all(|value| value.is_ascii_hexdigit())
    {
        return Err(TriggerError::Failed(
            registration.node_id.clone(),
            format!("{key} must be a 1-4 digit hexadecimal value"),
        ));
    }
    u16::from_str_radix(trimmed, 16).map_err(|source| {
        TriggerError::Failed(
            registration.node_id.clone(),
            format!("{key} is not a valid hexadecimal USB id: {source}"),
        )
    })
}

fn is_supported_http_method(method: &str) -> bool {
    matches!(
        method,
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    )
}

#[derive(Debug, Clone)]
pub struct ScheduleService {
    schedules: Vec<ScheduleTask>,
}

impl ScheduleService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schedules: Vec::new(),
        }
    }

    pub fn from_registrations(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        start: Instant,
    ) -> Result<Self, TriggerError> {
        let mut schedules = Vec::new();

        for registration in registrations {
            if registration.action_type != "trigger.schedule" {
                continue;
            }

            let spec = ScheduleSpec::from_registration(&registration)?;
            schedules.push(ScheduleTask {
                interval: spec.interval,
                next_due: start + spec.interval,
                registration,
                spec,
            });
        }

        Ok(Self { schedules })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.schedules.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.schedules.len()
    }

    pub fn mark_all_due_now(&mut self, now: Instant) {
        for schedule in &mut self.schedules {
            schedule.next_due = now;
        }
    }

    #[must_use]
    pub fn time_until_next_due(&self, now: Instant) -> Option<Duration> {
        self.schedules
            .iter()
            .map(|schedule| schedule.next_due.saturating_duration_since(now))
            .min()
    }

    pub fn due_events(&mut self, now: Instant, timestamp: SystemTime) -> Vec<TriggerEvent> {
        let timestamp_unix = unix_timestamp(timestamp);
        let mut events = Vec::new();

        for schedule in &mut self.schedules {
            if now < schedule.next_due {
                continue;
            }

            events.push(TriggerEvent {
                node_id: schedule.registration.node_id.clone(),
                payload: json!({
                    "scheduled_at_unix": timestamp_unix,
                    "interval_seconds": schedule.interval.as_secs(),
                    "schedule": {
                        "every": schedule.spec.every,
                        "unit": schedule.spec.unit,
                    },
                }),
                script_id: schedule.registration.script_id.clone(),
            });

            while schedule.next_due <= now {
                schedule.next_due += schedule.interval;
            }
        }

        events
    }
}

#[derive(Debug, Clone)]
struct ScheduleTask {
    interval: Duration,
    next_due: Instant,
    registration: TriggerRegistration,
    spec: ScheduleSpec,
}

#[derive(Debug, Clone)]
struct ScheduleSpec {
    every: u64,
    interval: Duration,
    unit: String,
}

impl ScheduleSpec {
    fn from_registration(registration: &TriggerRegistration) -> Result<Self, TriggerError> {
        let every = schedule_every(&registration.config).ok_or_else(|| {
            TriggerError::Failed(
                registration.node_id.clone(),
                "schedule trigger must define a positive every value".to_owned(),
            )
        })?;
        let unit = registration
            .config
            .get("unit")
            .and_then(Value::as_str)
            .unwrap_or("minutes");
        let unit_seconds = schedule_unit_seconds(unit).ok_or_else(|| {
            TriggerError::Failed(
                registration.node_id.clone(),
                format!("unsupported schedule unit {unit:?}"),
            )
        })?;

        Ok(Self {
            every,
            interval: Duration::from_secs(every.saturating_mul(unit_seconds)),
            unit: normalized_schedule_unit(unit).to_owned(),
        })
    }
}

fn schedule_every(config: &Value) -> Option<u64> {
    match config.get("every")? {
        Value::Number(value) => value.as_u64().filter(|value| *value > 0),
        Value::String(value) => value.trim().parse::<u64>().ok().filter(|value| *value > 0),
        _ => None,
    }
}

fn schedule_unit_seconds(unit: &str) -> Option<u64> {
    match normalized_schedule_unit(unit) {
        "seconds" => Some(1),
        "minutes" => Some(60),
        "hours" => Some(60 * 60),
        "days" => Some(24 * 60 * 60),
        _ => None,
    }
}

fn normalized_schedule_unit(unit: &str) -> &str {
    match unit.trim().to_ascii_lowercase().as_str() {
        "s" | "sec" | "second" | "seconds" => "seconds",
        "m" | "min" | "minute" | "minutes" => "minutes",
        "h" | "hr" | "hour" | "hours" => "hours",
        "d" | "day" | "days" => "days",
        _ => "",
    }
}

fn unix_timestamp(timestamp: SystemTime) -> u64 {
    timestamp
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn unix_timestamp_millis(timestamp: SystemTime) -> u128 {
    timestamp
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn creates_due_schedule_events_and_advances_next_due() {
        let start = Instant::now();
        let registration = TriggerRegistration {
            action_type: "trigger.schedule".to_owned(),
            config: json!({"every": "2", "unit": "seconds"}),
            node_id: "n-schedule".to_owned(),
            runner_type: "schedule".to_owned(),
            script_id: "script-1".to_owned(),
            script_name: "Script One".to_owned(),
        };
        let mut service = ScheduleService::from_registrations([registration], start)
            .expect("schedule should parse");

        assert!(service.due_events(start, UNIX_EPOCH).is_empty());

        let events = service.due_events(start + Duration::from_secs(2), UNIX_EPOCH);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].script_id, "script-1");
        assert_eq!(events[0].node_id, "n-schedule");
        assert_eq!(events[0].payload["interval_seconds"], 2);
        assert_eq!(events[0].payload["schedule"]["unit"], "seconds");
        assert_eq!(
            service.time_until_next_due(start + Duration::from_secs(2)),
            Some(Duration::from_secs(2))
        );
    }

    #[test]
    fn rejects_invalid_schedule_interval() {
        let registration = TriggerRegistration {
            action_type: "trigger.schedule".to_owned(),
            config: json!({"every": "0", "unit": "minutes"}),
            node_id: "n-schedule".to_owned(),
            runner_type: "schedule".to_owned(),
            script_id: "script-1".to_owned(),
            script_name: "Script One".to_owned(),
        };

        let error = ScheduleService::from_registrations([registration], Instant::now())
            .expect_err("zero interval should fail");

        assert!(error.to_string().contains("positive every"));
    }

    #[test]
    fn ignores_non_schedule_registrations() {
        let registration = TriggerRegistration {
            action_type: "trigger.manual".to_owned(),
            config: json!({}),
            node_id: "n-manual".to_owned(),
            runner_type: "manual".to_owned(),
            script_id: "script-1".to_owned(),
            script_name: "Script One".to_owned(),
        };

        let service = ScheduleService::from_registrations([registration], Instant::now())
            .expect("manual trigger should be ignored");

        assert!(service.is_empty());
    }

    #[test]
    fn validates_file_watch_path_without_runtime_templates() {
        let registration = file_watch_registration(json!({"path": "{{dynamic_path}}"}));

        let error = FileWatchSpec::from_registration(&registration)
            .expect_err("runtime templates should be rejected");

        assert!(error.to_string().contains("runtime variable templates"));
    }

    #[test]
    fn builds_file_watch_trigger_event_payload() {
        let registration = file_watch_registration(json!({"path": "C:/tmp/input.txt"}));
        let event = file_watch_event(
            &registration,
            Path::new("C:/tmp/input.txt"),
            Path::new("C:/tmp/input.txt"),
            "modified",
        );

        assert_eq!(event.script_id, "script-1");
        assert_eq!(event.node_id, "n-file");
        assert_eq!(event.payload["event"], "modified");
        assert_eq!(event.payload["path"], "C:/tmp/input.txt");
        assert_eq!(event.payload["watched_path"], "C:/tmp/input.txt");
    }

    #[test]
    fn parses_serial_input_trigger_config() {
        let registration = serial_input_registration(json!({
            "deviceId": "main-device"
        }));
        let devices = BTreeMap::from([("main-device".to_owned(), serial_device_config())]);

        let spec = SerialInputSpec::from_registration(&registration, &devices)
            .expect("serial config should parse");

        assert_eq!(spec.device_id, "main-device");
        assert_eq!(spec.port, "COM3");
        assert_eq!(spec.baud_rate, 115_200);
        assert_eq!(spec.read_mode, SerialReadMode::Line);
        assert!(spec.auto_reconnect);
        assert!(spec.validate_usb_identity);
        assert_eq!(spec.vendor_id, Some(0x1A86));
        assert_eq!(spec.product_id, Some(0x7523));
    }

    #[test]
    fn rejects_serial_input_when_runner_device_is_missing() {
        let registration = serial_input_registration(json!({
            "deviceId": "missing-device"
        }));
        let devices = BTreeMap::new();

        let error = SerialInputSpec::from_registration(&registration, &devices)
            .expect_err("missing runner serial device should fail");

        assert!(
            error
                .to_string()
                .contains("[serial.devices.missing-device]")
        );
    }

    #[test]
    fn builds_serial_input_trigger_event_payload() {
        let registration = serial_input_registration(json!({
            "deviceId": "main-device"
        }));
        let devices = BTreeMap::from([("main-device".to_owned(), serial_device_config())]);
        let spec = SerialInputSpec::from_registration(&registration, &devices)
            .expect("serial config should parse");
        let (sender, receiver) = std::sync::mpsc::channel();

        send_serial_event(&registration, &spec, &sender, b"hello");

        let event = receiver.recv().expect("serial event should be sent");
        assert_eq!(event.script_id, "script-1");
        assert_eq!(event.node_id, "n-serial");
        assert_eq!(event.payload["device_id"], "main-device");
        assert_eq!(event.payload["data"], "hello");
        assert_eq!(event.payload["bytes"], 5);
        assert!(event.payload["timestamp"].as_str().is_some());
    }

    #[test]
    fn matches_webhook_request_and_builds_payload() {
        let service = WebhookService::from_registrations([webhook_registration(json!({
            "method": "POST",
            "hookName": "deploy",
            "waitForResponse": false
        }))])
        .expect("webhook should register");
        let dispatch = service
            .dispatch_for_request(&WebhookRequest {
                body: r#"{"status":"ok"}"#.to_owned(),
                headers: BTreeMap::from([(
                    "content-type".to_owned(),
                    "application/json".to_owned(),
                )]),
                method: "post".to_owned(),
                path_and_query: "/events/deploy?source=test".to_owned(),
            })
            .expect("request should match webhook");

        assert_eq!(dispatch.event.script_id, "script-1");
        assert_eq!(dispatch.event.node_id, "n-webhook");
        assert_eq!(dispatch.event.payload["method"], "POST");
        assert_eq!(dispatch.event.payload["path"], "/events/deploy");
        assert_eq!(dispatch.event.payload["query"]["source"], "test");
        assert_eq!(dispatch.event.payload["json"]["status"], "ok");
        assert_eq!(dispatch.fallback_response.status_code, 200);
    }

    #[test]
    fn extracts_waiting_webhook_response_from_run_report() {
        let service = WebhookService::from_registrations([webhook_registration(json!({
            "method": "POST",
            "hookName": "deploy",
            "waitForResponse": true,
            "timeoutResponseStatus": "202",
            "timeoutResponseBody": "fallback"
        }))])
        .expect("webhook should register");
        let dispatch = service
            .dispatch_for_request(&WebhookRequest {
                body: String::new(),
                headers: BTreeMap::new(),
                method: "POST".to_owned(),
                path_and_query: "/events/deploy".to_owned(),
            })
            .expect("request should match webhook");
        let report = RunReport {
            identity: baudbound_runtime::RunIdentity {
                run_id: "run-1".to_owned(),
                script_id: "script-1".to_owned(),
                trigger_node_id: "n-webhook".to_owned(),
            },
            logs: Vec::new(),
            variables: BTreeMap::from([
                ("n-response.sent".to_owned(), Value::Bool(true)),
                (
                    "n-response.status_code".to_owned(),
                    Value::Number(serde_json::Number::from(201)),
                ),
                (
                    "n-response.content_type".to_owned(),
                    Value::String("application/json".to_owned()),
                ),
                (
                    "n-response.body".to_owned(),
                    Value::String(r#"{"ok":true}"#.to_owned()),
                ),
                (
                    "n-response.trigger_id".to_owned(),
                    Value::String("n-webhook".to_owned()),
                ),
            ]),
        };

        let response = service.response_for_report(&dispatch, &report);

        assert_eq!(response.status_code, 201);
        assert_eq!(response.content_type, "application/json");
        assert_eq!(response.body, r#"{"ok":true}"#);
    }

    #[test]
    fn waiting_webhook_uses_fallback_when_no_response_node_sent() {
        let service = WebhookService::from_registrations([webhook_registration(json!({
            "method": "POST",
            "hookName": "deploy",
            "waitForResponse": true,
            "timeoutResponseStatus": "202",
            "timeoutResponseBody": "fallback"
        }))])
        .expect("webhook should register");
        let dispatch = service
            .dispatch_for_request(&WebhookRequest {
                body: String::new(),
                headers: BTreeMap::new(),
                method: "POST".to_owned(),
                path_and_query: "/events/deploy".to_owned(),
            })
            .expect("request should match webhook");
        let report = RunReport {
            identity: baudbound_runtime::RunIdentity {
                run_id: "run-1".to_owned(),
                script_id: "script-1".to_owned(),
                trigger_node_id: "n-webhook".to_owned(),
            },
            logs: Vec::new(),
            variables: BTreeMap::new(),
        };

        let response = service.response_for_report(&dispatch, &report);

        assert_eq!(response.status_code, 202);
        assert_eq!(response.body, "fallback");
    }

    fn webhook_registration(config: Value) -> TriggerRegistration {
        TriggerRegistration {
            action_type: "trigger.webhook".to_owned(),
            config,
            node_id: "n-webhook".to_owned(),
            runner_type: "webhook".to_owned(),
            script_id: "script-1".to_owned(),
            script_name: "Script One".to_owned(),
        }
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
            product_id: Some("7523".to_owned()),
            read_mode: "line".to_owned(),
            stop_bits: "1".to_owned(),
            validate_usb_identity: true,
            vendor_id: Some("0x1A86".to_owned()),
        }
    }

    fn file_watch_registration(config: Value) -> TriggerRegistration {
        TriggerRegistration {
            action_type: "trigger.file_watch".to_owned(),
            config,
            node_id: "n-file".to_owned(),
            runner_type: "file_watch".to_owned(),
            script_id: "script-1".to_owned(),
            script_name: "Script One".to_owned(),
        }
    }

    fn serial_input_registration(config: Value) -> TriggerRegistration {
        TriggerRegistration {
            action_type: "trigger.serial_input".to_owned(),
            config,
            node_id: "n-serial".to_owned(),
            runner_type: "serial_input".to_owned(),
            script_id: "script-1".to_owned(),
            script_name: "Script One".to_owned(),
        }
    }
}
