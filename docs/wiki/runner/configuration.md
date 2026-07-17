---
title: Configuration and Serial Devices
description: Configure every runner, trigger, network-listener, and serial-device setting in config.toml.
tags: [runner, configuration, serial]
---
# Configuration and Serial Devices

The runner uses one validated TOML configuration. It creates a secure default automatically on first normal startup. Desktop users should prefer the validated Simple editor. Headless operators can edit the same file directly.

TOML is a text format for settings. A heading such as `[triggers]` begins a group. A line such as `hotkeys_enabled = true` assigns one value inside that group. A boolean value is a switch. `true` means enabled and `false` means disabled. Keep spelling, quotation marks, and section names exactly as documented.

Most desktop users do not need to open `config.toml` themselves. The Simple mode in **Config** edits the same settings with fields and switches.

## Path and precedence

Configuration resolution, highest priority first:

1. global CLI option `--config PATH`.
2. `BAUDBOUND_CONFIG` environment variable.
3. `config.toml` inside the resolved `BAUDBOUND_HOME`.
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

## Shared display settings

These settings affect both the desktop app and human readable CLI output.

| Key | Type and default | Meaning |
| --- | --- | --- |
| `display.time_format` | `12-hour` or `24-hour`. Default `24-hour` | Chooses how human readable timestamps are displayed |

This setting does not change stored Unix timestamps or JSON output. You can also change it with `baudbound config set display.time-format 12-hour` or `baudbound config set display.time-format 24-hour`.

## Update settings

Update discovery uses the official BaudBound release feed compiled into the runner. The config cannot redirect checks to an untrusted feed.

| Key | Type and default | Meaning |
| --- | --- | --- |
| `updates.automatic_checks` | boolean. `true` | Allows automatic checks in the desktop app, interactive CLI sessions, and the long running service |
| `updates.check_interval_hours` | positive integer. `24` | Minimum time between successful automatic checks |

The desktop application checks when it starts. Interactive CLI commands check when they start. A running `baudbound serve` process continues checking at the configured interval. Failed checks do not stop scripts or listener services. Run `baudbound update check` when you want to check immediately.

## Desktop application settings

These settings affect only the native Windows and Linux desktop app. They do not change `baudbound serve` on a headless machine.

| Key | Type and default | Meaning |
| --- | --- | --- |
| `desktop.launch_at_login` | boolean. `false` | Opens BaudBound after the current desktop user signs in |
| `desktop.start_background_runner_on_launch` | boolean. `false` | Starts trigger listeners when the desktop app opens |
| `desktop.start_minimized_to_tray` | boolean. `false` | Keeps the window hidden in the system tray when BaudBound starts automatically after login |
| `desktop.keep_running_on_close` | boolean. `true` | Hides the window in the tray instead of exiting when it is closed |

The login setting represents the desired operating system registration. BaudBound reconciles the real registration when the app starts and when Config is saved. Doctor reports a warning if the requested and actual states do not match.

## Runner settings

| Key | Type and default | Meaning | Restart/security impact |
| --- | --- | --- | --- |
| `runner.trigger_reload_seconds` | integer seconds. `2` | Interval for detecting installed-script registration changes | Restart service to change polling interval |
| `runner.run_history_max_records` | positive integer. `10000` | Maximum number of complete run records retained across all scripts | Lower values prune existing history immediately when the config is applied |
| `runner.run_history_max_age_days` | positive integer days. `30` | Maximum age of retained runs | Lower values prune expired history immediately when the config is applied |
| `runner.target_runtimes` | string array. `[]` | Empty uses host defaults. Explicit list restricts accepted package targets | Restart. Cannot grant unsupported targets |

Supported target strings are `Generic Headless`, `Linux Headless`, `Windows Headless`, `Generic Desktop`, `Windows Desktop`, and `Linux Desktop`. A runner accepts only the host-appropriate subset.

## External data limits

These limits prevent one action from loading an unexpectedly large response or file into memory. Every value must be greater than zero.

| Key | Type/default | Meaning |
| --- | --- | --- |
| `limits.max_http_response_bytes` | positive integer. `10485760` | Maximum HTTP response body returned to a workflow |
| `limits.max_file_download_bytes` | positive integer. `104857600` | Maximum file download size before the temporary download is removed |
| `limits.max_file_read_bytes` | positive integer. `10485760` | Maximum regular file size accepted by File Read |

File Download writes to a temporary file beside the destination. BaudBound replaces the destination only after the complete download passes the size limit. A failed oversized download does not leave partial destination data.

## Trigger-family switches

All keys are booleans. Schedule, file, hotkey, process, serial, and startup default to `true`. Network listeners default to `false`.

| Key | Listener family | Notes |
| --- | --- | --- |
| `triggers.schedules_enabled` | Schedule | Timers for enabled, approved scripts |
| `triggers.file_watch_enabled` | File Watch | Native filesystem watchers |
| `triggers.hotkeys_enabled` | Hotkey | Native Windows global keyboard listener |
| `triggers.process_watch_enabled` | App / Process Started | Process snapshot polling |
| `triggers.serial_enabled` | Serial Input | Physical readers from configured mappings |
| `triggers.startup_enabled` | Startup | Dispatches when eligible registrations load at service start |
| `triggers.webhooks_enabled` | Webhook | Opens configured HTTP listener |
| `triggers.websockets_enabled` | WebSocket | Opens configured WebSocket listener |

Disabling a family prevents all scripts in that family from registering. Restart the service after changing these switches.

## Webhook settings

| Key | Type/default | Valid value | Impact |
| --- | --- | --- | --- |
| `webhooks.bind` | string. `127.0.0.1` | Local IP/interface address | Non-loopback can expose routes to a network |
| `webhooks.port` | integer. `43891` | `1-65535` and available | Must not conflict with another process |
| `webhooks.max_body_bytes` | positive integer. `1048576` | Maximum accepted HTTP body | Bounds memory and request size |
| `webhooks.allow_browser_origins` | string array. `[]` | Exact HTTP or HTTPS browser origins | Empty rejects browser-origin requests |
| `webhooks.allow_unauthenticated_public_bind` | boolean. `false` | Permit a public listener while a trigger has auth disabled | Unsafe emergency override |

Keep loopback unless exposure controls are designed. See [Webhooks, WebSockets, and Network Access](network-listeners.md).

## WebSocket settings

| Key | Type/default | Valid value | Impact |
| --- | --- | --- | --- |
| `websockets.bind` | string. `127.0.0.1` | Local IP/interface address | Non-loopback can expose routes |
| `websockets.port` | integer. `43892` | `1-65535` and available | Must not conflict |
| `websockets.max_message_bytes` | positive integer. `1048576` | Maximum text message size | Bounds per-message work |
| `websockets.max_connections` | positive integer. `128` | Concurrent connection limit | Bounds sockets and registry state |
| `websockets.allow_browser_origins` | string array. `[]` | Exact HTTP or HTTPS browser origins | Empty rejects browser-origin handshakes |
| `websockets.allow_unauthenticated_public_bind` | boolean. `false` | Permit a public listener while a trigger has auth disabled | Unsafe emergency override |

Zero size or connection limits are rejected. Browser origins must contain only a scheme, host, and optional port. Wildcards and paths are rejected. A non-loopback listener refuses to start when any matching trigger has authentication disabled unless the unsafe override is explicitly enabled.

## Serial device model

Editor nodes store only a logical `deviceId`, such as `workbench-scale`. The runner maps it to machine-specific hardware under:

```toml
[serial.devices.workbench-scale]
```

This keeps COM and `/dev/tty*` names out of portable packages. Serial Input and Serial Write using the same ID share the mapping.

### Add by scanning

1. Connect the physical device.
2. Open **Tools** and find the serial device scanner.
3. Choose **Scan**.
4. Identify the intended card using its port, vendor ID, product ID, serial number, manufacturer, and product name where available.
5. Choose **Add** and enter the exact logical ID used in editor nodes.
6. Complete the protocol fields using the device manual.
7. Save Config and restart the background runner.
8. Scan again and confirm that the logical ID points to the intended hardware.

### Serial field reference

| Key | Type/default | Supported values and meaning |
| --- | --- | --- |
| `port` | string. Empty | Windows `COM` name or Linux device path. Required for active mapping |
| `baud_rate` | positive integer. `115200` | Device transmission rate from its manual |
| `data_bits` | integer. `8` | `5`, `6`, `7`, or `8` |
| `parity` | string. `none` | `none`, `odd`, or `even` |
| `stop_bits` | string. `1` | `1` or `2` |
| `flow_control` | string. `none` | `none`, `software`, or `hardware` |
| `read_mode` | string. `line` | `line` for newline-delimited text or `raw` for received byte chunks |
| `auto_reconnect` | boolean. `true` | Retry after disconnect instead of leaving reader failed |
| `validate_usb_identity` | boolean. `false` | Require configured USB identity fields to match opened port |
| `auto_rebind_port` | boolean. `false` | Find same unambiguous hardware after OS port-name change and save new port |
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
trigger_reload_seconds = 2
run_history_max_records = 10000
run_history_max_age_days = 30
target_runtimes = []

[display]
time_format = "24-hour"

[limits]
max_http_response_bytes = 10485760
max_file_download_bytes = 104857600
max_file_read_bytes = 10485760

[updates]
automatic_checks = true
check_interval_hours = 24

[desktop]
launch_at_login = false
start_background_runner_on_launch = false
start_minimized_to_tray = false
keep_running_on_close = true

[triggers]
schedules_enabled = true
file_watch_enabled = true
hotkeys_enabled = true
process_watch_enabled = true
serial_enabled = true
startup_enabled = true
webhooks_enabled = false
websockets_enabled = false

[webhooks]
bind = "127.0.0.1"
port = 43891
max_body_bytes = 1048576
allow_browser_origins = []
allow_unauthenticated_public_bind = false

[websockets]
bind = "127.0.0.1"
port = 43892
max_message_bytes = 1048576
max_connections = 128
allow_browser_origins = []
allow_unauthenticated_public_bind = false
```

### Linux headless restriction

```toml
[runner]
trigger_reload_seconds = 2
run_history_max_records = 10000
run_history_max_age_days = 30
target_runtimes = ["Generic Headless", "Linux Headless"]

[display]
time_format = "24-hour"

[limits]
max_http_response_bytes = 10485760
max_file_download_bytes = 104857600
max_file_read_bytes = 10485760

[updates]
automatic_checks = true
check_interval_hours = 24

[desktop]
launch_at_login = false
start_background_runner_on_launch = false
start_minimized_to_tray = false
keep_running_on_close = true

[triggers]
schedules_enabled = true
file_watch_enabled = true
hotkeys_enabled = false
process_watch_enabled = true
serial_enabled = true
startup_enabled = true
webhooks_enabled = false
websockets_enabled = false

[webhooks]
bind = "127.0.0.1"
port = 43891
max_body_bytes = 1048576
allow_browser_origins = []
allow_unauthenticated_public_bind = false

[websockets]
bind = "127.0.0.1"
port = 43892
max_message_bytes = 1048576
max_connections = 128
allow_browser_origins = []
allow_unauthenticated_public_bind = false
```

### Local webhook

Use the desktop defaults, then change only:

```toml
[triggers]
schedules_enabled = true
file_watch_enabled = true
hotkeys_enabled = true
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
