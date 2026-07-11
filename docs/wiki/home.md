---
title: BaudBound Documentation
description: Complete documentation for the BaudBound visual automation editor and runner.
tags: [overview]
---
# BaudBound

BaudBound is a visual automation platform for building workflows as connected nodes and running them on Windows or Linux. The browser-based editor produces portable `.bbs` packages; the native runner validates, approves, schedules, and executes them.

Start with [Getting Started](getting-started/index.md) to build, test, export, approve, and run a workflow. Use the navigation to find editor, runner, security, deployment, and contributor reference material.

## How it works

1. Build a workflow from triggers, control flow, and actions in the visual editor.
2. Verify and simulate its branches before exporting a `.bbs` package.
3. Import the package into a runner, review its requested access, and approve that exact revision.
4. Run it manually or keep listener-based triggers active through the background service.

## Local execution with explicit trust

The editor never receives the runner machine's production secrets or performs trusted native actions. Packages describe intended behavior and requested capabilities; the runner independently validates those declarations before execution.

Workflows can respond to schedules, webhooks, WebSockets, files, processes, hotkeys, startup, and serial devices. Actions cover data transformation, network requests, files, processes, desktop interaction, and connected devices where the selected target runtime supports them.

Package integrity, per-revision approval, encrypted runner secrets, target-runtime checks, and runner policy keep local automation reviewable instead of treating an exported package as automatically trusted.
