---
title: BaudBound Runner
description: Execute and operate BaudBound packages on Windows and Linux.
tags: [runner]
---
# BaudBound Runner

The runner is a Rust application that validates `.bbs` packages, manages installed scripts, enforces target-runtime and security policy, and executes workflows. The same `baudbound` binary provides a Tauri desktop UI and a complete CLI.

Launching `baudbound` with no subcommand opens the UI when a supported graphical session is available. In a headless session it prints status and CLI guidance instead of attempting to open a window.

The long-running `baudbound serve` process owns schedules, webhooks, WebSockets, file watchers, process watchers, serial listeners, startup triggers, and trigger reloads. Manual script administration remains available from separate CLI processes because durable state is coordinated through SQLite.

Runner data is stored beneath `BAUDBOUND_HOME`, or the platform application-data directory when that variable is unset. Configuration remains human-readable TOML; installed packages, approvals, run records, logs, secrets, and service state use the appropriate durable stores.
