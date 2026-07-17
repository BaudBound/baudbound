---
title: Desktop App Guide
description: Use every BaudBound desktop tab, background-runner control, security review, and update dialog.
tags: [runner, desktop, interface]
---
# Desktop App Guide

The BaudBound desktop application manages one local runner home through a native Tauri interface. It imports packages, reviews approval, starts or stops the desktop background runner, and displays durable state from the runner database.

The desktop window is a control interface. The background runner is the listener loop that waits for schedules, hotkeys, webhooks, files, processes, and devices. The window can remain open while the background runner is stopped. When that happens, you can still inspect data and change settings, but automatic triggers do not fire.

The interface refreshes automatically while it is open and when the window regains focus. Action results appear as notifications at the top center.

## Navigation overview

| Tab | Use it for |
| --- | --- |
| **Dashboard** | Overall health, counts, review queue, and recent activity |
| **Scripts** | Importing, updating, approving, enabling, running, and removing scripts |
| **Service** | Controlling the desktop-owned background runner and inspecting listeners |
| **Security** | Approval, risk, permissions, package integrity, and secret readiness |
| **Triggers** | Registration state and listener-family diagnostics |
| **Tools** | Screen-coordinate inspection and serial-port discovery |
| **Runs** | Per-run status, logs, results, and variable snapshots |
| **Logs** | Searching recent log messages across runs |
| **Config** | Validated runner and device configuration |
| **Doctor** | Native support, paths, runtime facts, and corrective diagnostics |
| **Settings** | Clock format, desktop login startup, window behavior, background startup, and update checks |

## Dashboard
<!-- desktop-tab:dashboard -->

The summary tiles count installed and enabled scripts, trigger registrations, and active problems. **Runner overview** distinguishes the desktop process from its background automation loop: the window can be open while listeners are stopped.

The **Review queue** highlights scripts with package, approval, secret, compatibility, or health problems. Review the actual issue in Scripts or Security rather than approving solely to clear the count.

**Recent activity** links the latest run records to their script and result. Use Runs for full details.

## Scripts
<!-- desktop-tab:scripts -->

Choose **Import package** to select a `.bbs` file. Import validates the package before modifying runner storage. A rejected package does not replace an installed revision.

Each script row provides frequent actions directly:

- **Run** starts a supported manual trigger only when the script is enabled and its current package revision is approved.
- **Approve** appears when the current revision needs review.
- The row expander shows package identity, health, declared permissions, triggers, and recent runs.
- The action menu contains less frequent operations such as update, enable or disable, revoke approval, secret management, and removal.

Multiple rows can remain expanded. The displayed package name is not the stable script identity. The ID shown with it controls update matching.

Before approval, open the review dialog and confirm target runtime, hash state, nodes, permissions, capabilities, risk, and missing secrets. The approval button remains unavailable when package validation or another blocking condition fails.

Updating requires a package with the same manifest identity. A changed package hash makes the previous approval stale, even when the display name remains unchanged.

Removing a script is destructive. Read [Script Management](script-management.md) for state consequences before confirming it.

## Service
<!-- desktop-tab:service -->

The Service tab controls the background runner owned by this desktop application:

- **Start** creates the long-running listener loop.
- **Reload** stops and restarts the loop so configuration and registrations are rebuilt.
- **Stop** requests an orderly shutdown.

The status badge and timestamp distinguish `running`, `stopping`, `failed`, and `stopped` states. Listener panels show enabled families, registration counts, runtime state, and diagnostics.

By default, closing the window hides it to the system tray and does not quit the application. Left-click the tray icon to restore the window. The tray menu can show the window, start, stop, or reload the background runner, or **Quit BaudBound**. Quit stops the background runner before exiting. You can change close behavior under Settings.

The background runner does not become an operating-system service. It exists only while the BaudBound desktop process remains running. Headless machines should use [Linux Background Service](linux-background-service.md).

## Security
<!-- desktop-tab:security -->

Security summarizes installed scripts, current approvals, high-risk packages, and scripts needing attention.

The review table includes:

- script name and stable identity.
- approval state.
- calculated risk.
- declared permissions.
- package filename and hash prefix.
- the active security issue, if any.

The secret panel lists each script's declarations and whether a value is configured. Setting a secret opens a protected input dialog. Removing it makes dependent runs ineligible until another value is supplied.

Approval does not override malformed packages, unsupported targets, missing required secrets, or runner policy. See [Approvals, Capabilities, and Risk](../security/approvals-capabilities.md).

## Triggers
<!-- desktop-tab:triggers -->

This tab groups registrations by trigger family and reports enabled, registered, active, and failed states. A registration exists only when its script and package are eligible and the listener family is enabled.

Trigger registrations refresh automatically after importing, updating, removing, enabling, disabling, approving, or revoking approval for a script. No manual reload is required. The execution boundary also checks current enablement and approval, so an outdated listener or queued event cannot bypass the latest script state.

A registered trigger is not necessarily receiving events. Inspect listener state, the script's approval and enablement, and family-specific prerequisites. [Background Service and Triggers](service-triggers.md) describes each family.

## Tools
<!-- desktop-tab:tools -->

### Screen coordinates

Choose **Detect monitors** on Windows to inspect the native virtual desktop. The summary shows its total size with separate X and Y coordinate ranges. Each monitor shows its device name, dimensions, separate X and Y ranges, DPI scale, and primary-display state.

Monitors placed to the left of or above the primary display have negative coordinates. The displayed maximum X and Y values are inclusive and can be entered directly into Get Pixel Color or absolute Move Mouse nodes. A coordinate in empty space between monitors is invalid even when it is inside the outer virtual-desktop rectangle.

Choose **Pick coordinate** to select a point directly from the screen. BaudBound opens a transparent selection overlay on every connected monitor. Click the required point or press Escape to cancel. After selection, Tools shows X, Y, the monitor name, and the original pixel color. Copy buttons let you copy either coordinate, the coordinate pair, or the color. Picker results are not saved automatically.

The picker closes its overlays before reading the pixel color. The overlay therefore does not change the returned color. Mixed-DPI layouts use native physical monitor bounds, so the selected coordinate uses the same coordinate system as Get Pixel Color and Move Mouse.

Screen-coordinate discovery and the related input actions are unavailable on Linux until native Linux desktop backends meet the same contract.

### Serial device scanner

Choose **Scan** to enumerate serial ports through the native serial library. Cards display available USB identity fields. Choose **Add**, provide the logical device ID used in editor nodes, and save the generated mapping.

The scanner adds configuration but does not own it. Edit complete serial settings under Config. Inspect active readers beside Serial Input registrations under Triggers. Use Doctor for missing logical device references and invalid configuration.

Enable USB identity validation when vendor and product IDs are available. Auto rebind requires identity validation and refuses ambiguous matches. See [Configuration and Serial Devices](configuration.md).

## Runs
<!-- desktop-tab:runs -->

Runs summarizes completed, failed, cancelled, and error-containing executions. Search and status filters narrow the list without changing stored records.

Expand a run to inspect:

- run and script identity.
- trigger node and timestamps.
- terminal result and error context.
- ordered node log entries with their individual emission times.
- captured variable snapshots.

Use the run ID when correlating an entry with Logs or CLI output. Secret plaintext should not be intentionally stored in logs or variable snapshots. Report a redaction failure as a security issue.

## Logs
<!-- desktop-tab:logs -->

Logs searches recent messages across scripts, nodes, and runs. Each entry includes the time when that message was emitted, level, script, node, message, and run ID. Two messages can have the same displayed time and still retain their original execution order.

The **Clear** button clears the current search text. It does not delete durable run history. Open the related run for ordered context and variable state.

Log levels are `debug`, `info`, `warn`, and `error`. An error message can appear in a run that later records another terminal state, so inspect the full run rather than inferring its result from one line.

## Config
<!-- desktop-tab:config -->

Use **Simple** mode for validated fields and switches. It covers runner identity, target runtimes, reload timing, trigger families, network listeners, and serial devices.

Use **Advanced** mode for the complete raw TOML. The CodeMirror editor supports line numbers, selection, normal keyboard editing, indentation, and scrolling. The runner validates the entire document before replacing the active configuration.

**Reload** discards unsaved edits and rereads the file. **Save** writes only valid configuration. Enable **Restart desktop background runner after saving** when listener, target, or device changes must apply immediately and the runner is currently active.

## Doctor
<!-- desktop-tab:diagnostics -->

Doctor is a diagnostic view, not another configuration editor. It reports:

- runner and package health checks.
- native desktop-action support on the current platform and session.
- resolved runner, configuration, storage, and executable paths.
- operating-system, architecture, desktop-session, and runtime facts.

Treat a failed check as blocking for the related feature. A warning describes a limitation or inactive service that may be intentional. Use the reported path and fact values when requesting support, but remove usernames and never include secrets.

## Settings
<!-- desktop-tab:settings -->

Settings are stored in the runner database and are separate from the runner TOML shown under Config. The page combines settings shared with the CLI and behavior that applies only to the desktop app.

- **Launch at login** registers BaudBound with the current Windows or Linux desktop session. It starts after that user signs in. It does not create a system service or start on a headless machine.
- **Start background runner on launch** starts trigger listeners whenever the desktop application opens.
- **Start login launch in the tray** keeps the main window hidden when the operating system launches BaudBound after login. Opening BaudBound manually still shows the window.
- **Keep running when the window closes** hides the window in the tray. When disabled, closing the window stops the desktop background runner and exits the application.
- **Clock format** changes human-readable desktop and CLI timestamps between 12-hour and 24-hour notation. The same preference can be changed with `baudbound settings set time-format`.
- **Automatically check for updates** checks the signed release feed once after the desktop application starts.

The clock setting changes only how time is displayed. Stored run and log timestamps remain UTC Unix values, and CLI JSON output does not change when the setting changes.

Linux login startup uses the current user's desktop-session autostart directory. It works only after a graphical login. Use [Linux Background Service](linux-background-service.md) when triggers must start before login or on a headless machine.

The login-startup badge reports the operating system registration state. If it says **Registration needs repair**, save the settings again. BaudBound rewrites enabled login entries during startup so an application update or moved AppImage points to the current executable.

## Application updates

When automatic update checks are enabled under Settings, the signed updater checks the configured release endpoint after startup. When a newer compatible release exists, the update dialog displays its version and release notes.

- **Later** closes an available-update dialog without installing.
- **Download** begins a signed download and shows progress.
- **Try again** retries a failed check or download.
- **Restart and install** asks the background runner to stop, installs the verified update, and restarts the application.

Do not terminate the application while installation is replacing files. If download or verification repeatedly fails, use the recovery steps in [Installation and Updates](installation.md).
