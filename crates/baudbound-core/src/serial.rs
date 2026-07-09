use std::collections::BTreeMap;

use baudbound_actions::SerialDeviceConfig as ActionSerialDeviceConfig;

use crate::{RunnerConfig, SerialDeviceSettings};

#[must_use]
pub fn action_serial_devices_from_config(config: &RunnerConfig) -> Vec<ActionSerialDeviceConfig> {
    serial_device_configs_from_settings(&config.serial.devices)
        .into_iter()
        .map(|device| ActionSerialDeviceConfig {
            auto_reconnect: device.auto_reconnect,
            auto_rebind_port: device.auto_rebind_port,
            baud_rate: device.baud_rate,
            data_bits: device.data_bits,
            device_id: device.device_id,
            flow_control: device.flow_control,
            manufacturer: device.manufacturer,
            parity: device.parity,
            port: device.port,
            product_id: device.product_id,
            product: device.product,
            read_mode: device.read_mode,
            serial_number: device.serial_number,
            stop_bits: device.stop_bits,
            validate_usb_identity: device.validate_usb_identity,
            vendor_id: device.vendor_id,
        })
        .collect()
}

#[must_use]
pub fn serial_device_configs_from_settings(
    devices: &BTreeMap<String, SerialDeviceSettings>,
) -> Vec<SerialDeviceConfig> {
    devices
        .iter()
        .filter_map(|(device_id, settings)| {
            let device_id = device_id.trim();
            let port = settings.port.trim();
            if device_id.is_empty() || port.is_empty() {
                return None;
            }

            Some(SerialDeviceConfig {
                auto_reconnect: settings.auto_reconnect,
                auto_rebind_port: settings.auto_rebind_port,
                baud_rate: settings.baud_rate,
                data_bits: settings.data_bits,
                device_id: device_id.to_owned(),
                flow_control: settings.flow_control.clone(),
                manufacturer: settings.manufacturer.clone(),
                parity: settings.parity.clone(),
                port: port.to_owned(),
                product_id: settings.product_id.clone(),
                product: settings.product.clone(),
                read_mode: settings.read_mode.clone(),
                serial_number: settings.serial_number.clone(),
                stop_bits: settings.stop_bits.clone(),
                validate_usb_identity: settings.validate_usb_identity,
                vendor_id: settings.vendor_id.clone(),
            })
        })
        .collect()
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
