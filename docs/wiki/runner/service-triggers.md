---
title: Background Service and Triggers
description: Operate long-running BaudBound listeners and understand registration, reload, payload, and failure behavior for every trigger.
tags: [runner, service, triggers]
---
# Background Service and Triggers

Manual runs exit after one execution. Schedules, watchers, network listeners, hotkeys, startup events, and serial readers must remain alive waiting for input, so they require a long-running background runner.

## Choose how to run listeners

| Method | Use | Lifetime |
| --- | --- | --- |
| Desktop **Service** Start | Normal interactive desktop use | Until stopped or desktop application quits |
| `baudbound serve` | Foreground testing and diagnostics | Until `Ctrl+C`, stop IPC, or process failure |
| OS service manager | Headless unattended operation | Supervised according to operator configuration |

Do not start more than one method against the same runner home.

## Registration eligibility

A trigger registers only when:

1. its package is structurally and semantically valid;
2. target runtime is supported by the runner;
3. package hash still matches installed bytes;
4. approval is current;
5. required secrets are configured;
6. script is enabled;
7. its trigger family is enabled in `config.toml`; and
8. family-specific prerequisites can initialize.

The service reports skipped and failed registrations instead of silently claiming they are active.

## Reload boundaries

Installed script import, update, enablement, approval, revocation, and removal create durable state changes and request an immediate trigger reload through SQLite. The running service also checks for changes at `runner.trigger_reload_seconds` and can receive an authenticated loopback IPC reload request.

Reload rebuilds script registrations. Configuration fields are loaded into service options at startup; use Service **Reload** or restart the external process after changing listener addresses, family switches, targets, or serial mappings.

## Manual

- **Prerequisite:** installed, current approval; enablement is not required for an explicit manual command.
- **Registration:** no waiting listener.
- **Payload:** caller-provided JSON or empty input.
- **Start:** Scripts **Run**, `baudbound script run SCRIPT`, or explicit trigger dispatch.
- **Failure:** missing/ambiguous Manual trigger, package problem, policy, target, or secret readiness.

## Schedule

- **Prerequisite:** `schedules_enabled = true`, enabled/current script, positive interval of at least one millisecond.
- **Registration:** one timer per Schedule node.
- **Payload:** interval seconds, configured every/unit, scheduled Unix time, and missed-interval count.
- **Reload:** unchanged registration preserves its next deadline; changed interval resets scheduling from reload time.
- **Duplicate prevention:** when delayed, the service advances past missed intervals and emits one due event with a count rather than replaying every occurrence.
- **Timer accuracy:** millisecond intervals use operating-system timers. They are suitable for short automation intervals, but they are not hard real-time guarantees and may run late while the system is busy.
- **Failure:** invalid duration or duplicate registration identity.

For a first test, use a one-minute schedule and Log action from [Tutorials](../tutorials/index.md).

## File Watch

- **Prerequisite:** `file_watch_enabled = true`; static existing regular file or directory accessible to the account running BaudBound.
- **Registration:** native watcher on the file's parent or selected directory; optional recursive mode.
- **Payload:** changed `path`, original watched path, and normalized event (`created`, `modified`, `deleted`, `renamed`).
- **Reload:** old watcher shuts down before replacement becomes authoritative.
- **Failure:** path missing/inaccessible, runtime template in path, unsupported path kind, or native watcher error.

Applications can produce several OS events for one save. Workflows should be idempotent or tolerate duplicates where practical.

## App / Process Started

- **Prerequisite:** `process_watch_enabled = true`; match mode supported on target.
- **Registration:** polling snapshot for process name, executable path, or Windows Desktop window title.
- **Payload:** process name, PID, executable path, and window title where available.
- **Behavior:** dispatches for newly observed matching processes; it does not repeatedly fire for the same still-running process.
- **Failure:** unsupported window-title mode, inaccessible process metadata, or listener worker failure.

## Startup

- **Prerequisite:** `startup_enabled = true`; enabled/current script.
- **Registration:** eligible Startup nodes are collected when a newly started service loads registrations.
- **Payload:** startup reason and service context.
- **Behavior:** fires once for that service startup/load cycle. It is not an operating-system boot hook by itself; the service manager or desktop application must start BaudBound.
- **Reload:** ordinary script reload does not mean every unchanged Startup trigger should be treated as a machine reboot.

## Hotkey

- **Prerequisite:** Windows Desktop target, active signed-in desktop session, enabled and current script, and an approved package.
- **Registration:** one or more distinct supported keys. Examples include `A`, `F8`, `K+L`, `F1+T`, `Ctrl+Shift+B`, `Numpad7`, and `MediaPlayPause`.
- **Matching:** the complete held-key set must match. `A` does not fire while another key is held, and `K+L` fires when the second required key is pressed regardless of which one was pressed first.
- **Delivery:** completing the physical chord dispatches once. Holding the chord does not dispatch repeated runs, and the keys remain available to the foreground application.
- **Privacy and recursion:** unmatched keys are not stored or logged. Keyboard input generated by software is ignored so an automation cannot trigger itself repeatedly.
- **Payload:** canonical captured `key` and event timestamp.
- **Failure:** invalid or unsupported key, absent desktop session, native hook installation failure, or listener worker failure.

The supported catalog includes letters, digits, `F1` through `F24`, navigation and editing keys, punctuation, numpad keys, and standard browser, volume, media, and launch keys that Windows identifies consistently. Firmware-only keys such as most `Fn` keys are unavailable to normal applications. Windows also reserves secure-attention input such as `Ctrl+Alt+Delete`, so it cannot be used as a BaudBound hotkey.

The desktop background runner installs the native listener automatically and updates it when eligible scripts change. The CLI `baudbound hotkey dispatch` and `baudbound hotkey listen --stdin` commands exercise the same dispatch path using explicit test input. They do not replace the native desktop listener.

## Serial Input

- **Prerequisite:** `serial_enabled = true`; logical device ID mapped to a valid port/protocol; optional identity checks pass.
- **Registration:** reader worker per eligible serial trigger/device.
- **Payload:** logical device ID, text/data, byte count, and timestamp.
- **Reconnect:** `auto_reconnect` retries after disconnect. Auto rebind can save an unambiguous changed port when identity requirements pass.
- **Failure:** missing mapping, open/permission error, identity mismatch, ambiguous rebind, invalid protocol, or worker read error.

Use [Configuration and Serial Devices](configuration.md) for hardware setup.

## Webhook

- **Prerequisite:** `webhooks_enabled = true`; configured bind/port available; network-server policy and approval allow registration.
- **Registration:** exact method plus `/events/HOOK_NAME` route.
- **Payload:** method, path, headers, query, body, parsed JSON, and response state.
- **Response:** immediate fallback or one reachable Webhook Response node with timeout fallback.
- **Failure:** port conflict, duplicate route, body limit, executor capacity, timeout, or response-node failure.

## WebSocket

- **Prerequisite:** `websockets_enabled = true`; bind/port available; network-server policy and approval allow registration.
- **Registration:** exact path beginning with `/`.
- **Payload:** message, route, connection ID, headers/query, and remote address.
- **Connection:** limited by configured message bytes and concurrent count. WebSocket Write replies only through an active known ID.
- **Failure:** route conflict, handshake path missing, limit exceeded, disconnect, send error, or listener failure.

Webhook and WebSocket exposure are documented in [Webhooks, WebSockets, and Network Access](network-listeners.md).

## Inspect and test

Show registrations:

```text
baudbound script triggers
```

Inspect service preflight without opening listeners:

```text
baudbound serve --dry-run
```

Use JSON for automation:

```text
baudbound serve --dry-run --json
```

Dispatch one known trigger with a test payload without waiting for its external source:

```text
baudbound script dispatch-trigger SCRIPT TRIGGER --payload-json '{"test":true}'
```

Replace `SCRIPT` and `TRIGGER` with values shown by inspection. Shell quoting differs; PowerShell accepts the single-quoted JSON example as one argument.

`baudbound serve --once --run-schedules-immediately` is useful for controlled schedule testing: schedules are marked due immediately and the service exits after the first due batch.

## Safe shutdown

In a foreground terminal, press `Ctrl+C` once and wait for final service status. In the desktop application, choose **Stop** or **Quit BaudBound**. For an external service, use its manager's stop command.

Forced termination can interrupt active actions and leave clients without a response. If shutdown does not complete, preserve logs and status before killing the process.
