import { describe, expect, it } from "vitest";

import type { SerialPortScanResult } from "@/lib/runner-api";
import {
  normalizeSerialDeviceId,
  serialDeviceSettingsFromPort,
  serialPortTypeLabel,
} from "@/views/tools/serial-device-model";

const usbPort: SerialPortScanResult = {
  manufacturer: "BaudBound",
  port: "COM7",
  port_type: "usb",
  product: "Controller",
  product_id: "5678",
  serial_number: "ABC123",
  vendor_id: "1234",
};

describe("serial device scanner model", () => {
  it("normalizes surrounding and repeated whitespace in logical IDs", () => {
    expect(normalizeSerialDeviceId("  main   controller ")).toBe("main_controller");
  });

  it("creates a safe serial config with USB validation when identity is complete", () => {
    expect(serialDeviceSettingsFromPort(usbPort)).toEqual({
      auto_reconnect: true,
      auto_rebind_port: false,
      baud_rate: 9_600,
      data_bits: 8,
      dtr_on_open: "deasserted",
      flow_control: "none",
      manufacturer: "BaudBound",
      max_message_bytes: 1_048_576,
      message_gap_ms: 100,
      open_stabilization_ms: 500,
      parity: "none",
      port: "COM7",
      product: "Controller",
      product_id: "5678",
      read_mode: "idle_gap",
      serial_number: "ABC123",
      stop_bits: "1",
      validate_usb_identity: true,
      vendor_id: "1234",
    });
  });

  it("does not enable USB validation for an incomplete identity", () => {
    const settings = serialDeviceSettingsFromPort({ ...usbPort, product_id: null });
    expect(settings.validate_usb_identity).toBe(false);
  });

  it("uses stable user-facing labels for known and unknown port types", () => {
    expect(serialPortTypeLabel("usb")).toBe("USB");
    expect(serialPortTypeLabel("bluetooth")).toBe("Bluetooth");
    expect(serialPortTypeLabel("pci")).toBe("PCI");
    expect(serialPortTypeLabel("other")).toBe("Unknown");
  });
});
