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
