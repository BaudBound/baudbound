---
title: Background Service
description: Run BaudBound continuously on desktops and headless systems.
tags: [runner, service]
---
# Background Service

`baudbound serve` is the long-lived runtime for listener-based triggers. It loads enabled and approved scripts, starts configured listener families, records health, and reloads script changes.

## Desktop background runner

The desktop app can own a background runner process for the current user session. It may continue after the main window closes and is controlled through the Service tab and tray. This is separate from an operating-system service.

## Headless operation

Run `baudbound serve` under the process supervisor used by the system. BaudBound does not install or manage systemd units. Administrators should create their own systemd, OpenRC, runit, container, or other supervisor configuration with an explicit user, working directory, `BAUDBOUND_HOME`, secret-key environment, restart policy, and logs.

Use graceful termination so the runner can stop listeners and finish durable state updates. Importing, updating, enabling, or disabling scripts from another CLI process does not require a manual restart: the service detects durable state changes at the configured reload interval. A trigger reload can also be requested from the UI.

Do not run both the desktop-owned runner and an external service against the same `BAUDBOUND_HOME` unless the deployment explicitly supports that topology. One active service owner avoids duplicate trigger delivery and port conflicts.
