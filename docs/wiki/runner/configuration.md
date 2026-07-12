---
title: Configuration and Serial Devices
description: Configure every runner, trigger, network-listener, and serial-device setting in config.toml.
tags: [runner, configuration, serial]
---
# Configuration and Serial Devices

The runner uses one validated TOML configuration. It creates a secure default automatically on first normal startup. Desktop users should prefer the validated Simple editor; headless operators can edit the same file directly.

## Path and precedence

Configuration resolution, highest priority first:

1. global CLI option `--config PATH`;
2. `BAUDBOUND_CONFIG` environment variable;
3. `config.toml` inside the resolved `BAUDBOUND_HOME`; and
4. the platform-default runner home.

Print the exact path without guessing:

```text
baudbound config path
```

`BAUDBOUND_CONFIG` moves only the TOML file. `BAUDBOUND_HOME` moves configuration, database, and installed package storage unless a separate config path overrides it.

## Edit safely

### Desktop

Open **Config** and use **Simple** mode. Save is disabled until values pass UI validation. Enable **Restart desktop background runner after saving** when an active listener should immediately rebuild.

Use **Advanced** only when you need the raw document. Invalid TOML is rejected and does not replace the last valid configuration. **Reload** discards unsaved edits.

### Headless

Print a complete current template:

```text
baudbound config print
```

Edit the path from `baudbound config path` under the account that runs BaudBound. Restart the background service after changes. `baudbound config init` exists for explicit provisioning, but normal startup already initializes a missing file.

## Runner settings

| Key | Type and default | Meaning | Restart/security impact |
| --- | --- | --- | --- |
| `runner.name` | optional string; `BaudBound Runner` when blank | Display name in status and UI | No privilege change; reload UI/status |
| `runner.trigger_reload_seconds` | integer seconds; `2` | Interval for detecting installed-script registration changes | Restart service to change polling interval |
| `runner.target_runtimes` | string array; `[]` | Empty uses host defaults; explicit list restricts accepted package targets | Restart; cannot grant unsupported targets |

Supported target strings are `Generic Headless`, `Linux Headless`, `Windows Headless`, `Generic Desktop`, `Windows Desktop`, and `Linux Desktop`. A runner accepts only the host-appropriate subset.

## Trigger-family switches

All keys are booleans. Schedule, file, process, serial, and startup default to `true`; network listeners default to `false`.

| Key | Listener family | Notes |
| --- | --- | --- |
| `triggers.schedules_enabled` | Schedule | Timers for enabled, approved scripts |
| `triggers.file_watch_enabled` | File Watch | Native filesystem watchers |
| `triggers.process_watch_enabled` | App / Process Started | Process snapshot polling |
| `triggers.serial_enabled` | Serial Input | Physical readers from configured mappings |
| `triggers.startup_enabled` | Startup | Dispatches when eligible registrations load at service start |
| `triggers.webhooks_enabled` | Webhook | Opens configured HTTP listener |
| `triggers.websockets_enabled` | WebSocket | Opens configured WebSocket listener |

Disabling a family prevents all scripts in that family from registering. Restart the service after changing these switches.

## Webhook settings

| Key | Type/default | Valid value | Impact |
| --- | --- | --- | --- |
| `webhooks.bind` | string; `127.0.0.1` | Local IP/interface address | Non-loopback can expose routes to a network |
| `webhooks.port` | integer; `43891` | `1-65535` and available | Must not conflict with another process |
| `webhooks.max_body_bytes` | positive integer; `1048576` | Maximum accepted HTTP body | Bounds memory and request size |

Keep loopback unless exposure controls are designed. See [Webhooks, WebSockets, and Network Access](network-listeners.md).

## WebSocket settings

| Key | Type/default | Valid value | Impact |
| --- | --- | --- | --- |
| `websockets.bind` | string; `127.0.0.1` | Local IP/interface address | Non-loopback can expose routes |
| `websockets.port` | integer; `43892` | `1-65535` and available | Must not conflict |
| `websockets.max_message_bytes` | positive integer; `1048576` | Maximum text message size | Bounds per-message work |
| `websockets.max_connections` | positive integer; `128` | Concurrent connection limit | Bounds sockets and registry state |

Zero message size or connection count is rejected.

## Serial device model

Editor nodes store only a logical `deviceId`, such as `workbench-scale`. The runner maps it to machine-specific hardware under:

```toml
[serial.devices.workbench-scale]
```

This keeps COM and `/dev/tty*` names out of portable packages. Serial Input and Serial Write using the same ID share the mapping.

### Add by scanning

1. Connect the physical device.
2. Open **Devices** and choose **Scan**.
3. Identify the intended card using port, vendor/product IDs, serial number, manufacturer, and product.
4. Choose **Add** and enter the exact logical ID used in editor nodes.
5. Complete protocol fields from the hardware manual.
6. Save Config and restart the background runner.
7. Scan again and confirm the configured mapping resolves to the intended hardware.

### Serial field reference

| Key | Type/default | Supported values and meaning |
| --- | --- | --- |
| `port` | string; empty | Windows `COM` name or Linux device path; required for active mapping |
| `baud_rate` | positive integer; `115200` | Device transmission rate from its manual |
| `data_bits` | integer; `8` | `5`, `6`, `7`, or `8` |
| `parity` | string; `none` | `none`, `odd`, or `even` |
| `stop_bits` | string; `1` | `1` or `2` |
| `flow_control` | string; `none` | `none`, `software`, or `hardware` |
| `read_mode` | string; `line` | `line` for newline-delimited text or `raw` for received byte chunks |
| `auto_reconnect` | boolean; `true` | Retry after disconnect instead of leaving reader failed |
| `validate_usb_identity` | boolean; `false` | Require configured USB identity fields to match opened port |
| `auto_rebind_port` | boolean; `false` | Find same unambiguous hardware after OS port-name change and save new port |
| `vendor_id` | optional string | USB hexadecimal vendor ID |
| `product_id` | optional string | USB hexadecimal product ID |
| `serial_number` | optional string | Strong discriminator between identical models when available |
| `manufacturer` | optional string | Additional expected descriptor |
| `product` | optional string | Additional expected descriptor |

Auto rebind requires `validate_usb_identity = true` plus nonblank vendor and product IDs. Invalid relationships are rejected when the configuration loads.

### Identity and port changes

Operating systems can assign another COM or tty name after reconnect. With auto rebind enabled, BaudBound scans for configured identity and updates `port` only when one unambiguous match exists. It refuses zero or multiple matches instead of guessing.

Vendor and product IDs identify a model, not always one physical unit. Add serial number whenever multiple identical devices may be connected.

### Linux permissions

The runner account needs read/write access to the device node. Distribution rules differ: the owning group may be `dialout`, `uucp`, or another udev-defined group.

Inspect the actual device:

```bash
ls -l /dev/ttyUSB0
```

Add the account running BaudBound only to the group shown by the target system's policy, then restart its login or service session. Do not make serial devices world-writable.

## Complete examples {.tabset}

### Desktop defaults

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

### Linux headless restriction

```toml
[runner]
name = "Automation Server"
trigger_reload_seconds = 2
target_runtimes = ["Generic Headless", "Linux Headless"]

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

### Local webhook

Use the desktop defaults, then change only:

```toml
[triggers]
schedules_enabled = true
file_watch_enabled = true
process_watch_enabled = true
serial_enabled = true
startup_enabled = true
webhooks_enabled = true
websockets_enabled = false
```

### Serial device

```toml
[serial.devices.workbench-scale]
port = "COM3"
baud_rate = 115200
data_bits = 8
parity = "none"
stop_bits = "1"
flow_control = "none"
read_mode = "line"
auto_reconnect = true
validate_usb_identity = true
auto_rebind_port = true
vendor_id = "1A86"
product_id = "7523"
serial_number = "DEVICE-SERIAL"
manufacturer = ""
product = ""
```

Replace protocol and identity values with those reported for the real device. On Linux, replace `COM3` with its actual device path.

## Validation and failure behavior

Configuration is parsed before the runner opens storage or starts listeners. A syntax, type, range, or relationship error stops startup and includes the resolved path and cause. The desktop Save action validates before replacing the file.

Installed-script changes can reload without a full service restart. Listener addresses, enabled families, target restrictions, and serial mappings are constructed from configuration and require restart/reload of the background runner.
