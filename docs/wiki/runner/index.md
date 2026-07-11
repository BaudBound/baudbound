---
title: Runner
description: Install, manage, and execute BaudBound automation packages.
tags:
  - runner
---
# Runner

The BaudBound runner is one Rust application with desktop and command-line interfaces. It validates packages independently from the editor and stores durable state in SQLite.

The desktop application provides script management, approvals, encrypted secrets, run history, trigger status, serial-device configuration, and diagnostics. The same `baudbound` executable also exposes CLI commands for headless environments.

- [Install the runner](installation.md)
- [Run your first package](quick-start.md)
- [Understand approvals and package integrity](../security/index.md)

On graphical Windows and Linux sessions, launching `baudbound` without a subcommand opens the desktop interface. On headless Linux, the same command prints runner status.
