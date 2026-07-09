use std::time::Duration;

use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest};
use serialport::{
    DataBits, FlowControl, Parity, SerialPortBuilder, SerialPortType, StopBits, available_ports,
};

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
pub(crate) struct SerialDeviceSpec {
    pub(crate) baud_rate: u32,
    data_bits: DataBits,
    pub(crate) device_id: String,
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

pub(crate) fn serial_port_builder(
    device: &SerialDeviceSpec,
    timeout: Duration,
) -> SerialPortBuilder {
    serialport::new(&device.port, device.baud_rate)
        .data_bits(device.data_bits)
        .flow_control(device.flow_control)
        .parity(device.parity)
        .stop_bits(device.stop_bits)
        .timeout(timeout)
}

pub(crate) fn validate_usb_identity(
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
        return crate::failed(
            request,
            format!(
                "serial port {} was not found while validating USB identity",
                device.port
            ),
        );
    };
    let SerialPortType::UsbPort(info) = port.port_type else {
        return crate::failed(
            request,
            format!(
                "serial port {} is not reported as a USB serial device",
                device.port
            ),
        );
    };

    validate_usb_port_identity(request, device, &info)
}

fn validate_usb_port_identity(
    request: &RuntimeActionRequest,
    device: &SerialDeviceSpec,
    info: &serialport::UsbPortInfo,
) -> Result<(), RuntimeActionError> {
    if let Some(vendor_id) = device.vendor_id
        && info.vid != vendor_id
    {
        return crate::failed(
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
        return crate::failed(
            request,
            format!(
                "serial port {} product id mismatch: expected {:04X}, got {:04X}",
                device.port, product_id, info.pid
            ),
        );
    }
    if let Some(serial_number) = &device.serial_number
        && optional_string_mismatch(info.serial_number.as_deref(), serial_number)
    {
        return crate::failed(
            request,
            format!(
                "serial port {} serial number mismatch: expected {serial_number}",
                device.port
            ),
        );
    }
    if let Some(manufacturer) = &device.manufacturer
        && optional_string_mismatch(info.manufacturer.as_deref(), manufacturer)
    {
        return crate::failed(
            request,
            format!(
                "serial port {} manufacturer mismatch: expected {manufacturer}",
                device.port
            ),
        );
    }
    if let Some(product) = &device.product
        && optional_string_mismatch(info.product.as_deref(), product)
    {
        return crate::failed(
            request,
            format!(
                "serial port {} product mismatch: expected {product}",
                device.port
            ),
        );
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
