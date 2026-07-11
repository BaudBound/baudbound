---
title: Desktop UI
description: Operate scripts, services, devices, security, and configuration through the desktop application.
tags: [runner, desktop]
---
# Desktop UI

The desktop application provides the same runner operations as the CLI through task-focused views.

## Dashboard

Shows runner health, installed and enabled script totals, recent runs, attention items, and the state of the desktop background runner.

## Scripts

Imports, updates, enables, disables, runs, removes, and reviews installed packages. Approval review shows requested capabilities before accepting the exact package revision.

## Service and triggers

The Service tab controls the desktop-owned background runner. The Triggers tab reports loaded listener state and allows trigger reloads. The UI does not install or control external system services.

## Security

Shows package approval state, capabilities, risk, and secret requirements.

## Devices

Scans serial ports and adds selected hardware to runner configuration. Identity information helps bind logical device IDs to physical hardware.

## Runs and logs

Runs show execution status, node logs, and variable snapshots. Logs can be filtered by script, node, run, level, or message.

## Config and Doctor

Config offers a simple form and an advanced TOML editor. Saving validates the complete configuration before replacing it. Doctor runs diagnostics for storage, configuration, platform support, desktop integration, and service state.

Closing the main window can leave the desktop background runner active when configured. The tray opens the window or exits the application. An external headless service is managed by the operating system, not by this UI.
