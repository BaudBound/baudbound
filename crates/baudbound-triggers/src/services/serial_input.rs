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

use serde::Serialize;
use serde_json::json;
use serialport::{
    DataBits, FlowControl, Parity, SerialPortBuilder, SerialPortType, StopBits, available_ports,
};

use crate::{
    SerialPortRebindSink, TriggerError, TriggerEvent, TriggerRegistration,
    TriggerServiceDiagnostics, required_config_string, try_send_trigger_event, unix_timestamp,
    unix_timestamp_millis,
};
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
        sender: SyncSender<TriggerEvent>,
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
pub(crate) struct SerialInputSpec {
    pub(crate) auto_reconnect: bool,
    pub(crate) auto_rebind_port: bool,
    pub(crate) baud_rate: u32,
    pub(crate) data_bits: DataBits,
    pub(crate) device_id: String,
    pub(crate) flow_control: FlowControl,
    pub(crate) manufacturer: Option<String>,
    pub(crate) parity: Parity,
    pub(crate) port: String,
    pub(crate) product_id: Option<u16>,
    pub(crate) product: Option<String>,
    pub(crate) read_mode: SerialReadMode,
    pub(crate) serial_number: Option<String>,
    pub(crate) stop_bits: StopBits,
    pub(crate) validate_usb_identity: bool,
    pub(crate) vendor_id: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SerialReadMode {
    Line,
    Raw,
}

impl SerialInputSpec {
    pub(crate) fn from_registration(
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
    sender: SyncSender<TriggerEvent>,
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
    sender: &SyncSender<TriggerEvent>,
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
    sender: &SyncSender<TriggerEvent>,
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

pub(crate) fn usb_port_matches_identity(
    port: &serialport::SerialPortInfo,
    spec: &SerialInputSpec,
) -> bool {
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
