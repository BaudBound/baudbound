//! Trigger adapter contracts for runner implementations.

use std::{
    collections::BTreeMap,
    io,
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use baudbound_actions::WebSocketMessageSink;
use baudbound_runtime::RunReport;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use serialport::{
    DataBits, FlowControl, Parity, SerialPortBuilder, SerialPortType, StopBits, available_ports,
};
use sysinfo::{ProcessesToUpdate, System};
use thiserror::Error;
use tungstenite::{
    Error as WebSocketError, Message, WebSocket, accept_hdr,
    handshake::server::{
        Request as WebSocketHandshakeRequest, Response as WebSocketHandshakeResponse,
    },
};

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

pub trait SerialPortRebindSink: Send + Sync {
    fn update_serial_device_port(&self, device_id: &str, port: &str) -> Result<(), String>;
}

#[derive(Debug, Clone, Serialize)]
pub struct TriggerServiceDiagnostics {
    pub running: bool,
    pub state: &'static str,
    pub summary: String,
}

impl TriggerServiceDiagnostics {
    fn active(registrations: usize, label: &str) -> Self {
        Self {
            running: registrations > 0,
            state: if registrations > 0 { "active" } else { "idle" },
            summary: format!("{registrations} {label} registered"),
        }
    }

    fn thread_backed(running: bool, registrations: usize, label: &str) -> Self {
        let active = running && registrations > 0;
        Self {
            running: active,
            state: if active {
                "active"
            } else if registrations > 0 {
                "stopped"
            } else {
                "idle"
            },
            summary: format!("{registrations} {label} registered"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SerialReaderStatus {
    pub auto_reconnect: bool,
    pub auto_rebind_port: bool,
    pub device_id: String,
    pub last_error: Option<String>,
    pub last_error_unix: Option<u64>,
    pub last_event_unix: Option<u64>,
    pub node_id: String,
    pub port: String,
    pub script_id: String,
    pub state: &'static str,
}

type SerialReaderStatusMap = Arc<Mutex<BTreeMap<String, SerialReaderStatus>>>;

#[derive(Debug, Default)]
pub struct WebSocketConnectionRegistry {
    connections: Mutex<BTreeMap<String, Arc<Mutex<WebSocket<TcpStream>>>>>,
}

impl WebSocketConnectionRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn insert(&self, connection_id: String, websocket: WebSocket<TcpStream>) {
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        connections.insert(connection_id, Arc::new(Mutex::new(websocket)));
    }

    fn remove(&self, connection_id: &str) {
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        connections.remove(connection_id);
    }
}

impl WebSocketMessageSink for WebSocketConnectionRegistry {
    fn send_text(&self, connection_id: &str, message: &str) -> Result<usize, String> {
        let connection = {
            let connections = self
                .connections
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            connections.get(connection_id).cloned()
        }
        .ok_or_else(|| format!("unknown WebSocket connection id {connection_id:?}"))?;

        let bytes = message.len();
        connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .send(Message::Text(message.to_owned().into()))
            .map_err(|source| format!("failed to write WebSocket message: {source}"))?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone)]
pub struct StartupService {
    events: Vec<TriggerEvent>,
}

impl StartupService {
    #[must_use]
    pub fn empty() -> Self {
        Self { events: Vec::new() }
    }

    pub fn from_registrations(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        startup_time: SystemTime,
    ) -> Result<Self, TriggerError> {
        let events = registrations
            .into_iter()
            .filter(|registration| registration.action_type == "trigger.startup")
            .map(|registration| TriggerEvent {
                node_id: registration.node_id,
                payload: json!({
                    "reason": "runner_startup",
                    "timestamp": unix_timestamp_millis(startup_time).to_string(),
                }),
                script_id: registration.script_id,
            })
            .collect();

        Ok(Self { events })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::active(self.len(), "startup trigger")
    }

    pub fn drain_events(&mut self) -> Vec<TriggerEvent> {
        std::mem::take(&mut self.events)
    }
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

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::active(self.len(), "file watcher")
    }
}

pub struct ProcessStartedService {
    handles: Vec<JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

impl ProcessStartedService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            handles: Vec::new(),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        sender: Sender<TriggerEvent>,
    ) -> Result<Self, TriggerError> {
        let specs = registrations
            .into_iter()
            .filter(|registration| registration.action_type == "trigger.process_started")
            .map(ProcessStartedSpec::from_registration)
            .collect::<Result<Vec<_>, _>>()?;

        if specs.is_empty() {
            return Ok(Self::empty());
        }

        let running = Arc::new(AtomicBool::new(true));
        let thread_running = Arc::clone(&running);
        let handle = thread::spawn(move || {
            run_process_started_watcher(specs, sender, thread_running);
        });

        Ok(Self {
            handles: vec![handle],
            running,
        })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::thread_backed(
            self.running.load(Ordering::Relaxed),
            self.len(),
            "process watcher thread",
        )
    }
}

impl Drop for ProcessStartedService {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }
}

pub struct WebSocketService {
    handle: Option<JoinHandle<()>>,
    route_count: usize,
    running: Arc<AtomicBool>,
}

impl WebSocketService {
    #[must_use]
    pub fn empty(_registry: Arc<WebSocketConnectionRegistry>) -> Self {
        Self {
            handle: None,
            route_count: 0,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        bind: &str,
        port: u16,
        max_message_bytes: usize,
        sender: Sender<TriggerEvent>,
        registry: Arc<WebSocketConnectionRegistry>,
    ) -> Result<Self, TriggerError> {
        let routes = registrations
            .into_iter()
            .filter(|registration| registration.action_type == "trigger.websocket")
            .map(WebSocketRoute::from_registration)
            .collect::<Result<Vec<_>, _>>()?;

        if routes.is_empty() {
            return Ok(Self::empty(registry));
        }

        let address = format!("{bind}:{port}");
        let listener = TcpListener::bind(&address).map_err(|source| {
            TriggerError::Failed(
                "trigger.websocket".to_owned(),
                format!("failed to bind WebSocket listener on {address}: {source}"),
            )
        })?;
        listener.set_nonblocking(true).map_err(|source| {
            TriggerError::Failed(
                "trigger.websocket".to_owned(),
                format!("failed to configure WebSocket listener: {source}"),
            )
        })?;

        let running = Arc::new(AtomicBool::new(true));
        let thread_running = Arc::clone(&running);
        let thread_registry = Arc::clone(&registry);
        let route_count = routes.len();
        let handle = thread::spawn(move || {
            run_websocket_listener(
                listener,
                routes,
                max_message_bytes,
                sender,
                thread_registry,
                thread_running,
            );
        });

        Ok(Self {
            handle: Some(handle),
            route_count,
            running,
        })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.route_count == 0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.route_count
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::thread_backed(
            self.running.load(Ordering::Relaxed),
            self.len(),
            "WebSocket route",
        )
    }
}

impl Drop for WebSocketService {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub struct SerialInputService {
    handles: Vec<JoinHandle<()>>,
    running: Arc<AtomicBool>,
    reader_statuses: SerialReaderStatusMap,
}

impl SerialInputService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            handles: Vec::new(),
            running: Arc::new(AtomicBool::new(false)),
            reader_statuses: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn start(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        devices: impl IntoIterator<Item = SerialDeviceConfig>,
        sender: Sender<TriggerEvent>,
        rebind_sink: Option<Arc<dyn SerialPortRebindSink>>,
    ) -> Result<Self, TriggerError> {
        let mut handles = Vec::new();
        let running = Arc::new(AtomicBool::new(true));
        let reader_statuses = Arc::new(Mutex::new(BTreeMap::new()));
        let devices = devices
            .into_iter()
            .map(|device| (device.device_id.clone(), device))
            .collect::<BTreeMap<_, _>>();

        for registration in registrations {
            if registration.action_type != "trigger.serial_input" {
                continue;
            }

            let spec = SerialInputSpec::from_registration(&registration, &devices)?;
            set_serial_reader_status(
                &reader_statuses,
                &registration,
                &spec,
                "starting",
                None,
                false,
            );
            let thread_registration = registration.clone();
            let thread_sender = sender.clone();
            let thread_running = Arc::clone(&running);
            let thread_statuses = Arc::clone(&reader_statuses);
            let thread_rebind_sink = rebind_sink.clone();
            handles.push(thread::spawn(move || {
                run_serial_input_reader(
                    thread_registration,
                    spec,
                    thread_sender,
                    thread_running,
                    thread_statuses,
                    thread_rebind_sink,
                );
            }));
        }

        Ok(Self {
            handles,
            running,
            reader_statuses,
        })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::thread_backed(
            self.running.load(Ordering::Relaxed),
            self.len(),
            "serial reader thread",
        )
    }

    #[must_use]
    pub fn reader_statuses(&self) -> Vec<SerialReaderStatus> {
        sorted_serial_reader_statuses(&self.reader_statuses)
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
pub struct HotkeyService {
    bindings: BTreeMap<String, Vec<TriggerRegistration>>,
}

impl HotkeyService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            bindings: BTreeMap::new(),
        }
    }

    pub fn from_registrations(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
    ) -> Result<Self, TriggerError> {
        let mut bindings = BTreeMap::<String, Vec<TriggerRegistration>>::new();
        for registration in registrations {
            if registration.action_type != "trigger.hotkey" {
                continue;
            }

            let hotkey = HotkeySpec::from_registration(&registration)?;
            bindings
                .entry(hotkey.normalized_key)
                .or_default()
                .push(registration);
        }

        Ok(Self { bindings })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.values().map(Vec::len).sum()
    }

    #[must_use]
    pub fn registered_hotkeys(&self) -> Vec<&str> {
        self.bindings.keys().map(String::as_str).collect()
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        let hotkey_count = self.bindings.len();
        let registration_count = self.len();
        TriggerServiceDiagnostics {
            running: registration_count > 0,
            state: if registration_count > 0 {
                "active"
            } else {
                "idle"
            },
            summary: format!(
                "{registration_count} trigger(s) across {hotkey_count} hotkey binding(s)"
            ),
        }
    }

    pub fn events_for_key(
        &self,
        key: &str,
        timestamp: SystemTime,
    ) -> Result<Vec<TriggerEvent>, TriggerError> {
        let normalized_key = normalize_hotkey(key).map_err(|message| {
            TriggerError::Failed("trigger.hotkey".to_owned(), message.to_owned())
        })?;
        let Some(registrations) = self.bindings.get(&normalized_key) else {
            return Ok(Vec::new());
        };
        let timestamp = unix_timestamp_millis(timestamp).to_string();

        Ok(registrations
            .iter()
            .map(|registration| TriggerEvent {
                node_id: registration.node_id.clone(),
                payload: json!({
                    "key": normalized_key,
                    "timestamp": timestamp,
                }),
                script_id: registration.script_id.clone(),
            })
            .collect())
    }
}

#[derive(Debug, Clone)]
struct HotkeySpec {
    normalized_key: String,
}

impl HotkeySpec {
    fn from_registration(registration: &TriggerRegistration) -> Result<Self, TriggerError> {
        let key = registration
            .config
            .get("key")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    "hotkey trigger must define key".to_owned(),
                )
            })?;
        if key.contains("{{") || key.contains("}}") {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "hotkey trigger key cannot use runtime variable templates".to_owned(),
            ));
        }

        Ok(Self {
            normalized_key: normalize_hotkey(key)
                .map_err(|message| TriggerError::Failed(registration.node_id.clone(), message))?,
        })
    }
}

fn normalize_hotkey(input: &str) -> Result<String, String> {
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut meta = false;
    let mut primary_key = None::<String>;

    for part in input
        .split(['+', '-'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        match normalized_hotkey_token(part).as_str() {
            "Ctrl" => ctrl = true,
            "Alt" => alt = true,
            "Shift" => shift = true,
            "Meta" => meta = true,
            token if primary_key.is_none() => primary_key = Some(token.to_owned()),
            token => {
                return Err(format!(
                    "hotkey {input:?} contains multiple primary keys ({:?} and {token:?})",
                    primary_key.unwrap_or_default()
                ));
            }
        }
    }

    let primary_key =
        primary_key.ok_or_else(|| format!("hotkey {input:?} must include a primary key"))?;
    let mut parts = Vec::new();
    if ctrl {
        parts.push("Ctrl".to_owned());
    }
    if alt {
        parts.push("Alt".to_owned());
    }
    if shift {
        parts.push("Shift".to_owned());
    }
    if meta {
        parts.push("Meta".to_owned());
    }
    parts.push(primary_key);

    Ok(parts.join("+"))
}

fn normalized_hotkey_token(input: &str) -> String {
    match input.trim().to_ascii_lowercase().as_str() {
        "ctrl" | "control" => "Ctrl".to_owned(),
        "alt" | "option" => "Alt".to_owned(),
        "shift" => "Shift".to_owned(),
        "meta" | "cmd" | "command" | "win" | "windows" | "super" => "Meta".to_owned(),
        "esc" | "escape" => "Escape".to_owned(),
        "return" | "enter" => "Enter".to_owned(),
        "space" | "spacebar" => "Space".to_owned(),
        "tab" => "Tab".to_owned(),
        "backspace" => "Backspace".to_owned(),
        "delete" | "del" => "Delete".to_owned(),
        "insert" | "ins" => "Insert".to_owned(),
        "home" => "Home".to_owned(),
        "end" => "End".to_owned(),
        "pageup" | "page_up" | "page up" => "PageUp".to_owned(),
        "pagedown" | "page_down" | "page down" => "PageDown".to_owned(),
        "up" | "arrowup" | "arrow up" => "ArrowUp".to_owned(),
        "down" | "arrowdown" | "arrow down" => "ArrowDown".to_owned(),
        "left" | "arrowleft" | "arrow left" => "ArrowLeft".to_owned(),
        "right" | "arrowright" | "arrow right" => "ArrowRight".to_owned(),
        token if token.len() == 1 => token.to_ascii_uppercase(),
        token => {
            let mut chars = token.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            format!("{}{}", first.to_uppercase(), chars.as_str())
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

#[derive(Debug, Clone)]
struct ProcessStartedSpec {
    match_mode: String,
    registration: TriggerRegistration,
    target: String,
}

impl ProcessStartedSpec {
    fn from_registration(registration: TriggerRegistration) -> Result<Self, TriggerError> {
        let target = registration
            .config
            .get("target")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    "process started trigger must define target".to_owned(),
                )
            })?
            .to_owned();
        let match_mode = registration
            .config
            .get("matchMode")
            .and_then(Value::as_str)
            .unwrap_or("process_name")
            .trim()
            .to_owned();
        if !matches!(
            match_mode.as_str(),
            "process_name" | "executable_path" | "window_title"
        ) {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                format!("unsupported process started match mode {match_mode:?}"),
            ));
        }
        if match_mode == "window_title" {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "window title process start matching requires the desktop runner".to_owned(),
            ));
        }

        Ok(Self {
            match_mode,
            registration,
            target,
        })
    }
}

fn run_process_started_watcher(
    specs: Vec<ProcessStartedSpec>,
    sender: Sender<TriggerEvent>,
    running: Arc<AtomicBool>,
) {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let mut seen_processes = system.processes().keys().copied().collect::<Vec<_>>();
    seen_processes.sort_by_key(|pid| pid.as_u32());

    while running.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_secs(1));
        system.refresh_processes(ProcessesToUpdate::All, true);

        let mut current_processes = system.processes().keys().copied().collect::<Vec<_>>();
        current_processes.sort_by_key(|pid| pid.as_u32());

        for pid in current_processes.iter().copied().filter(|pid| {
            seen_processes
                .binary_search_by_key(&pid.as_u32(), |seen_pid| seen_pid.as_u32())
                .is_err()
        }) {
            let Some(process) = system.process(pid) else {
                continue;
            };
            for spec in &specs {
                if process_matches_spec(process, spec) {
                    let _ = sender.send(process_started_event(spec, process));
                }
            }
        }

        seen_processes = current_processes;
    }
}

fn process_matches_spec(process: &sysinfo::Process, spec: &ProcessStartedSpec) -> bool {
    match spec.match_mode.as_str() {
        "process_name" => process
            .name()
            .to_string_lossy()
            .eq_ignore_ascii_case(spec.target.trim()),
        "executable_path" => process
            .exe()
            .map(|path| normalize_path_string(&path.display().to_string()))
            .is_some_and(|path| path == normalize_path_string(&spec.target)),
        _ => false,
    }
}

fn process_started_event(spec: &ProcessStartedSpec, process: &sysinfo::Process) -> TriggerEvent {
    TriggerEvent {
        node_id: spec.registration.node_id.clone(),
        payload: json!({
            "executable_path": process.exe().map(|path| path.display().to_string()).unwrap_or_default(),
            "process_id": process.pid().as_u32(),
            "process_name": process.name().to_string_lossy(),
            "timestamp": unix_timestamp_millis(SystemTime::now()).to_string(),
            "window_title": "",
        }),
        script_id: spec.registration.script_id.clone(),
    }
}

fn normalize_path_string(path: &str) -> String {
    path.trim().replace('\\', "/").to_ascii_lowercase()
}

#[derive(Debug, Clone)]
struct WebSocketRoute {
    path: String,
    registration: TriggerRegistration,
}

impl WebSocketRoute {
    fn from_registration(registration: TriggerRegistration) -> Result<Self, TriggerError> {
        let path = registration
            .config
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    "WebSocket trigger must define path".to_owned(),
                )
            })?
            .to_owned();
        if !path.starts_with('/') {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "WebSocket path must start with '/'".to_owned(),
            ));
        }

        Ok(Self { path, registration })
    }
}

#[derive(Debug, Clone)]
struct WebSocketHandshake {
    headers: BTreeMap<String, String>,
    path: String,
    query: BTreeMap<String, String>,
}

fn run_websocket_listener(
    listener: TcpListener,
    routes: Vec<WebSocketRoute>,
    max_message_bytes: usize,
    sender: Sender<TriggerEvent>,
    registry: Arc<WebSocketConnectionRegistry>,
    running: Arc<AtomicBool>,
) {
    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, remote_address)) => {
                let routes = routes.clone();
                let sender = sender.clone();
                let registry = Arc::clone(&registry);
                let running = Arc::clone(&running);
                thread::spawn(move || {
                    handle_websocket_connection(
                        stream,
                        remote_address.to_string(),
                        routes,
                        max_message_bytes,
                        sender,
                        registry,
                        running,
                    );
                });
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => {
                tracing::warn!("WebSocket listener accept failed: {error}");
                thread::sleep(Duration::from_millis(250));
            }
        }
    }
}

#[allow(clippy::result_large_err)] // tungstenite's handshake callback uses http::Response as its error type.
fn handle_websocket_connection(
    stream: TcpStream,
    remote_address: String,
    routes: Vec<WebSocketRoute>,
    max_message_bytes: usize,
    sender: Sender<TriggerEvent>,
    registry: Arc<WebSocketConnectionRegistry>,
    running: Arc<AtomicBool>,
) {
    let handshake = Arc::new(Mutex::new(None::<WebSocketHandshake>));
    let handshake_capture = Arc::clone(&handshake);
    let Ok(mut websocket) = accept_hdr(
        stream,
        move |request: &WebSocketHandshakeRequest, response: WebSocketHandshakeResponse| {
            let (path, query) = split_path_and_query(
                request
                    .uri()
                    .path_and_query()
                    .map_or("/", |value| value.as_str()),
            );
            let headers = request
                .headers()
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (name.as_str().to_ascii_lowercase(), value.to_owned()))
                })
                .collect();
            *handshake_capture
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(WebSocketHandshake {
                headers,
                path,
                query,
            });
            Ok(response)
        },
    ) else {
        return;
    };
    let _ = websocket
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(250)));
    let _ = websocket
        .get_mut()
        .set_write_timeout(Some(Duration::from_secs(5)));
    let handshake = handshake
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
        .unwrap_or_else(|| WebSocketHandshake {
            headers: BTreeMap::new(),
            path: "/".to_owned(),
            query: BTreeMap::new(),
        });
    let Some(route) = routes
        .iter()
        .find(|route| route.path == handshake.path)
        .cloned()
    else {
        let _ = websocket.close(None);
        return;
    };

    let connection_id = format!(
        "{}-{}",
        route.registration.node_id,
        unix_timestamp_millis(SystemTime::now())
    );
    registry.insert(connection_id.clone(), websocket);

    while running.load(Ordering::Relaxed) {
        let connection = {
            let connections = registry
                .connections
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            connections.get(&connection_id).cloned()
        };
        let Some(connection) = connection else {
            break;
        };
        let result = connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .read();
        match result {
            Ok(message) if message.is_text() || message.is_binary() => {
                let payload = websocket_payload(
                    &route,
                    &handshake,
                    &connection_id,
                    &remote_address,
                    message,
                    max_message_bytes,
                );
                match payload {
                    Ok(payload) => {
                        let _ = sender.send(TriggerEvent {
                            node_id: route.registration.node_id.clone(),
                            payload,
                            script_id: route.registration.script_id.clone(),
                        });
                    }
                    Err(error) => {
                        tracing::warn!("WebSocket message rejected: {error}");
                    }
                }
            }
            Ok(message) if message.is_close() => break,
            Ok(_) => {}
            Err(WebSocketError::Io(error))
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.kind() == io::ErrorKind::TimedOut => {}
            Err(WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed) => break,
            Err(error) => {
                tracing::warn!("WebSocket connection {connection_id} failed: {error}");
                break;
            }
        }
    }

    registry.remove(&connection_id);
}

fn websocket_payload(
    route: &WebSocketRoute,
    handshake: &WebSocketHandshake,
    connection_id: &str,
    remote_address: &str,
    message: Message,
    max_message_bytes: usize,
) -> Result<Value, String> {
    let bytes = match message {
        Message::Text(value) => value.to_string().into_bytes(),
        Message::Binary(value) => value.to_vec(),
        _ => Vec::new(),
    };
    if bytes.len() > max_message_bytes {
        return Err(format!(
            "message from {connection_id} exceeded {max_message_bytes} bytes"
        ));
    }
    let message = String::from_utf8_lossy(&bytes).into_owned();
    let json_body = serde_json::from_str::<Value>(&message).unwrap_or_else(|_| json!({}));

    Ok(json!({
        "connection_id": connection_id,
        "headers": handshake.headers,
        "json": json_body,
        "message": message,
        "path": handshake.path,
        "query": handshake.query,
        "remote_address": remote_address,
        "trigger_id": route.registration.node_id,
    }))
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
    pub auto_rebind_port: bool,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub device_id: String,
    pub flow_control: String,
    pub manufacturer: Option<String>,
    pub parity: String,
    pub port: String,
    pub product_id: Option<String>,
    pub product: Option<String>,
    pub read_mode: String,
    pub serial_number: Option<String>,
    pub stop_bits: String,
    pub validate_usb_identity: bool,
    pub vendor_id: Option<String>,
}

#[derive(Debug, Clone)]
struct SerialInputSpec {
    auto_reconnect: bool,
    auto_rebind_port: bool,
    baud_rate: u32,
    data_bits: DataBits,
    device_id: String,
    flow_control: FlowControl,
    manufacturer: Option<String>,
    parity: Parity,
    port: String,
    product_id: Option<u16>,
    product: Option<String>,
    read_mode: SerialReadMode,
    serial_number: Option<String>,
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

        if device.auto_rebind_port && !validate_usb_identity {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "serial device config must enable validate_usb_identity when auto_rebind_port is enabled"
                    .to_owned(),
            ));
        }
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
            auto_rebind_port: device.auto_rebind_port,
            baud_rate: device.baud_rate,
            data_bits: parse_data_bits(&device.data_bits.to_string()),
            device_id,
            flow_control: parse_flow_control(&device.flow_control),
            manufacturer: normalized_optional_string(&device.manufacturer),
            parity: parse_parity(&device.parity),
            port: device.port.clone(),
            product_id,
            product: normalized_optional_string(&device.product),
            read_mode: parse_read_mode(&device.read_mode),
            serial_number: normalized_optional_string(&device.serial_number),
            stop_bits: parse_stop_bits(&device.stop_bits),
            validate_usb_identity,
            vendor_id,
        })
    }
}

fn run_serial_input_reader(
    registration: TriggerRegistration,
    mut spec: SerialInputSpec,
    sender: Sender<TriggerEvent>,
    running: Arc<AtomicBool>,
    reader_statuses: SerialReaderStatusMap,
    rebind_sink: Option<Arc<dyn SerialPortRebindSink>>,
) {
    while running.load(Ordering::Relaxed) {
        set_serial_reader_status(
            &reader_statuses,
            &registration,
            &spec,
            "validating_usb_identity",
            None,
            false,
        );
        if let Err(error) = validate_serial_usb_identity(&spec) {
            let error = match rebind_serial_port_if_possible(
                &registration,
                &mut spec,
                &reader_statuses,
                rebind_sink.as_deref(),
                &error,
            ) {
                Ok(true) => continue,
                Ok(false) => error,
                Err(rebind_error) => rebind_error,
            };
            set_serial_reader_status(
                &reader_statuses,
                &registration,
                &spec,
                "usb_identity_failed",
                Some(error.clone()),
                false,
            );
            tracing::warn!(
                "serial input trigger {} USB identity validation failed: {}",
                registration.node_id,
                error
            );
            if !spec.auto_reconnect {
                set_serial_reader_status(
                    &reader_statuses,
                    &registration,
                    &spec,
                    "stopped",
                    Some(error),
                    false,
                );
                return;
            }
            sleep_serial_reconnect_delay(&running);
            continue;
        }

        set_serial_reader_status(
            &reader_statuses,
            &registration,
            &spec,
            "connecting",
            None,
            false,
        );
        let mut port = match serial_port_builder(&spec, Duration::from_millis(250)).open() {
            Ok(port) => {
                set_serial_reader_status(
                    &reader_statuses,
                    &registration,
                    &spec,
                    "connected",
                    None,
                    false,
                );
                port
            }
            Err(error) => {
                let error = error.to_string();
                let error = match rebind_serial_port_if_possible(
                    &registration,
                    &mut spec,
                    &reader_statuses,
                    rebind_sink.as_deref(),
                    &error,
                ) {
                    Ok(true) => continue,
                    Ok(false) => error,
                    Err(rebind_error) => rebind_error,
                };
                set_serial_reader_status(
                    &reader_statuses,
                    &registration,
                    &spec,
                    "open_failed",
                    Some(error.clone()),
                    false,
                );
                tracing::warn!(
                    "serial input trigger {} failed to open {}: {}",
                    registration.node_id,
                    spec.port,
                    error
                );
                if !spec.auto_reconnect {
                    set_serial_reader_status(
                        &reader_statuses,
                        &registration,
                        &spec,
                        "stopped",
                        Some(error),
                        false,
                    );
                    return;
                }
                sleep_serial_reconnect_delay(&running);
                continue;
            }
        };

        set_serial_reader_status(
            &reader_statuses,
            &registration,
            &spec,
            "reading",
            None,
            false,
        );
        let result = match spec.read_mode {
            SerialReadMode::Line => read_serial_lines(
                &registration,
                &spec,
                &sender,
                &running,
                &mut *port,
                &reader_statuses,
            ),
            SerialReadMode::Raw => read_serial_raw_chunks(
                &registration,
                &spec,
                &sender,
                &running,
                &mut *port,
                &reader_statuses,
            ),
        };

        if let Err(error) = result {
            let error = error.to_string();
            set_serial_reader_status(
                &reader_statuses,
                &registration,
                &spec,
                "read_failed",
                Some(error.clone()),
                false,
            );
            tracing::warn!(
                "serial input trigger {} read loop ended for {}: {}",
                registration.node_id,
                spec.port,
                error
            );
            if !spec.auto_reconnect {
                set_serial_reader_status(
                    &reader_statuses,
                    &registration,
                    &spec,
                    "stopped",
                    Some(error),
                    false,
                );
                return;
            }
            sleep_serial_reconnect_delay(&running);
        }
    }
    set_serial_reader_status(
        &reader_statuses,
        &registration,
        &spec,
        "stopped",
        None,
        false,
    );
}

fn read_serial_lines(
    registration: &TriggerRegistration,
    spec: &SerialInputSpec,
    sender: &Sender<TriggerEvent>,
    running: &AtomicBool,
    port: &mut dyn serialport::SerialPort,
    reader_statuses: &SerialReaderStatusMap,
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
                send_serial_event(registration, spec, sender, &line, Some(reader_statuses));
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
    reader_statuses: &SerialReaderStatusMap,
) -> io::Result<()> {
    let mut buffer = [0_u8; 1024];
    while running.load(Ordering::Relaxed) {
        match port.read(&mut buffer) {
            Ok(0) => {}
            Ok(bytes_read) => send_serial_event(
                registration,
                spec,
                sender,
                &buffer[..bytes_read],
                Some(reader_statuses),
            ),
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
    reader_statuses: Option<&SerialReaderStatusMap>,
) {
    if let Some(reader_statuses) = reader_statuses {
        set_serial_reader_status(reader_statuses, registration, spec, "reading", None, true);
    }
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

fn set_serial_reader_status(
    reader_statuses: &SerialReaderStatusMap,
    registration: &TriggerRegistration,
    spec: &SerialInputSpec,
    state: &'static str,
    error: Option<String>,
    event: bool,
) {
    let now = unix_timestamp(SystemTime::now());
    let key = serial_reader_status_key(&registration.script_id, &registration.node_id);
    let Ok(mut statuses) = reader_statuses.lock() else {
        return;
    };
    let previous_error = statuses
        .get(&key)
        .and_then(|status| status.last_error.clone());
    let previous_error_unix = statuses.get(&key).and_then(|status| status.last_error_unix);
    let previous_event_unix = statuses.get(&key).and_then(|status| status.last_event_unix);
    statuses.insert(
        key,
        SerialReaderStatus {
            auto_reconnect: spec.auto_reconnect,
            auto_rebind_port: spec.auto_rebind_port,
            device_id: spec.device_id.clone(),
            last_error: error.clone().or(previous_error),
            last_error_unix: if error.is_some() {
                Some(now)
            } else {
                previous_error_unix
            },
            last_event_unix: if event {
                Some(now)
            } else {
                previous_event_unix
            },
            node_id: registration.node_id.clone(),
            port: spec.port.clone(),
            script_id: registration.script_id.clone(),
            state,
        },
    );
}

fn sorted_serial_reader_statuses(
    reader_statuses: &SerialReaderStatusMap,
) -> Vec<SerialReaderStatus> {
    let Ok(statuses) = reader_statuses.lock() else {
        return Vec::new();
    };
    statuses.values().cloned().collect()
}

fn serial_reader_status_key(script_id: &str, node_id: &str) -> String {
    format!("{script_id}::{node_id}")
}

fn serial_port_builder(spec: &SerialInputSpec, timeout: Duration) -> SerialPortBuilder {
    serialport::new(&spec.port, spec.baud_rate)
        .data_bits(spec.data_bits)
        .flow_control(spec.flow_control)
        .parity(spec.parity)
        .stop_bits(spec.stop_bits)
        .timeout(timeout)
}

fn rebind_serial_port_if_possible(
    registration: &TriggerRegistration,
    spec: &mut SerialInputSpec,
    reader_statuses: &SerialReaderStatusMap,
    rebind_sink: Option<&dyn SerialPortRebindSink>,
    original_error: &str,
) -> Result<bool, String> {
    if !spec.auto_rebind_port {
        return Ok(false);
    }

    set_serial_reader_status(
        reader_statuses,
        registration,
        spec,
        "rebinding_port",
        Some(original_error.to_owned()),
        false,
    );
    let matching_ports = find_matching_serial_ports(spec)?;
    match matching_ports.as_slice() {
        [] => Err(format!(
            "serial port {} failed ({original_error}) and no matching USB serial device was found for device id {}",
            spec.port, spec.device_id
        )),
        [port] if port.port_name == spec.port => Ok(false),
        [port] => {
            if let Some(sink) = rebind_sink {
                sink.update_serial_device_port(&spec.device_id, &port.port_name)
                    .map_err(|error| {
                        format!(
                            "found matching serial device on {} but failed to save port rebind: {error}",
                            port.port_name
                        )
                    })?;
            } else {
                return Err(format!(
                    "found matching serial device on {} but this runner cannot persist serial port rebinds",
                    port.port_name
                ));
            }
            spec.port.clone_from(&port.port_name);
            set_serial_reader_status(
                reader_statuses,
                registration,
                spec,
                "port_rebound",
                None,
                false,
            );
            Ok(true)
        }
        ports => {
            let names = ports
                .iter()
                .map(|port| port.port_name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "serial device id {} matched multiple ports ({names}); set serial_number, manufacturer, or product to make auto_rebind_port unambiguous",
                spec.device_id
            ))
        }
    }
}

fn find_matching_serial_ports(
    spec: &SerialInputSpec,
) -> Result<Vec<serialport::SerialPortInfo>, String> {
    if !spec.validate_usb_identity {
        return Err("auto_rebind_port requires validate_usb_identity".to_owned());
    }
    if spec.vendor_id.is_none() || spec.product_id.is_none() {
        return Err("auto_rebind_port requires vendor_id and product_id".to_owned());
    }

    available_ports()
        .map_err(|source| format!("failed to list serial ports for auto rebind: {source}"))
        .map(|ports| {
            ports
                .into_iter()
                .filter(|port| usb_port_matches_identity(port, spec))
                .collect()
        })
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

    validate_usb_port_identity(&info, spec)?;
    Ok(())
}

fn usb_port_matches_identity(port: &serialport::SerialPortInfo, spec: &SerialInputSpec) -> bool {
    let SerialPortType::UsbPort(info) = &port.port_type else {
        return false;
    };
    validate_usb_port_identity(info, spec).is_ok()
}

fn validate_usb_port_identity(
    info: &serialport::UsbPortInfo,
    spec: &SerialInputSpec,
) -> Result<(), String> {
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
    if let Some(serial_number) = &spec.serial_number
        && optional_string_mismatch(info.serial_number.as_deref(), serial_number)
    {
        return Err(format!("serial number mismatch: expected {serial_number}"));
    }
    if let Some(manufacturer) = &spec.manufacturer
        && optional_string_mismatch(info.manufacturer.as_deref(), manufacturer)
    {
        return Err(format!("manufacturer mismatch: expected {manufacturer}"));
    }
    if let Some(product) = &spec.product
        && optional_string_mismatch(info.product.as_deref(), product)
    {
        return Err(format!("product mismatch: expected {product}"));
    }
    Ok(())
}

fn optional_string_mismatch(actual: Option<&str>, expected: &str) -> bool {
    actual
        .map(str::trim)
        .is_none_or(|actual| !actual.eq_ignore_ascii_case(expected.trim()))
}

fn normalized_optional_string(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
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
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::active(self.len(), "webhook route")
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

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::active(self.len(), "schedule")
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
    fn parses_hotkey_registration_and_builds_payload() {
        let service = HotkeyService::from_registrations([hotkey_registration(json!({
            "key": "Control + Option + b"
        }))])
        .expect("hotkey should parse");

        assert_eq!(service.len(), 1);
        assert_eq!(service.registered_hotkeys(), ["Ctrl+Alt+B"]);

        let events = service
            .events_for_key("Ctrl+Alt+B", UNIX_EPOCH + Duration::from_secs(7))
            .expect("hotkey event should build");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].script_id, "script-1");
        assert_eq!(events[0].node_id, "n-hotkey");
        assert_eq!(events[0].payload["key"], "Ctrl+Alt+B");
        assert_eq!(events[0].payload["timestamp"], "7000");
    }

    #[test]
    fn hotkey_service_supports_multiple_scripts_on_same_combo() {
        let mut first = hotkey_registration(json!({"key": "Ctrl+Alt+B"}));
        first.script_id = "script-1".to_owned();
        first.node_id = "n-hotkey-1".to_owned();
        let mut second = hotkey_registration(json!({"key": "control-option-b"}));
        second.script_id = "script-2".to_owned();
        second.node_id = "n-hotkey-2".to_owned();

        let service =
            HotkeyService::from_registrations([first, second]).expect("hotkeys should parse");
        let events = service
            .events_for_key("Ctrl+Alt+B", UNIX_EPOCH)
            .expect("hotkey events should build");

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].script_id, "script-1");
        assert_eq!(events[1].script_id, "script-2");
    }

    #[test]
    fn reports_trigger_service_diagnostics() {
        let schedule = ScheduleService::from_registrations(
            [TriggerRegistration {
                action_type: "trigger.schedule".to_owned(),
                config: json!({"every": "2", "unit": "seconds"}),
                node_id: "n-schedule".to_owned(),
                runner_type: "schedule".to_owned(),
                script_id: "script-1".to_owned(),
                script_name: "Script One".to_owned(),
            }],
            Instant::now(),
        )
        .expect("schedule should parse");
        let hotkeys = HotkeyService::from_registrations([
            hotkey_registration(json!({"key": "Ctrl+Alt+B"})),
            hotkey_registration(json!({"key": "Ctrl+Alt+C"})),
        ])
        .expect("hotkeys should parse");

        let schedule_diagnostics = schedule.diagnostics();
        let hotkey_diagnostics = hotkeys.diagnostics();

        assert!(schedule_diagnostics.running);
        assert_eq!(schedule_diagnostics.state, "active");
        assert!(schedule_diagnostics.summary.contains("1 schedule"));
        assert!(hotkey_diagnostics.running);
        assert_eq!(hotkey_diagnostics.state, "active");
        assert!(hotkey_diagnostics.summary.contains("2 hotkey binding(s)"));
    }

    #[test]
    fn hotkey_service_ignores_unmatched_keys() {
        let service = HotkeyService::from_registrations([hotkey_registration(json!({
            "key": "Ctrl+Alt+B"
        }))])
        .expect("hotkey should parse");

        assert!(
            service
                .events_for_key("Ctrl+Alt+C", UNIX_EPOCH)
                .expect("unmatched key should be accepted")
                .is_empty()
        );
    }

    #[test]
    fn rejects_hotkey_without_primary_key() {
        let registration = hotkey_registration(json!({"key": "Ctrl+Alt"}));

        let error =
            HotkeySpec::from_registration(&registration).expect_err("missing key should fail");

        assert!(error.to_string().contains("primary key"));
    }

    #[test]
    fn rejects_hotkey_runtime_templates() {
        let registration = hotkey_registration(json!({"key": "{{dynamic_key}}"}));

        let error =
            HotkeySpec::from_registration(&registration).expect_err("template key should fail");

        assert!(error.to_string().contains("runtime variable templates"));
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
    fn creates_startup_events_once_and_drains_them() {
        let registration = startup_registration();
        let startup_time = UNIX_EPOCH + Duration::from_secs(42);
        let mut service = StartupService::from_registrations([registration], startup_time)
            .expect("startup trigger should parse");

        assert_eq!(service.len(), 1);

        let events = service.drain_events();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].script_id, "script-1");
        assert_eq!(events[0].node_id, "n-startup");
        assert_eq!(events[0].payload["reason"], "runner_startup");
        assert_eq!(events[0].payload["timestamp"], "42000");
        assert!(service.is_empty());
    }

    #[test]
    fn parses_process_started_registration() {
        let registration = process_started_registration(json!({
            "matchMode": "process_name",
            "target": "app.exe"
        }));

        let spec = ProcessStartedSpec::from_registration(registration)
            .expect("process started trigger should parse");

        assert_eq!(spec.match_mode, "process_name");
        assert_eq!(spec.target, "app.exe");
    }

    #[test]
    fn rejects_desktop_only_process_started_window_matching() {
        let registration = process_started_registration(json!({
            "matchMode": "window_title",
            "target": "BaudBound"
        }));

        let error = ProcessStartedSpec::from_registration(registration)
            .expect_err("window matching should require desktop runner");

        assert!(error.to_string().contains("desktop runner"));
    }

    #[test]
    fn parses_websocket_trigger_route() {
        let registration = websocket_registration(json!({
            "path": "/events/messages"
        }));

        let route =
            WebSocketRoute::from_registration(registration).expect("websocket route should parse");

        assert_eq!(route.path, "/events/messages");
        assert_eq!(route.registration.node_id, "n-websocket");
    }

    #[test]
    fn builds_websocket_trigger_payload() {
        let route = WebSocketRoute::from_registration(websocket_registration(json!({
            "path": "/events/messages"
        })))
        .expect("websocket route should parse");
        let handshake = WebSocketHandshake {
            headers: BTreeMap::from([("sec-websocket-protocol".to_owned(), "json".to_owned())]),
            path: "/events/messages".to_owned(),
            query: BTreeMap::from([("token".to_owned(), "abc".to_owned())]),
        };

        let payload = websocket_payload(
            &route,
            &handshake,
            "conn-1",
            "127.0.0.1:50000",
            Message::Text(r#"{"event":"ok"}"#.to_owned().into()),
            1024,
        )
        .expect("websocket payload should build");

        assert_eq!(payload["connection_id"], "conn-1");
        assert_eq!(payload["path"], "/events/messages");
        assert_eq!(payload["query"]["token"], "abc");
        assert_eq!(payload["headers"]["sec-websocket-protocol"], "json");
        assert_eq!(payload["message"], r#"{"event":"ok"}"#);
        assert_eq!(payload["json"]["event"], "ok");
        assert_eq!(payload["remote_address"], "127.0.0.1:50000");
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

        send_serial_event(&registration, &spec, &sender, b"hello", None);

        let event = receiver.recv().expect("serial event should be sent");
        assert_eq!(event.script_id, "script-1");
        assert_eq!(event.node_id, "n-serial");
        assert_eq!(event.payload["device_id"], "main-device");
        assert_eq!(event.payload["data"], "hello");
        assert_eq!(event.payload["bytes"], 5);
        assert!(event.payload["timestamp"].as_str().is_some());
    }

    #[test]
    fn tracks_serial_reader_status() {
        let registration = serial_input_registration(json!({
            "deviceId": "main-device"
        }));
        let devices = BTreeMap::from([("main-device".to_owned(), serial_device_config())]);
        let spec = SerialInputSpec::from_registration(&registration, &devices)
            .expect("serial config should parse");
        let statuses = Arc::new(Mutex::new(BTreeMap::new()));
        let (sender, receiver) = std::sync::mpsc::channel();

        set_serial_reader_status(
            &statuses,
            &registration,
            &spec,
            "connecting",
            Some("open failed".to_owned()),
            false,
        );
        send_serial_event(&registration, &spec, &sender, b"hello", Some(&statuses));

        let _ = receiver.recv().expect("serial event should be sent");
        let readers = sorted_serial_reader_statuses(&statuses);
        assert_eq!(readers.len(), 1);
        assert_eq!(readers[0].device_id, "main-device");
        assert_eq!(readers[0].state, "reading");
        assert_eq!(readers[0].last_error.as_deref(), Some("open failed"));
        assert!(readers[0].last_error_unix.is_some());
        assert!(readers[0].last_event_unix.is_some());
    }

    #[test]
    fn matches_serial_usb_identity_with_optional_stronger_fields() {
        let registration = serial_input_registration(json!({
            "deviceId": "main-device"
        }));
        let devices = BTreeMap::from([(
            "main-device".to_owned(),
            SerialDeviceConfig {
                auto_rebind_port: true,
                serial_number: Some("ABC123".to_owned()),
                manufacturer: Some("Acme".to_owned()),
                product: Some("Controller".to_owned()),
                ..serial_device_config()
            },
        )]);
        let spec = SerialInputSpec::from_registration(&registration, &devices)
            .expect("serial config should parse");
        let matching_port = serial_port_info(
            "COM7",
            0x1A86,
            0x7523,
            Some("ABC123"),
            Some("Acme"),
            Some("Controller"),
        );
        let wrong_serial = serial_port_info(
            "COM8",
            0x1A86,
            0x7523,
            Some("XYZ789"),
            Some("Acme"),
            Some("Controller"),
        );

        assert!(usb_port_matches_identity(&matching_port, &spec));
        assert!(!usb_port_matches_identity(&wrong_serial, &spec));
    }

    fn serial_port_info(
        port_name: &str,
        vid: u16,
        pid: u16,
        serial_number: Option<&str>,
        manufacturer: Option<&str>,
        product: Option<&str>,
    ) -> serialport::SerialPortInfo {
        serialport::SerialPortInfo {
            port_name: port_name.to_owned(),
            port_type: SerialPortType::UsbPort(serialport::UsbPortInfo {
                vid,
                pid,
                serial_number: serial_number.map(ToOwned::to_owned),
                manufacturer: manufacturer.map(ToOwned::to_owned),
                product: product.map(ToOwned::to_owned),
            }),
        }
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

    fn websocket_registration(config: Value) -> TriggerRegistration {
        TriggerRegistration {
            action_type: "trigger.websocket".to_owned(),
            config,
            node_id: "n-websocket".to_owned(),
            runner_type: "websocket".to_owned(),
            script_id: "script-1".to_owned(),
            script_name: "Script One".to_owned(),
        }
    }

    fn startup_registration() -> TriggerRegistration {
        TriggerRegistration {
            action_type: "trigger.startup".to_owned(),
            config: json!({}),
            node_id: "n-startup".to_owned(),
            runner_type: "startup".to_owned(),
            script_id: "script-1".to_owned(),
            script_name: "Script One".to_owned(),
        }
    }

    fn process_started_registration(config: Value) -> TriggerRegistration {
        TriggerRegistration {
            action_type: "trigger.process_started".to_owned(),
            config,
            node_id: "n-process-started".to_owned(),
            runner_type: "process_started".to_owned(),
            script_id: "script-1".to_owned(),
            script_name: "Script One".to_owned(),
        }
    }

    fn hotkey_registration(config: Value) -> TriggerRegistration {
        TriggerRegistration {
            action_type: "trigger.hotkey".to_owned(),
            config,
            node_id: "n-hotkey".to_owned(),
            runner_type: "hotkey".to_owned(),
            script_id: "script-1".to_owned(),
            script_name: "Script One".to_owned(),
        }
    }

    fn serial_device_config() -> SerialDeviceConfig {
        SerialDeviceConfig {
            auto_reconnect: true,
            auto_rebind_port: false,
            baud_rate: 115_200,
            data_bits: 8,
            device_id: "main-device".to_owned(),
            flow_control: "none".to_owned(),
            manufacturer: None,
            parity: "none".to_owned(),
            port: "COM3".to_owned(),
            product_id: Some("7523".to_owned()),
            product: None,
            read_mode: "line".to_owned(),
            serial_number: None,
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
