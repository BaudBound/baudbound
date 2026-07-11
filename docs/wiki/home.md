---
title: BaudBound Documentation
description: Complete documentation for the BaudBound visual automation editor and runner.
tags: [overview]
---
# BaudBound

BaudBound is a visual automation platform for building workflows as connected nodes and running them on Windows or Linux. The browser-based editor produces portable `.bbs` packages; the native runner validates, approves, schedules, and executes them.

## Use BaudBound

- [Get started](getting-started/index.md) with the editor and your first runner.
- Learn the [visual editor](editor/index.md), [variables](editor/variables.md), and [simulation](editor/simulation.md).
- Install and operate the [runner](runner/index.md), including its [desktop UI](runner/desktop-ui.md) and [CLI](runner/cli-reference.md).
- Understand [packages](package-format/index.md), [security](security/index.md), and [target runtimes](editor/target-runtimes.md).

## Operate and deploy

- Configure [background services](runner/background-service.md), [triggers](runner/triggers.md), [serial devices](runner/serial-devices.md), and [secrets](runner/secrets.md).
- Deploy the [public editor and schema server](deployment/index.md).
- Diagnose problems with the [troubleshooting guide](runner/troubleshooting.md).

## Build BaudBound

The [developer documentation](developers/index.md) covers architecture, setup, testing, package contracts, documentation publishing, and releases.

BaudBound 2.0 is the second iteration of the project and the first release built around the current package contract and Rust runner architecture.
