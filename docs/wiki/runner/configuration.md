---
title: Runner Configuration
description: Configure targets, triggers, HTTP listeners, WebSockets, and serial devices in config.toml.
tags: [runner, configuration]
---
# Runner Configuration

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

The desktop Config tab provides validated simple controls and an advanced TOML editor. Advanced edits are parsed and fully validated before an atomic write. The optional restart switch restarts the desktop background runner after a successful save.

Listener configuration is reloaded periodically. Invalid replacement configuration is rejected without replacing the last valid in-memory state. See [Triggers](triggers.md) and [Serial devices](serial-devices.md).
