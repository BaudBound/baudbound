---
title: BaudBound Runner
description: Choose desktop, CLI, or headless operation and understand the runner's responsibilities.
tags: [runner, overview]
---
# BaudBound Runner

The native BaudBound runner validates `.bbs` packages, installs scripts, recalculates security requirements, records per-revision approval, executes graphs, owns long-running triggers, and stores durable state. Windows and Linux are supported.

## Choose an operating mode

| Mode | Start it with | Best for | Long-running triggers |
| --- | --- | --- | --- |
| **Desktop application** | Launch BaudBound or run `baudbound --gui` | Interactive Windows/Linux users | Start and supervise from Service or tray |
| **CLI command** | `baudbound COMMAND` | Scripting, inspection, package lifecycle, and one manual run | Command exits after its task |
| **Foreground service** | `baudbound serve` | Testing listeners or external service-manager supervision | Active while process remains running |

Running `baudbound` without a subcommand opens the desktop UI when a desktop session is available. In a headless session it prints status instead of trying to open a window. Use `baudbound --gui` to request the desktop UI explicitly.

## One runner home

One runner home contains one logical installation's configuration, SQLite state, and installed package copies. The default is under the current user's platform data directory. `BAUDBOUND_HOME` can select a dedicated location.

Desktop and CLI commands use the same state when launched by the same account and environment. The recommended Linux background-service setup also runs under that account, so normal `baudbound` commands manage the same runner home.

Do not run multiple services against one home. Database coordination does not prevent duplicate listeners from competing for network ports, files, desktop hooks, or serial devices.

See [Storage, Backups, and Recovery](storage-backups.md) for exact paths and backup rules.

## What happens during import and execution

1. The package archive, paths, documents, schema-shaped data, and semantic graph are validated.
2. Permissions, capabilities, risk, and target compatibility are recalculated independently from executable program data.
3. The package is copied under the runner home and indexed in SQLite.
4. The operator reviews and approves the exact installed hash.
5. Required secret values and runtime policy are checked before a run.
6. Trigger data enters the graph runtime. This data can contain information such as a webhook body, a changed file path, a serial message, or a pressed hotkey.
7. Each selected output follows its connections sequentially in the explicit package execution order.
8. Supported native adapters perform side effects.
9. Redacted logs, variables, status, and result are stored as a run record.

Fan-out connections are not parallel work. The runtime completes the first destination branch before starting the next destination from the same output. This matches editor simulation. Node IDs and canvas positions do not decide the order.

BaudBound runs only one instance of an installed script at a time. When another trigger starts the same script while it is already running, the new run waits in the trigger queue. Runs for different installed scripts can use separate workers and execute at the same time. This keeps persistent variables predictable inside one script without preventing independent automations from running together.

Changing installed package bytes or importing an updated revision makes approval invalid or stale.

## Desktop background runner

The desktop application includes a background-runner supervisor and system tray. Starting it activates eligible schedules, watchers, webhooks, WebSockets, hotkeys, startup triggers, and serial readers.

By default, closing the window hides it to the tray. Desktop Settings can launch BaudBound after login, start the background runner with the app, change close behavior, and keep an automatic login launch hidden. Choosing **Quit BaudBound** stops the background runner and exits. This is desktop-session startup, not an operating-system service, and it does not remain alive after the desktop process exits.

Use [Desktop App Guide](desktop-app.md) for every tab and control.

## Headless operation

`baudbound serve` stays in the foreground and handles signals and authenticated local control requests. On a server, the operator supplies a service-manager definition and environment rather than asking the desktop UI to manage an external service.

Follow [Linux Background Service](linux-background-service.md) for systemd, OpenRC, and runit examples.

## Continue by task

| Task | Page |
| --- | --- |
| Install or update | [Installation and Updates](installation.md) |
| Use the desktop interface | [Desktop App Guide](desktop-app.md) |
| Import, approve, enable, or update a script | [Script Management](script-management.md) |
| Use terminal commands | [CLI Reference](cli-reference.md) |
| Configure listeners and serial devices | [Configuration and Serial Devices](configuration.md) |
| Operate waiting triggers | [Background Service and Triggers](service-triggers.md) |
| Protect and configure secrets | [Secrets](secrets.md) |
| Diagnose a run | [Runs, Logs, and Troubleshooting](runs-logs-troubleshooting.md) |
