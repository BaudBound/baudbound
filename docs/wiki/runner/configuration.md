---
title: Configuration and Serial Devices
description: Configure targets, listeners, and local serial hardware in config.toml.
tags: [runner, configuration]
---
# Configuration and Serial Devices

The runner always has a TOML configuration file. If none exists, startup atomically creates a secure default. Locate it with `baudbound config path`.

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

An empty `target_runtimes` uses the runner defaults for the current OS and session. Explicit values restrict packages further. Binding network listeners to a non-loopback address exposes them to the network and should be paired with host firewall and reverse-proxy policy.

The desktop Config tab provides validated simple controls and an advanced TOML editor. Invalid configuration is rejected. The optional restart switch restarts the desktop background runner after a successful save.

Listener configuration is reloaded periodically.

## Serial devices

Editor serial nodes store only a logical `deviceId`. The runner maps that ID to machine-specific port and protocol settings.

The Devices view scans available ports. Select a port, choose Add, and assign a unique device ID. Each entry can configure port, baud rate, data bits, parity, stop bits, flow control, read mode, and automatic reconnect. Serial Input and Serial Write nodes using the same ID share that mapping.

Enable identity validation to require matching vendor and product IDs. Add a USB serial number when multiple devices share the same model; manufacturer and product strings alone may not distinguish identical hardware.

Operating systems can assign a different COM or tty path after reconnect. `auto_rebind_port` allows the runner to find the configured hardware on another port and save an unambiguous match. It requires identity validation plus vendor and product IDs. The runner refuses ambiguous matches rather than guessing.
