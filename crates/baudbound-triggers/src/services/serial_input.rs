use std::{
    collections::BTreeMap,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime},
};

use baudbound_actions::SerialConnectionRegistry;
use serde::Serialize;
use serde_json::json;

use crate::{
    SerialPortRebindSink, TriggerError, TriggerEvent, TriggerRegistration,
    TriggerServiceDiagnostics, required_config_string, try_send_trigger_event, unix_timestamp,
    unix_timestamp_millis,
};

mod framing;

use framing::{FrameEvent, IdleGapFramer, LineFramer};
#[derive(Debug, Clone, Serialize)]
pub struct SerialReaderStatus {
    pub auto_reconnect: bool,
    pub auto_rebind_port: bool,
    pub device_id: String,
    pub buffered_bytes: usize,
    pub last_error: Option<String>,
    pub last_error_unix: Option<u64>,
    pub last_event_unix: Option<u64>,
    pub last_framing_error: Option<String>,
    pub last_framing_error_unix: Option<u64>,
    pub last_rebind_result: Option<String>,
    pub last_rebind_unix: Option<u64>,
    pub node_id: String,
    pub port: String,
    pub read_mode: &'static str,
    pub script_id: String,
    pub state: &'static str,
}

type SerialReaderStatusMap = Arc<Mutex<BTreeMap<String, SerialReaderStatus>>>;

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
        connections: Arc<SerialConnectionRegistry>,
        sender: SyncSender<TriggerEvent>,
        rebind_sink: Option<Arc<dyn SerialPortRebindSink>>,
    ) -> Result<Self, TriggerError> {
        let mut handles = Vec::new();
        let running = Arc::new(AtomicBool::new(true));
        let reader_statuses = Arc::new(Mutex::new(BTreeMap::new()));
        let readers = serial_reader_groups(registrations, &connections)?;

        for (_, (spec, registrations)) in readers {
            set_serial_reader_statuses(
                &reader_statuses,
                &registrations,
                &spec,
                "starting",
                None,
                false,
            );
            let thread_sender = sender.clone();
            let thread_running = Arc::clone(&running);
            let thread_statuses = Arc::clone(&reader_statuses);
            let thread_rebind_sink = rebind_sink.clone();
            let thread_connections = Arc::clone(&connections);
            handles.push(thread::spawn(move || {
                run_serial_input_reader(
                    registrations,
                    spec,
                    thread_sender,
                    thread_running,
                    thread_statuses,
                    thread_rebind_sink,
                    thread_connections,
                );
            }));
        }

        Ok(Self {
            handles,
            running,
            reader_statuses,
        })
    }

    pub fn validate(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        connections: &SerialConnectionRegistry,
    ) -> Result<(), TriggerError> {
        serial_reader_groups(registrations, connections).map(|_| ())
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

pub(crate) fn serial_reader_groups(
    registrations: impl IntoIterator<Item = TriggerRegistration>,
    connections: &SerialConnectionRegistry,
) -> Result<BTreeMap<String, (SerialInputSpec, Vec<TriggerRegistration>)>, TriggerError> {
    let mut readers = BTreeMap::<String, (SerialInputSpec, Vec<TriggerRegistration>)>::new();
    for registration in registrations {
        if registration.action_type != "trigger.serial_input" {
            continue;
        }
        let spec = SerialInputSpec::from_registration(&registration, connections)?;
        readers
            .entry(spec.device_id.clone())
            .or_insert_with(|| (spec, Vec::new()))
            .1
            .push(registration);
    }
    Ok(readers)
}

impl Drop for SerialInputService {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }
}

pub use baudbound_actions::SerialDeviceConfig;

#[derive(Debug, Clone)]
pub(crate) struct SerialInputSpec {
    pub(crate) auto_reconnect: bool,
    pub(crate) auto_rebind_port: bool,
    pub(crate) device_id: String,
    pub(crate) max_message_bytes: usize,
    pub(crate) message_gap: Duration,
    pub(crate) open_stabilization: Duration,
    pub(crate) port: String,
    pub(crate) read_mode: SerialReadMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SerialReadMode {
    IdleGap,
    Line,
    Raw,
}

impl SerialReadMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::IdleGap => "idle_gap",
            Self::Line => "line",
            Self::Raw => "raw",
        }
    }
}

impl SerialInputSpec {
    pub(crate) fn from_registration(
        registration: &TriggerRegistration,
        connections: &SerialConnectionRegistry,
    ) -> Result<Self, TriggerError> {
        let device_id = required_config_string(registration, "deviceId")?;
        let device = connections.config(&device_id).ok_or_else(|| {
            TriggerError::Failed(
                registration.node_id.clone(),
                format!(
                    "serial input trigger references device id {device_id:?}, but runner config has no matching [serial.devices.{device_id}] entry"
                ),
            )
        })?;
        if device.auto_rebind_port && !device.validate_usb_identity {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "serial device config must enable validate_usb_identity when auto_rebind_port is enabled"
                    .to_owned(),
            ));
        }
        if device.validate_usb_identity
            && device
                .vendor_id
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "serial device config must define vendor_id when USB identity validation is enabled"
                    .to_owned(),
            ));
        }
        if device.validate_usb_identity
            && device
                .product_id
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "serial device config must define product_id when USB identity validation is enabled"
                    .to_owned(),
            ));
        }

        Ok(Self {
            auto_reconnect: device.auto_reconnect,
            auto_rebind_port: device.auto_rebind_port,
            device_id,
            max_message_bytes: device.max_message_bytes,
            message_gap: Duration::from_millis(device.message_gap_ms),
            open_stabilization: Duration::from_millis(device.open_stabilization_ms),
            port: device.port,
            read_mode: parse_read_mode(registration, &device.read_mode)?,
        })
    }
}

fn run_serial_input_reader(
    registrations: Vec<TriggerRegistration>,
    mut spec: SerialInputSpec,
    sender: SyncSender<TriggerEvent>,
    running: Arc<AtomicBool>,
    reader_statuses: SerialReaderStatusMap,
    rebind_sink: Option<Arc<dyn SerialPortRebindSink>>,
    connections: Arc<SerialConnectionRegistry>,
) {
    let reader_name = registrations
        .first()
        .map(|registration| registration.node_id.as_str())
        .unwrap_or("serial-reader");
    while running.load(Ordering::Relaxed) {
        set_serial_reader_statuses(
            &reader_statuses,
            &registrations,
            &spec,
            "validating_usb_identity",
            None,
            false,
        );
        if let Err(error) = connections.validate_identity(&spec.device_id) {
            let error = match rebind_serial_port_if_possible(
                &registrations,
                &mut spec,
                &reader_statuses,
                rebind_sink.as_deref(),
                &connections,
                &error,
            ) {
                Ok(true) => continue,
                Ok(false) => error,
                Err(rebind_error) => rebind_error,
            };
            set_serial_reader_statuses(
                &reader_statuses,
                &registrations,
                &spec,
                "usb_identity_failed",
                Some(error.clone()),
                false,
            );
            tracing::warn!(
                "serial input trigger {} USB identity validation failed: {}",
                reader_name,
                error
            );
            if !spec.auto_reconnect {
                set_serial_reader_statuses(
                    &reader_statuses,
                    &registrations,
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

        set_serial_reader_statuses(
            &reader_statuses,
            &registrations,
            &spec,
            "connecting",
            None,
            false,
        );
        match connections.connect(&spec.device_id, serial_read_timeout(&spec)) {
            Ok(active_port) => {
                spec.port = active_port;
                set_serial_reader_statuses(
                    &reader_statuses,
                    &registrations,
                    &spec,
                    "stabilizing",
                    None,
                    false,
                );
                if !sleep_while_running(&running, spec.open_stabilization) {
                    break;
                }
                if !spec.open_stabilization.is_zero() {
                    set_serial_reader_statuses(
                        &reader_statuses,
                        &registrations,
                        &spec,
                        "discarding_startup_input",
                        None,
                        false,
                    );
                    if let Err(error) = connections.clear_input(&spec.device_id) {
                        connections.close(&spec.device_id);
                        set_serial_reader_statuses(
                            &reader_statuses,
                            &registrations,
                            &spec,
                            "startup_input_clear_failed",
                            Some(error.clone()),
                            false,
                        );
                        tracing::warn!(
                            "serial input trigger {} failed to discard startup input for {}: {}",
                            reader_name,
                            spec.port,
                            error
                        );
                        if !spec.auto_reconnect {
                            return;
                        }
                        sleep_serial_reconnect_delay(&running);
                        continue;
                    }
                }
            }
            Err(error) => {
                let error = match rebind_serial_port_if_possible(
                    &registrations,
                    &mut spec,
                    &reader_statuses,
                    rebind_sink.as_deref(),
                    &connections,
                    &error,
                ) {
                    Ok(true) => continue,
                    Ok(false) => error,
                    Err(rebind_error) => rebind_error,
                };
                set_serial_reader_statuses(
                    &reader_statuses,
                    &registrations,
                    &spec,
                    "open_failed",
                    Some(error.clone()),
                    false,
                );
                tracing::warn!(
                    "serial input trigger {} failed to open {}: {}",
                    reader_name,
                    spec.port,
                    error
                );
                if !spec.auto_reconnect {
                    set_serial_reader_statuses(
                        &reader_statuses,
                        &registrations,
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
        }
        set_serial_reader_statuses(
            &reader_statuses,
            &registrations,
            &spec,
            "reading",
            None,
            false,
        );
        let result = match spec.read_mode {
            SerialReadMode::IdleGap => read_serial_idle_gap(
                &registrations,
                &spec,
                &sender,
                &running,
                &reader_statuses,
                &connections,
            ),
            SerialReadMode::Line => read_serial_lines(
                &registrations,
                &spec,
                &sender,
                &running,
                &reader_statuses,
                &connections,
            ),
            SerialReadMode::Raw => read_serial_raw_chunks(
                &registrations,
                &spec,
                &sender,
                &running,
                &reader_statuses,
                &connections,
            ),
        };

        if let Err(error) = result {
            let error = error.to_string();
            connections.close(&spec.device_id);
            set_serial_reader_statuses(
                &reader_statuses,
                &registrations,
                &spec,
                "read_failed",
                Some(error.clone()),
                false,
            );
            tracing::warn!(
                "serial input trigger {} read loop ended for {}: {}",
                reader_name,
                spec.port,
                error
            );
            if !spec.auto_reconnect {
                set_serial_reader_statuses(
                    &reader_statuses,
                    &registrations,
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
    connections.close(&spec.device_id);
    set_serial_reader_statuses(
        &reader_statuses,
        &registrations,
        &spec,
        "stopped",
        None,
        false,
    );
}

fn read_serial_lines(
    registrations: &[TriggerRegistration],
    spec: &SerialInputSpec,
    sender: &SyncSender<TriggerEvent>,
    running: &AtomicBool,
    reader_statuses: &SerialReaderStatusMap,
    connections: &SerialConnectionRegistry,
) -> io::Result<()> {
    let mut framer = LineFramer::new(spec.max_message_bytes);
    let mut buffer = [0_u8; 1024];
    while running.load(Ordering::Relaxed) {
        match connections.read(&spec.device_id, &mut buffer, serial_read_timeout(spec)) {
            Ok(0) => {}
            Ok(bytes_read) => {
                for event in framer.push(&buffer[..bytes_read]) {
                    handle_frame_event(registrations, spec, sender, reader_statuses, event);
                }
                update_serial_reader_buffers(
                    reader_statuses,
                    registrations,
                    framer.buffered_bytes(),
                );
            }
            Err(error) if error.kind() == io::ErrorKind::TimedOut => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn read_serial_idle_gap(
    registrations: &[TriggerRegistration],
    spec: &SerialInputSpec,
    sender: &SyncSender<TriggerEvent>,
    running: &AtomicBool,
    reader_statuses: &SerialReaderStatusMap,
    connections: &SerialConnectionRegistry,
) -> io::Result<()> {
    let mut framer = IdleGapFramer::new(spec.message_gap, spec.max_message_bytes);
    let mut buffer = [0_u8; 1024];
    while running.load(Ordering::Relaxed) {
        let now = std::time::Instant::now();
        match connections.read(&spec.device_id, &mut buffer, serial_read_timeout(spec)) {
            Ok(0) => {}
            Ok(bytes_read) => {
                if let Some(event) = framer.push(&buffer[..bytes_read], now) {
                    handle_frame_event(registrations, spec, sender, reader_statuses, event);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::TimedOut => {}
            Err(error) => return Err(error),
        }
        if let Some(event) = framer.poll(std::time::Instant::now()) {
            handle_frame_event(registrations, spec, sender, reader_statuses, event);
        }
        update_serial_reader_buffers(reader_statuses, registrations, framer.buffered_bytes());
    }
    Ok(())
}

fn handle_frame_event(
    registrations: &[TriggerRegistration],
    spec: &SerialInputSpec,
    sender: &SyncSender<TriggerEvent>,
    reader_statuses: &SerialReaderStatusMap,
    event: FrameEvent,
) {
    match event {
        FrameEvent::Message(message) => {
            for registration in registrations {
                send_serial_event(registration, spec, sender, &message, Some(reader_statuses));
            }
        }
        FrameEvent::Oversized => record_serial_framing_errors(
            reader_statuses,
            registrations,
            format!(
                "message exceeded the configured {} byte limit and was discarded",
                spec.max_message_bytes
            ),
        ),
    }
}

fn read_serial_raw_chunks(
    registrations: &[TriggerRegistration],
    spec: &SerialInputSpec,
    sender: &SyncSender<TriggerEvent>,
    running: &AtomicBool,
    reader_statuses: &SerialReaderStatusMap,
    connections: &SerialConnectionRegistry,
) -> io::Result<()> {
    let mut buffer = [0_u8; 1024];
    while running.load(Ordering::Relaxed) {
        match connections.read(&spec.device_id, &mut buffer, serial_read_timeout(spec)) {
            Ok(0) => {}
            Ok(bytes_read) => {
                for registration in registrations {
                    send_serial_event(
                        registration,
                        spec,
                        sender,
                        &buffer[..bytes_read],
                        Some(reader_statuses),
                    );
                }
            }
            Err(error) if error.kind() == io::ErrorKind::TimedOut => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(crate) fn send_serial_event(
    registration: &TriggerRegistration,
    spec: &SerialInputSpec,
    sender: &SyncSender<TriggerEvent>,
    bytes: &[u8],
    reader_statuses: Option<&SerialReaderStatusMap>,
) {
    if let Some(reader_statuses) = reader_statuses {
        set_serial_reader_status(reader_statuses, registration, spec, "reading", None, true);
    }
    let data = String::from_utf8_lossy(bytes).into_owned();
    try_send_trigger_event(
        sender,
        TriggerEvent {
            action_type: registration.action_type.clone(),
            node_id: registration.node_id.clone(),
            payload: json!({
                "bytes": bytes.len(),
                "data": data,
                "device_id": spec.device_id,
                "timestamp": unix_timestamp_millis(SystemTime::now()).to_string(),
            }),
            script_id: registration.script_id.clone(),
        },
        "serial input",
    );
}

pub(crate) fn set_serial_reader_status(
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
    let previous_buffered_bytes = statuses.get(&key).map_or(0, |status| status.buffered_bytes);
    let previous_framing_error = statuses
        .get(&key)
        .and_then(|status| status.last_framing_error.clone());
    let previous_framing_error_unix = statuses
        .get(&key)
        .and_then(|status| status.last_framing_error_unix);
    let previous_rebind_result = statuses
        .get(&key)
        .and_then(|status| status.last_rebind_result.clone());
    let previous_rebind_unix = statuses
        .get(&key)
        .and_then(|status| status.last_rebind_unix);
    statuses.insert(
        key,
        SerialReaderStatus {
            auto_reconnect: spec.auto_reconnect,
            auto_rebind_port: spec.auto_rebind_port,
            buffered_bytes: if event { 0 } else { previous_buffered_bytes },
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
            last_framing_error: previous_framing_error,
            last_framing_error_unix: previous_framing_error_unix,
            last_rebind_result: previous_rebind_result,
            last_rebind_unix: previous_rebind_unix,
            node_id: registration.node_id.clone(),
            port: spec.port.clone(),
            read_mode: spec.read_mode.as_str(),
            script_id: registration.script_id.clone(),
            state,
        },
    );
}

fn set_serial_reader_statuses(
    reader_statuses: &SerialReaderStatusMap,
    registrations: &[TriggerRegistration],
    spec: &SerialInputSpec,
    state: &'static str,
    last_error: Option<String>,
    record_event: bool,
) {
    for registration in registrations {
        set_serial_reader_status(
            reader_statuses,
            registration,
            spec,
            state,
            last_error.clone(),
            record_event,
        );
    }
}

fn update_serial_reader_buffer(
    reader_statuses: &SerialReaderStatusMap,
    registration: &TriggerRegistration,
    buffered_bytes: usize,
) {
    let key = serial_reader_status_key(&registration.script_id, &registration.node_id);
    if let Ok(mut statuses) = reader_statuses.lock()
        && let Some(status) = statuses.get_mut(&key)
    {
        status.buffered_bytes = buffered_bytes;
    }
}

fn update_serial_reader_buffers(
    reader_statuses: &SerialReaderStatusMap,
    registrations: &[TriggerRegistration],
    buffered_bytes: usize,
) {
    for registration in registrations {
        update_serial_reader_buffer(reader_statuses, registration, buffered_bytes);
    }
}

fn record_serial_framing_error(
    reader_statuses: &SerialReaderStatusMap,
    registration: &TriggerRegistration,
    error: String,
) {
    let key = serial_reader_status_key(&registration.script_id, &registration.node_id);
    if let Ok(mut statuses) = reader_statuses.lock()
        && let Some(status) = statuses.get_mut(&key)
    {
        status.buffered_bytes = 0;
        status.last_framing_error = Some(error.clone());
        status.last_framing_error_unix = Some(unix_timestamp(SystemTime::now()));
    }
    tracing::warn!(
        "serial input trigger {} framing error: {}",
        registration.node_id,
        error
    );
}

fn record_serial_framing_errors(
    reader_statuses: &SerialReaderStatusMap,
    registrations: &[TriggerRegistration],
    error: String,
) {
    for registration in registrations {
        record_serial_framing_error(reader_statuses, registration, error.clone());
    }
}

fn record_serial_rebind_result(
    reader_statuses: &SerialReaderStatusMap,
    registrations: &[TriggerRegistration],
    result: String,
) {
    let timestamp = unix_timestamp(SystemTime::now());
    if let Ok(mut statuses) = reader_statuses.lock() {
        for registration in registrations {
            let key = serial_reader_status_key(&registration.script_id, &registration.node_id);
            if let Some(status) = statuses.get_mut(&key) {
                status.last_rebind_result = Some(result.clone());
                status.last_rebind_unix = Some(timestamp);
            }
        }
    }
}

pub(crate) fn sorted_serial_reader_statuses(
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

fn serial_read_timeout(spec: &SerialInputSpec) -> Duration {
    match spec.read_mode {
        SerialReadMode::IdleGap => spec.message_gap.min(Duration::from_millis(25)),
        SerialReadMode::Line | SerialReadMode::Raw => Duration::from_millis(250),
    }
}

fn rebind_serial_port_if_possible(
    registrations: &[TriggerRegistration],
    spec: &mut SerialInputSpec,
    reader_statuses: &SerialReaderStatusMap,
    rebind_sink: Option<&dyn SerialPortRebindSink>,
    connections: &SerialConnectionRegistry,
    original_error: &str,
) -> Result<bool, String> {
    if !spec.auto_rebind_port {
        return Ok(false);
    }

    set_serial_reader_statuses(
        reader_statuses,
        registrations,
        spec,
        "rebinding_port",
        Some(original_error.to_owned()),
        false,
    );
    let result = (|| {
        let matching_ports = connections.matching_ports(&spec.device_id)?;
        match select_rebind_port(&spec.device_id, &spec.port, &matching_ports, original_error)? {
            None => Ok(false),
            Some(port_name) => {
                if let Some(sink) = rebind_sink {
                    sink.update_serial_device_port(&spec.device_id, &port_name)
                    .map_err(|error| {
                        format!(
                            "found matching serial device on {} but failed to save port rebind: {error}",
                            port_name
                        )
                    })?;
                } else {
                    return Err(format!(
                        "found matching serial device on {} but this runner cannot persist serial port rebinds",
                        port_name
                    ));
                }
                connections.update_port(&spec.device_id, &port_name)?;
                spec.port.clone_from(&port_name);
                set_serial_reader_statuses(
                    reader_statuses,
                    registrations,
                    spec,
                    "port_rebound",
                    None,
                    false,
                );
                Ok(true)
            }
        }
    })();
    let diagnostic = match &result {
        Ok(true) => format!("port changed to {}", spec.port),
        Ok(false) => "no port change was needed".to_owned(),
        Err(error) => format!("failed: {error}"),
    };
    record_serial_rebind_result(reader_statuses, registrations, diagnostic);
    result
}

pub(crate) fn select_rebind_port(
    device_id: &str,
    current_port: &str,
    matching_ports: &[serialport::SerialPortInfo],
    original_error: &str,
) -> Result<Option<String>, String> {
    match matching_ports {
        [] => Err(format!(
            "serial port {current_port} failed ({original_error}) and no matching USB serial device was found for device id {device_id}"
        )),
        [port] if port.port_name == current_port => Ok(None),
        [port] => Ok(Some(port.port_name.clone())),
        ports => {
            let names = ports
                .iter()
                .map(|port| port.port_name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "serial device id {device_id} matched multiple ports ({names}); set serial_number, manufacturer, or product to make auto_rebind_port unambiguous"
            ))
        }
    }
}

fn sleep_serial_reconnect_delay(running: &AtomicBool) {
    let _ = sleep_while_running(running, Duration::from_secs(1));
}

fn sleep_while_running(running: &AtomicBool, duration: Duration) -> bool {
    let mut remaining = duration;
    while !remaining.is_zero() {
        if !running.load(Ordering::Relaxed) {
            return false;
        }
        let step = remaining.min(Duration::from_millis(100));
        thread::sleep(step);
        remaining = remaining.saturating_sub(step);
    }
    running.load(Ordering::Relaxed)
}

fn parse_read_mode(
    registration: &TriggerRegistration,
    value: &str,
) -> Result<SerialReadMode, TriggerError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "idle_gap" => Ok(SerialReadMode::IdleGap),
        "line" => Ok(SerialReadMode::Line),
        "raw" => Ok(SerialReadMode::Raw),
        other => Err(TriggerError::Failed(
            registration.node_id.clone(),
            format!("unsupported serial read mode {other:?}"),
        )),
    }
}
