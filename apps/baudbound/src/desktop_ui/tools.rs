use serde::Serialize;
use serialport::SerialPortType;

use crate::desktop_actions::screen_tools::{self, MonitorDiscoveryPayload};

#[tauri::command]
pub(super) fn discover_monitors() -> Result<MonitorDiscoveryPayload, String> {
    screen_tools::discover_monitors()
}

#[tauri::command]
pub(super) fn scan_serial_ports() -> Result<Vec<SerialPortScanPayload>, String> {
    serialport::available_ports()
        .map_err(|source| format!("failed to scan serial ports: {source}"))
        .map(|ports| {
            ports
                .into_iter()
                .map(serial_port_scan_payload)
                .collect::<Vec<_>>()
        })
}

#[derive(Serialize)]
pub(super) struct SerialPortScanPayload {
    manufacturer: Option<String>,
    port: String,
    port_type: String,
    product: Option<String>,
    product_id: Option<String>,
    serial_number: Option<String>,
    vendor_id: Option<String>,
}

fn serial_port_scan_payload(port: serialport::SerialPortInfo) -> SerialPortScanPayload {
    match port.port_type {
        SerialPortType::UsbPort(info) => SerialPortScanPayload {
            manufacturer: info.manufacturer,
            port: port.port_name,
            port_type: "usb".to_owned(),
            product: info.product,
            product_id: Some(format!("{:04X}", info.pid)),
            serial_number: info.serial_number,
            vendor_id: Some(format!("{:04X}", info.vid)),
        },
        SerialPortType::BluetoothPort => empty_serial_port_payload(port.port_name, "bluetooth"),
        SerialPortType::PciPort => empty_serial_port_payload(port.port_name, "pci"),
        SerialPortType::Unknown => empty_serial_port_payload(port.port_name, "unknown"),
    }
}

fn empty_serial_port_payload(port: String, port_type: &str) -> SerialPortScanPayload {
    SerialPortScanPayload {
        manufacturer: None,
        port,
        port_type: port_type.to_owned(),
        product: None,
        product_id: None,
        serial_number: None,
        vendor_id: None,
    }
}
