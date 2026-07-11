---
title: Configuration and Serial Devices
description: Configure targets, listeners, and local serial hardware in config.toml.
tags: [runner, configuration]
---
# Configuration and Serial Devices

The runner always has a TOML configuration file. If none exists, startup atomically creates a secure default. Locate it with `baudbound config path`.

## Choose how to edit the configuration

Desktop users should open **Config** and use **Simple** mode. It provides validated switches, number fields, addresses, ports, and target-runtime selection. Choose **Save** after making changes. Use **Advanced** mode only when a setting is not exposed by the simple form or when editing a headless runner configuration.

The advanced editor contains the complete raw TOML file. TOML section names, keys, quotes, and value types must remain valid. An invalid document is rejected and does not replace the last valid configuration.

Headless users can print the resolved path and a complete example without changing the active file:

```text
baudbound config path
baudbound config print
```

Edit the path printed by `config path` with the text editor used for system administration. Restart a running service after saving because listener addresses and trigger families are initialized from configuration at service startup.

## Main settings

The following shortened example shows the main sections. Keep settings not shown here when editing an existing generated file.

```toml
[runner]
name = "BaudBound Runner"
trigger_reload_seconds = 2
target_runtimes = []

[triggers]
schedules_enabled = true
file_watch_enabled = true
process_watch_enabled = true
serial_enabled = true
startup_enabled = true
webhooks_enabled = false
websockets_enabled = false

[webhooks]
bind = "127.0.0.1"
port = 43891
max_body_bytes = 1048576

[websockets]
bind = "127.0.0.1"
port = 43892
max_message_bytes = 1048576
max_connections = 128
```

`runner.name` is the display name shown in status output. `trigger_reload_seconds` controls how quickly a running service notices imported, updated, enabled, or disabled scripts.

An empty `target_runtimes` uses the runner defaults for the current operating system and whether a desktop session is available. Explicit values restrict the runner further; they cannot make unsupported actions available.

Each value under `[triggers]` enables or disables one listener family. Disabling a family prevents all installed scripts using that trigger from registering it.

Webhook and WebSocket `bind` values of `127.0.0.1` accept connections only from the same machine. Do not change them to `0.0.0.0` until firewall, reverse proxy, authentication, and network exposure have been planned.

The desktop Config tab provides validated simple controls and an advanced TOML editor. Invalid configuration is rejected. The optional restart switch restarts the desktop background runner after a successful save.

Installed-script changes are detected periodically. Changes to `config.toml` require the desktop background runner or headless service to restart.

## Serial devices

Editor serial nodes store only a logical `deviceId`. The runner maps that ID to machine-specific port and protocol settings.

To add a serial device from the desktop application:

1. Connect the physical device.
2. Open **Devices** and choose **Scan**.
3. Find the card matching the expected port, manufacturer, product, vendor ID, and product ID.
4. Choose **Add** and enter the same logical device ID used by the editor node, for example `workbench-scale`.
5. Configure baud rate, data bits, parity, stop bits, flow control, read mode, and automatic reconnect to match the device documentation.
6. Save the configuration and restart the background runner.

Serial Input and Serial Write nodes using the same device ID share that mapping. Device IDs must be unique within one runner configuration.

Enable identity validation to require matching vendor and product IDs. Add a USB serial number when multiple devices share the same model; manufacturer and product strings alone may not distinguish identical hardware.

Operating systems can assign a different COM or tty path after reconnect. `auto_rebind_port` allows the runner to find the configured hardware on another port and save an unambiguous match. It requires identity validation plus vendor and product IDs. The runner refuses ambiguous matches rather than guessing.

After setup, reconnect the device and scan again. Confirm that the configured device still resolves to the intended hardware before enabling unattended serial triggers.
