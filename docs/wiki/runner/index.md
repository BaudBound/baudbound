---
title: BaudBound Runner
description: Execute and operate BaudBound packages on Windows and Linux.
tags: [runner]
---
# BaudBound Runner

The runner validates `.bbs` packages, manages installed scripts, enforces target-runtime and security policy, and executes workflows. The same `baudbound` application provides a desktop UI and CLI.

Launching `baudbound` with no subcommand opens the UI when a supported graphical session is available. In a headless session it prints status and CLI guidance instead of attempting to open a window.

The long-running `baudbound serve` process handles schedules, webhooks, WebSockets, file watchers, process watchers, serial listeners, startup triggers, and trigger reloads. Script administration commands remain available while it is running.

Runner data is stored beneath `BAUDBOUND_HOME`, or the platform application-data directory when that variable is unset. Use `baudbound config path` to locate the active configuration.

## Desktop application

Launching `baudbound` without a command opens the desktop application in a supported graphical session.

**Dashboard** shows runner health, script totals, recent runs, attention items, and the desktop background runner.

**Scripts** imports, updates, enables, disables, runs, removes, and reviews packages. Approval review shows requested capabilities before accepting an exact package revision.

**Service** controls the desktop-owned background runner. **Triggers** reports loaded listeners and can request a reload. The application does not install or control external operating-system services.

**Security** shows approvals, capabilities, risk, and required secrets. **Devices** scans serial ports and creates logical device mappings.

**Runs** and **Logs** expose execution results, node messages, and variable snapshots. **Config** provides validated simple controls and an advanced TOML editor. **Doctor** checks storage, configuration, platform support, desktop integration, and service state.

Closing the main window can leave the desktop background runner active when configured. Use the tray to reopen the window or exit the application completely.
