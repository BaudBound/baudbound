use std::collections::BTreeMap;

pub use baudbound_actions::SerialDeviceConfig;

use crate::{RunnerConfig, SerialDeviceSettings};

#[must_use]
pub fn action_serial_devices_from_config(config: &RunnerConfig) -> Vec<SerialDeviceConfig> {
    serial_device_configs_from_settings(&config.serial.devices)
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
                dtr_on_open: settings.dtr_on_open.clone(),
                flow_control: settings.flow_control.clone(),
                manufacturer: settings.manufacturer.clone(),
                max_message_bytes: settings.max_message_bytes,
                message_gap_ms: settings.message_gap_ms,
                open_stabilization_ms: settings.open_stabilization_ms,
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
