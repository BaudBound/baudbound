import type { SerialDeviceSettings, SerialPortScanResult } from "@/lib/runner-api";

export function normalizeSerialDeviceId(value: string) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function serialDeviceSettingsFromPort(
  port: SerialPortScanResult,
): SerialDeviceSettings {
  const hasUsbIdentity = Boolean(port.vendor_id && port.product_id);
  return {
    auto_reconnect: true,
    auto_rebind_port: false,
    baud_rate: 9_600,
    data_bits: 8,
    dtr_on_open: "deasserted",
    flow_control: "none",
    manufacturer: port.manufacturer,
    max_message_bytes: 1_048_576,
    message_gap_ms: 100,
    open_stabilization_ms: 500,
    parity: "none",
    port: port.port,
    product: port.product,
    product_id: port.product_id,
    read_mode: "idle_gap",
    serial_number: port.serial_number,
    stop_bits: "1",
    validate_usb_identity: hasUsbIdentity,
    vendor_id: port.vendor_id,
  };
}

export function serialPortTypeLabel(value: string) {
  if (value === "usb") return "USB";
  if (value === "bluetooth") return "Bluetooth";
  if (value === "pci") return "PCI";
  return "Unknown";
}
