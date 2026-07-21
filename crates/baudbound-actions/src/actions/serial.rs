use std::{
    collections::BTreeMap,
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

use serialport::{
    ClearBuffer, DataBits, FlowControl, Parity, SerialPort, SerialPortBuilder, SerialPortInfo,
    SerialPortType, StopBits, available_ports,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialDeviceConfig {
    pub auto_reconnect: bool,
    pub auto_rebind_port: bool,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub device_id: String,
    pub dtr_on_open: String,
    pub flow_control: String,
    pub manufacturer: Option<String>,
    pub max_message_bytes: usize,
    pub message_gap_ms: u64,
    pub open_stabilization_ms: u64,
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
pub(crate) struct SerialDeviceSpec {
    pub(crate) baud_rate: u32,
    data_bits: DataBits,
    pub(crate) device_id: String,
    dtr_on_open: DtrOnOpen,
    flow_control: FlowControl,
    manufacturer: Option<String>,
    parity: Parity,
    pub(crate) port: String,
    product_id: Option<u16>,
    product: Option<String>,
    serial_number: Option<String>,
    stop_bits: StopBits,
    validate_usb_identity: bool,
    vendor_id: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DtrOnOpen {
    Deasserted,
    Asserted,
    Preserve,
}

impl SerialDeviceSpec {
    pub(crate) fn from_config(config: SerialDeviceConfig) -> Option<Self> {
        let device_id = config.device_id.trim().to_owned();
        let port = config.port.trim().to_owned();
        if device_id.is_empty() || port.is_empty() {
            return None;
        }

        Some(Self {
            baud_rate: config.baud_rate,
            data_bits: parse_data_bits(&config.data_bits.to_string()),
            device_id,
            dtr_on_open: parse_dtr_on_open(&config.dtr_on_open),
            flow_control: parse_flow_control(&config.flow_control),
            manufacturer: normalized_optional_string(config.manufacturer),
            parity: parse_parity(&config.parity),
            port,
            product_id: config.product_id.and_then(|value| parse_usb_hex_id(&value)),
            product: normalized_optional_string(config.product),
            serial_number: normalized_optional_string(config.serial_number),
            stop_bits: parse_stop_bits(&config.stop_bits),
            validate_usb_identity: config.validate_usb_identity,
            vendor_id: config.vendor_id.and_then(|value| parse_usb_hex_id(&value)),
        })
    }
}

struct SerialConnectionState {
    config: SerialDeviceConfig,
    port: Option<Box<dyn SerialPort>>,
    spec: SerialDeviceSpec,
}

pub struct SerialConnectionRegistry {
    devices: BTreeMap<String, Arc<Mutex<SerialConnectionState>>>,
}

impl SerialConnectionRegistry {
    #[must_use]
    pub fn new(devices: impl IntoIterator<Item = SerialDeviceConfig>) -> Self {
        Self {
            devices: devices
                .into_iter()
                .filter_map(|config| {
                    let spec = SerialDeviceSpec::from_config(config.clone())?;
                    Some((
                        spec.device_id.clone(),
                        Arc::new(Mutex::new(SerialConnectionState {
                            config,
                            port: None,
                            spec,
                        })),
                    ))
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn config(&self, device_id: &str) -> Option<SerialDeviceConfig> {
        let state = self.devices.get(device_id)?.lock().ok()?;
        Some(state.config.clone())
    }

    pub fn read(
        &self,
        device_id: &str,
        buffer: &mut [u8],
        timeout: Duration,
    ) -> std::io::Result<usize> {
        let state = self
            .devices
            .get(device_id)
            .ok_or_else(|| std::io::Error::other(format!("unknown serial device {device_id:?}")))?;
        let mut state = state.lock().map_err(|_| {
            std::io::Error::other(format!(
                "serial device {device_id:?} connection lock is poisoned"
            ))
        })?;
        ensure_open(&mut state, timeout).map_err(std::io::Error::other)?;
        let result = state
            .port
            .as_mut()
            .expect("serial port is open")
            .read(buffer);
        if result
            .as_ref()
            .is_err_and(|error| error.kind() != std::io::ErrorKind::TimedOut)
        {
            state.port = None;
        }
        result
    }

    pub fn connect(&self, device_id: &str, timeout: Duration) -> Result<String, String> {
        let state = self
            .devices
            .get(device_id)
            .ok_or_else(|| format!("unknown serial device {device_id:?}"))?;
        let mut state = state
            .lock()
            .map_err(|_| format!("serial device {device_id:?} connection lock is poisoned"))?;
        ensure_open(&mut state, timeout)?;
        Ok(state.spec.port.clone())
    }

    pub fn write(
        &self,
        device_id: &str,
        payload: &[u8],
        timeout: Duration,
    ) -> Result<String, String> {
        let state = self
            .devices
            .get(device_id)
            .ok_or_else(|| format!("unknown serial device {device_id:?}"))?;
        let mut state = state
            .lock()
            .map_err(|_| format!("serial device {device_id:?} connection lock is poisoned"))?;
        ensure_open(&mut state, timeout)?;
        let port_name = state.spec.port.clone();
        let result = {
            let port = state.port.as_mut().expect("serial port is open");
            port.write_all(payload)
                .and_then(|()| port.flush())
                .map_err(|source| source.to_string())
        };
        if result.is_err() {
            state.port = None;
        }
        result.map(|()| port_name)
    }

    pub fn close(&self, device_id: &str) {
        if let Some(state) = self.devices.get(device_id)
            && let Ok(mut state) = state.lock()
        {
            state.port = None;
        }
    }

    pub fn clear_input(&self, device_id: &str) -> Result<(), String> {
        let state = self
            .devices
            .get(device_id)
            .ok_or_else(|| format!("unknown serial device {device_id:?}"))?;
        let state = state
            .lock()
            .map_err(|_| format!("serial device {device_id:?} connection lock is poisoned"))?;
        state
            .port
            .as_ref()
            .ok_or_else(|| format!("serial device {device_id:?} is not connected"))?
            .clear(ClearBuffer::Input)
            .map_err(|source| {
                format!(
                    "failed to clear serial port {} input buffer: {source}",
                    state.spec.port
                )
            })
    }

    pub fn update_port(&self, device_id: &str, port: &str) -> Result<(), String> {
        let state = self
            .devices
            .get(device_id)
            .ok_or_else(|| format!("unknown serial device {device_id:?}"))?;
        let mut state = state
            .lock()
            .map_err(|_| format!("serial device {device_id:?} connection lock is poisoned"))?;
        state.port = None;
        state.config.port = port.to_owned();
        state.spec.port = port.to_owned();
        Ok(())
    }

    pub fn validate_identity(&self, device_id: &str) -> Result<(), String> {
        let state = self
            .devices
            .get(device_id)
            .ok_or_else(|| format!("unknown serial device {device_id:?}"))?
            .lock()
            .map_err(|_| format!("serial device {device_id:?} connection lock is poisoned"))?;
        validate_serial_usb_identity(&state.spec)
    }

    pub fn matching_ports(&self, device_id: &str) -> Result<Vec<SerialPortInfo>, String> {
        let state = self
            .devices
            .get(device_id)
            .ok_or_else(|| format!("unknown serial device {device_id:?}"))?
            .lock()
            .map_err(|_| format!("serial device {device_id:?} connection lock is poisoned"))?;
        if !state.spec.validate_usb_identity {
            return Err("auto_rebind_port requires validate_usb_identity".to_owned());
        }
        if state.spec.vendor_id.is_none() || state.spec.product_id.is_none() {
            return Err("auto_rebind_port requires vendor_id and product_id".to_owned());
        }
        available_ports()
            .map_err(|source| format!("failed to list serial ports for auto rebind: {source}"))
            .map(|ports| {
                ports
                    .into_iter()
                    .filter(|port| usb_port_matches_identity(port, &state.spec))
                    .collect()
            })
    }
}

impl Default for SerialConnectionRegistry {
    fn default() -> Self {
        Self::new([])
    }
}

fn ensure_open(state: &mut SerialConnectionState, timeout: Duration) -> Result<(), String> {
    if let Some(port) = state.port.as_mut() {
        port.set_timeout(timeout).map_err(|source| {
            format!(
                "failed to configure serial port {} timeout: {source}",
                state.spec.port
            )
        })?;
        return Ok(());
    }
    validate_serial_usb_identity(&state.spec)?;
    state.port = Some(
        serial_port_builder(&state.spec, timeout)
            .open()
            .map_err(|source| {
                format!("failed to open serial port {}: {source}", state.spec.port)
            })?,
    );
    Ok(())
}

pub(crate) fn serial_port_builder(
    device: &SerialDeviceSpec,
    timeout: Duration,
) -> SerialPortBuilder {
    let builder = serialport::new(&device.port, device.baud_rate)
        .data_bits(device.data_bits)
        .flow_control(device.flow_control)
        .parity(device.parity)
        .stop_bits(device.stop_bits)
        .timeout(timeout);
    match device.dtr_on_open {
        DtrOnOpen::Deasserted => builder.dtr_on_open(false),
        DtrOnOpen::Asserted => builder.dtr_on_open(true),
        DtrOnOpen::Preserve => builder.preserve_dtr_on_open(),
    }
}

fn parse_dtr_on_open(value: &str) -> DtrOnOpen {
    match value.trim().to_ascii_lowercase().as_str() {
        "asserted" => DtrOnOpen::Asserted,
        "preserve" => DtrOnOpen::Preserve,
        _ => DtrOnOpen::Deasserted,
    }
}

fn validate_serial_usb_identity(device: &SerialDeviceSpec) -> Result<(), String> {
    if !device.validate_usb_identity {
        return Ok(());
    }
    let ports = available_ports().map_err(|source| {
        format!("failed to list serial ports for USB identity validation: {source}")
    })?;
    let port = ports
        .into_iter()
        .find(|port| port.port_name == device.port)
        .ok_or_else(|| {
            format!(
                "serial port {} was not found while validating USB identity",
                device.port
            )
        })?;
    let SerialPortType::UsbPort(info) = port.port_type else {
        return Err(format!(
            "serial port {} is not reported as a USB serial device",
            device.port
        ));
    };
    validate_usb_port_identity_message(device, &info)
}

fn usb_port_matches_identity(port: &SerialPortInfo, device: &SerialDeviceSpec) -> bool {
    let SerialPortType::UsbPort(info) = &port.port_type else {
        return false;
    };
    validate_usb_port_identity_message(device, info).is_ok()
}

fn validate_usb_port_identity_message(
    device: &SerialDeviceSpec,
    info: &serialport::UsbPortInfo,
) -> Result<(), String> {
    if let Some(vendor_id) = device.vendor_id
        && info.vid != vendor_id
    {
        return Err(format!(
            "vendor id mismatch: expected {:04X}, got {:04X}",
            vendor_id, info.vid
        ));
    }
    if let Some(product_id) = device.product_id
        && info.pid != product_id
    {
        return Err(format!(
            "product id mismatch: expected {:04X}, got {:04X}",
            product_id, info.pid
        ));
    }
    if let Some(serial_number) = &device.serial_number
        && optional_string_mismatch(info.serial_number.as_deref(), serial_number)
    {
        return Err(format!("serial number mismatch: expected {serial_number}"));
    }
    if let Some(manufacturer) = &device.manufacturer
        && optional_string_mismatch(info.manufacturer.as_deref(), manufacturer)
    {
        return Err(format!("manufacturer mismatch: expected {manufacturer}"));
    }
    if let Some(product) = &device.product
        && optional_string_mismatch(info.product.as_deref(), product)
    {
        return Err(format!("product mismatch: expected {product}"));
    }
    Ok(())
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

fn optional_string_mismatch(actual: Option<&str>, expected: &str) -> bool {
    actual
        .map(str::trim)
        .is_none_or(|actual| !actual.eq_ignore_ascii_case(expected.trim()))
}

fn normalized_optional_string(value: Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usb_identity_matching_uses_required_and_optional_fields() {
        let spec = SerialDeviceSpec::from_config(SerialDeviceConfig {
            auto_reconnect: true,
            auto_rebind_port: true,
            baud_rate: 9_600,
            data_bits: 8,
            device_id: "controller".to_owned(),
            dtr_on_open: "deasserted".to_owned(),
            flow_control: "none".to_owned(),
            manufacturer: Some(" acme ".to_owned()),
            max_message_bytes: 1024,
            message_gap_ms: 100,
            open_stabilization_ms: 500,
            parity: "none".to_owned(),
            port: "COM3".to_owned(),
            product_id: Some("7523".to_owned()),
            product: Some(" controller ".to_owned()),
            read_mode: "idle_gap".to_owned(),
            serial_number: Some("ABC123".to_owned()),
            stop_bits: "1".to_owned(),
            validate_usb_identity: true,
            vendor_id: Some("1A86".to_owned()),
        })
        .expect("serial device config should produce a spec");
        let matching = serial_port_info("ABC123", "Acme", "Controller");
        let wrong_serial = serial_port_info("XYZ789", "Acme", "Controller");

        assert!(usb_port_matches_identity(&matching, &spec));
        assert!(!usb_port_matches_identity(&wrong_serial, &spec));
    }

    #[test]
    fn usb_identity_matching_rejects_native_and_numeric_mismatches() {
        let spec = SerialDeviceSpec::from_config(SerialDeviceConfig {
            product_id: Some("7523".to_owned()),
            validate_usb_identity: true,
            vendor_id: Some("1A86".to_owned()),
            ..test_config()
        })
        .expect("serial device config should produce a spec");
        let native = SerialPortInfo {
            port_name: "COM1".to_owned(),
            port_type: SerialPortType::Unknown,
        };
        let wrong_vendor = usb_port_info(0x1234, 0x7523);
        let wrong_product = usb_port_info(0x1A86, 0x5678);

        assert!(!usb_port_matches_identity(&native, &spec));
        assert!(!usb_port_matches_identity(&wrong_vendor, &spec));
        assert!(!usb_port_matches_identity(&wrong_product, &spec));
    }

    #[test]
    fn updating_a_port_invalidates_the_connection_and_changes_shared_config() {
        let mut config = test_config();
        config.port = "COM3".to_owned();
        let registry = SerialConnectionRegistry::new([config]);

        registry
            .update_port("controller", "COM7")
            .expect("port should update");

        assert_eq!(
            registry.config("controller").map(|config| config.port),
            Some("COM7".to_owned())
        );
    }

    #[test]
    fn dtr_policy_parses_supported_values_and_defaults_safely() {
        assert_eq!(parse_dtr_on_open("deasserted"), DtrOnOpen::Deasserted);
        assert_eq!(parse_dtr_on_open("asserted"), DtrOnOpen::Asserted);
        assert_eq!(parse_dtr_on_open("preserve"), DtrOnOpen::Preserve);
        assert_eq!(parse_dtr_on_open("unknown"), DtrOnOpen::Deasserted);
    }

    fn test_config() -> SerialDeviceConfig {
        SerialDeviceConfig {
            auto_reconnect: true,
            auto_rebind_port: false,
            baud_rate: 9_600,
            data_bits: 8,
            device_id: "controller".to_owned(),
            dtr_on_open: "deasserted".to_owned(),
            flow_control: "none".to_owned(),
            manufacturer: None,
            max_message_bytes: 1024,
            message_gap_ms: 100,
            open_stabilization_ms: 500,
            parity: "none".to_owned(),
            port: "COM3".to_owned(),
            product_id: None,
            product: None,
            read_mode: "idle_gap".to_owned(),
            serial_number: None,
            stop_bits: "1".to_owned(),
            validate_usb_identity: false,
            vendor_id: None,
        }
    }

    fn serial_port_info(serial_number: &str, manufacturer: &str, product: &str) -> SerialPortInfo {
        SerialPortInfo {
            port_name: "COM7".to_owned(),
            port_type: SerialPortType::UsbPort(serialport::UsbPortInfo {
                vid: 0x1A86,
                pid: 0x7523,
                serial_number: Some(serial_number.to_owned()),
                manufacturer: Some(manufacturer.to_owned()),
                product: Some(product.to_owned()),
            }),
        }
    }

    fn usb_port_info(vid: u16, pid: u16) -> SerialPortInfo {
        SerialPortInfo {
            port_name: "COM7".to_owned(),
            port_type: SerialPortType::UsbPort(serialport::UsbPortInfo {
                vid,
                pid,
                serial_number: None,
                manufacturer: None,
                product: None,
            }),
        }
    }
}
