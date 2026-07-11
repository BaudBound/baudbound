---
title: Trigger Services
description: Operate schedules, files, processes, webhooks, WebSockets, hotkeys, startup, and serial listeners.
tags: [runner, triggers]
---
# Trigger Services

Manual triggers run on demand. Other trigger types need `baudbound serve` or the desktop background runner.

**Schedule** calculates due times and persists dispatch state so reloads do not duplicate already-recorded occurrences. Immediate schedule mode is intended for controlled testing.

**File Watch** monitors configured paths using native filesystem notifications and emits normalized event data.

**Process Started** polls native process information and dispatches on newly matching processes. Window-title matching is restricted to supported Windows Desktop targets.

**Webhook** accepts HTTP requests on the configured listener. A trigger may respond immediately or wait for a Webhook Response node. Request bodies are bounded by `max_body_bytes`; every waiting request has a finite lifecycle.

**WebSocket** accepts bounded client connections and dispatches inbound messages. WebSocket Write sends through the connection associated with that run; there is no arbitrary user-defined socket name.

**Hotkey** uses native desktop registration and requires an interactive supported desktop session.

**Serial Input** reads the logical device configured by the trigger's `deviceId`. Physical port and protocol settings belong to runner configuration.

**Startup** dispatches when the service successfully loads the enabled script. Reload behavior is guarded so ordinary polling does not repeatedly fire startup triggers.

Listener failures are isolated and reported through service health and logs. One invalid script must not terminate unrelated listeners.
