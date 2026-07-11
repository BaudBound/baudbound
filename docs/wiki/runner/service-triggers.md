---
title: Background Service and Triggers
description: Run BaudBound continuously and operate listener-based triggers.
tags: [runner, service]
---
# Background Service and Triggers

`baudbound serve` is the long-lived runtime for listener-based triggers. It loads enabled and approved scripts, starts configured listener families, records health, and reloads script changes.

## Desktop background runner

The desktop app can own a background runner process for the current user session. It may continue after the main window closes and is controlled through the Service tab and tray. This is separate from an operating-system service.

## Headless operation

Run `baudbound serve` under the process supervisor used by the system. BaudBound does not install or manage systemd units. The [Linux Background Service](../self-hosting/linux-background-service.md) guide provides complete systemd, OpenRC, and runit configurations with a dedicated account, persistent `BAUDBOUND_HOME`, secret-key environment, restart policy, and service logs.

Use graceful termination so the runner can stop listeners and finish durable state updates. Importing, updating, enabling, or disabling scripts from another CLI process does not require a manual restart: the service detects durable state changes at the configured reload interval. A trigger reload can also be requested from the UI.

Do not run both the desktop-owned runner and an external service against the same `BAUDBOUND_HOME` unless the deployment explicitly supports that topology. One active service owner avoids duplicate trigger delivery and port conflicts.

## Trigger behavior

Manual triggers run on demand. The following triggers require the background runner:

**Schedule** starts at calculated due times and avoids redispatching already recorded occurrences after reloads.

**File Watch** observes configured paths and provides normalized filesystem events.

**Process Started** starts when a matching process appears. Window-title matching is available only on supported Windows Desktop targets.

**Webhook** accepts bounded HTTP requests. It may respond immediately or wait for a Webhook Response node.

**WebSocket** starts from inbound messages. WebSocket Write replies through the connection associated with that run.

**Hotkey** uses desktop key registration and requires an interactive supported session.

**Serial Input** reads the logical device selected by its `deviceId`. Physical port and protocol settings are runner configuration.

**Startup** runs when the service loads an enabled script, not on every reload check.

Listener failures appear in service status and logs. One invalid script does not stop unrelated listeners.
