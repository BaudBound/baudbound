---
title: Serial Devices
description: Scan, identify, configure, reconnect, and safely rebind serial hardware.
tags: [runner, serial, devices]
---
# Serial Devices

Editor serial nodes contain only a logical `deviceId`. Port names, protocol settings, reconnect behavior, and USB identity belong to the runner because they are machine-specific.

## Add a device

The Devices tab scans ports with the native serial library and displays available hardware. Select a port, choose Add, and assign a unique logical device ID. The UI writes a validated device entry to `config.toml`.

Each entry can configure port, baud rate, data bits, parity, stop bits, flow control, read mode, and automatic reconnect. Serial Input and Serial Write nodes using the same ID share that runner mapping.

## USB identity

Enable identity validation to require matching vendor and product IDs. A USB serial number is the strongest discriminator when devices share the same model. Manufacturer and product strings provide additional evidence but should not be the sole identity for identical hardware.

## Port rebinding

Operating systems may assign a different COM or tty path after reconnect. `auto_rebind_port` permits the runner to scan other ports for the configured identity and update the stored port after an unambiguous match. It requires identity validation plus vendor and product IDs.

The runner refuses ambiguous matches rather than guessing. For multiple identical devices, configure serial numbers. Automatic reconnect uses bounded retry behavior and reports disconnected, reconnecting, identity-mismatch, ambiguous, or connected status in Devices and service logs.
