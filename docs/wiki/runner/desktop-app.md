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

| Tab           | Use it for                                                                                                  |
| ------------- | ----------------------------------------------------------------------------------------------------------- |
| **Dashboard** | Overall health, counts, review queue, and recent activity                                                   |
| **Scripts**   | Importing, updating, approving, enabling, running, and removing scripts                                     |
| **Service**   | Controlling the desktop-owned background runner and inspecting listeners                                    |
| **Security**  | Approval, risk, permissions, package integrity, secrets, and network trigger tokens                         |
| **Tools**     | Screen-coordinate inspection and serial-port discovery                                                      |
| **Runs**      | Per-run status, logs, results, and variable snapshots                                                       |
| **Logs**      | Searching recent log messages across runs                                                                   |
| **Monitor**   | Watching live trigger input and execution queue decisions                                                   |
| **Variables** | Inspecting stored persistent and global values and package defaults                                         |
| **Config**    | Shared, runner, and desktop application configuration                                                       |
| **Doctor**    | Native support, registered triggers, serial-reader health, paths, runtime facts, and corrective diagnostics |
| **About**     | Runner version, project links, licensing, credits, update status, and release notes                         |

## Dashboard

<!-- desktop-tab:dashboard -->

The summary tiles count installed and enabled scripts, trigger registrations, and active problems. **Runner overview** distinguishes the desktop process from its background automation loop: the window can be open while listeners are stopped.

The **Review queue** highlights scripts with package, approval, secret, compatibility, or health problems. Review the actual issue in Scripts or Security rather than approving solely to clear the count.

**Recent activity** links the latest run records to their script and result. Use Runs for full details.

## Scripts

<!-- desktop-tab:scripts -->

Choose **Import** to select a `.bbs` file. The file picker is attached to the BaudBound window. After you select a file, the window regains focus and the import begins. Import validates the package before modifying runner storage. A rejected package does not replace an installed revision. If the file picker or import fails, BaudBound displays the reason in a notification.

Importing a package does not create network credentials. When you approve a package that contains Webhook or WebSocket triggers, BaudBound creates the required tokens and opens a one time token dialog. Save every displayed token before continuing. Only token hashes are stored, so a closed token dialog cannot be reopened. Use Security to generate a replacement if a token is lost. Updating a package preserves tokens for unchanged network triggers. Tokens for newly added network triggers appear after you approve the updated package.

Each script row provides frequent actions directly:

- **Run** appears only when the script contains a Manual trigger. It starts the Manual trigger when the script is enabled and its current package revision is approved.
- **Stop** replaces **Run** while that script is running. It requests cancellation for every active run of that script.
- **Approve** appears when the current revision needs review.
- The **View details** button opens package description, author, links, tags, package identity, health, declared permissions, triggers, and recent runs in a large dialog.
- Each recent run in the script details dialog has its own **View details** button. It opens that run's complete logs, result, and variable snapshot without closing the script details.
- The remaining row buttons enable or disable the script and remove it.

The Script Details dialog includes the information supplied by the verified package. Empty optional fields are hidden. Website and source links open in the system browser only after you select them.

The displayed package name is not the stable script identity. The ID shown with it controls update matching.

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

By default, closing the window hides it to the system tray and does not quit the application. Left click the tray icon to restore the window. The tray menu can show the window, start, stop, or reload the background runner, or **Quit BaudBound**. Quit stops the background runner before exiting. You can change close behavior under Config.

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

The network trigger authentication table lists every installed Webhook and WebSocket trigger. It shows whether token protection is enabled, the non-secret token ending, creation time, and last rotation. Generate a replacement token here when a credential is lost or exposed. The new plaintext token appears once.

The secret panel lists each script's declarations and whether a value is configured. Setting a secret opens a protected input dialog. Removing it makes dependent runs ineligible until another value is supplied.

Approval does not override malformed packages, unsupported targets, missing required secrets, or runner policy. See [Approvals, Capabilities, and Risk](../security/approvals-capabilities.md).

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

The scanner adds configuration but does not own it. New mappings use `9600` baud and Idle gap framing. Edit complete serial settings under Config. Native serial settings, BaudBound message framing, and USB identity controls are shown as separate groups. Inspect active readers and Serial Input registrations under Doctor. Use the same page for missing logical device references and invalid configuration.

Enable USB identity validation when vendor and product IDs are available. Auto rebind requires identity validation and refuses ambiguous matches. See [Configuration and Serial Devices](configuration.md).

## Runs

<!-- desktop-tab:runs -->

**Currently running** lists every active execution, including runs started by Manual, Startup, Schedule, Hotkey, File Watch, Process Started, Serial Input, Webhook, WebSocket, and Sub-script paths. Run start, log, cancellation, and finish changes are sent directly from the Rust runner to the desktop window. They do not wait for a dashboard timer. Each entry shows the script, trigger, start time, run ID, and live log messages. Choose **Stop** to request cancellation for one run. A stopping run remains visible until execution has reached a safe cancellation point.

Live messages are filtered through the same secret redaction rules as stored logs. **Follow output** is enabled by default and keeps the newest message in view. Turn it off before scrolling through older messages. The preview keeps the newest 500 messages and reports how many older messages were omitted. A run can have no messages while it is waiting inside an action. The empty message area does not mean the run has stopped.

Below the active list, Runs summarizes completed, failed, cancelled, and error containing executions. The runner refreshes this durable history after SQLite commits each terminal record. Search and status filters query all retained records without changing them. Use the page controls below the table to choose 25, 50, 100, or 200 rows per page.

Select individual runs with the checkbox in each row. The checkbox in the table header selects the current page. After selecting a complete page, **Select all matching runs** selects every record included by the current search and filters. The selection remains active while you move between pages.

Choose **Export selected** to create portable run files. One selected run creates one JSON file. Several selected runs create a ZIP archive with one JSON file per run and a manifest that lists the included run IDs. Each run file contains the complete stored logs, final non secret variable snapshot, variable scopes, trigger details, script details, runner version, platform, and storage schema version. Managed secret values are never included.

Choose **View details** on a run to inspect:

- run and script identity.
- trigger node and timestamps.
- terminal result and error context.
- ordered node log entries with their individual emission times.
- captured variable snapshots.

Use the run ID when correlating an entry with Logs or CLI output. Secret plaintext should not be intentionally stored in logs or variable snapshots. Report a redaction failure as a security issue.

## Logs

<!-- desktop-tab:logs -->

Logs searches all retained messages across scripts, nodes, and runs. Each entry includes the time when that message was emitted, level, script, node, action type, message, run ID, and original position inside its run. Two messages can have the same displayed time and still retain their original execution order. Use the page controls to move through the complete result.

Choose **Export JSON** to save the matching log records with runner and query context. Choose **Export CSV** when the records need to be opened in a spreadsheet or another compatible tool. Both options export all records included by the current search, not only the visible page.

Use **Clear logs** to permanently remove stored messages while preserving run status, identifiers, completion times, and final variables. BaudBound asks for confirmation before deleting them.

Log levels are `debug`, `info`, `warn`, and `error`. An error message can appear in a run that later records another terminal state, so inspect the full run rather than inferring its result from one line.

## Monitor

<!-- desktop-tab:monitor -->

Monitor shows input received by registered triggers while the background runner is active. Use it to confirm what a serial device, webhook, WebSocket client, schedule, file watcher, hotkey, process watcher, startup trigger, or Manual trigger sent to the runner. It does not open devices or network listeners by itself. The related trigger must already be registered and active.

Choose **Start monitoring** before reproducing the event. Monitoring is off by default and remains active when you open another tab. Choose **Stop monitoring** when the input is no longer needed. Monitor data stays in memory only and is removed when BaudBound exits.

Each row shows the event time, script, trigger node, exact trigger type, queue result, and payload preview. Choose **View details** to inspect the complete captured payload. Control characters such as carriage returns and newlines are shown visibly so device framing problems do not remain hidden.

The queue result has two possible values:

1. **Queued** means the runner accepted the event for script execution.
2. **Rejected** means the execution queue did not accept the event. Open the details to read the exact reason.

**Pause view** freezes the visible list while capture continues. **Follow newest** is enabled by default and keeps the latest row visible. Search and filters only change what is displayed. **Clear** removes captured rows from the current monitor session and does not delete runs or logs.

The monitor keeps the latest 500 events. Payloads larger than 64 KiB are shortened in the monitor copy. The trigger still receives its original payload. If the monitor cannot copy an event without waiting, BaudBound reports an omitted monitor event. This does not mean that execution was rejected and it does not affect the script.

BaudBound removes known network authentication headers from monitored payloads. Other trigger data can still contain private information. Inspect monitor data before sharing a screenshot or report.

## Variables

<!-- desktop-tab:variables -->

Variables provides one place to inspect values that exist outside a single completed run. **Stored values** contains current persistent and global values from runner storage. It shows the scope, owning script where applicable, current JSON value, and last update time.

Stored values update through runner events as soon as a successful variable write is committed. The page does not poll the database for changes.

**Declared defaults** contains the default variables from every installed package. It shows the script, scope, declared type, default value, and description. Runtime defaults reset when a run starts. Persistent defaults initialize durable state only when the runner has not stored a value yet.

Choose **Export variables** to save all stored values and declared defaults in one JSON file. The export also includes scopes, types, script ownership, runner information, and declaration warnings. The current search does not limit the export. Managed secret values are never included.

Node outputs and ordinary runtime values exist only inside one execution. Open that execution under Runs to inspect its final snapshot. Secret values are not shown on Variables. Open Security to see whether required secrets are configured without revealing their plaintext.

## Config

<!-- desktop-tab:config -->

Use **Simple** mode for validated fields and switches. The page separates settings by ownership so you can see where each value applies.

**Shared configuration** contains the clock format and update checks used by both the desktop app and CLI.

**Runner configuration** contains target runtimes, reload timing, trigger families, network listeners, and serial devices.

**Desktop configuration** contains login startup, automatic background startup, tray startup, and close behavior. These values affect the graphical app only. They do not change a headless `baudbound serve` process.

Use **Advanced** mode for the complete raw TOML. The CodeMirror editor supports line numbers, selection, normal keyboard editing, indentation, and scrolling. The runner validates the entire document before replacing the active configuration.

**Reload** discards unsaved edits and rereads the file. **Save** writes only valid configuration. **Reset** asks for confirmation and then replaces the configuration file with BaudBound defaults. Reset does not remove installed scripts, approvals, secrets, variables, or run history.

Enable **Restart desktop background runner after saving** when listener, target, or device changes must apply immediately and the runner is currently active. The same choice applies when resetting. Display, update, and desktop window changes apply without restarting the background runner.

**Launch at login** registers BaudBound with the current Windows or Linux desktop session. It starts after that user signs in. It does not create a system service or start on a headless machine.

**Start background runner on launch** starts trigger listeners whenever the desktop application opens.

**Hide window when launched at login** keeps BaudBound in the system tray when it starts automatically after you sign in. Opening BaudBound manually still shows the window.

**Keep running when the window closes** hides the window in the tray. When disabled, closing the window stops the desktop background runner and exits the application.

**Clock format** changes human readable desktop and CLI timestamps between 12 hour and 24 hour notation. The same value can be changed with `baudbound config set display.time-format`.

**Automatically check for updates** uses the signed release feed after the configured check interval has elapsed.

Linux login startup uses the current user's desktop session autostart directory. It works only after a graphical login. Use [Linux Background Service](linux-background-service.md) when triggers must start before login or on a headless machine.

The login startup badge reports the operating system registration state. If it reports a mismatch, save Config again. BaudBound rewrites enabled login entries during startup so the registration points to the current executable and resolved config path. The generated entry starts BaudBound with `--gui --autostart`. This opens the graphical application and lets it apply **Hide window when launched at login**. Normal shortcuts do not need either flag.

## Doctor

<!-- desktop-tab:diagnostics -->

Doctor is a diagnostic view, not another configuration editor. It reports:

- runner and package health checks.
- native desktop-action support on the current platform and session.
- resolved runner, configuration, storage, and executable paths.
- operating-system, architecture, desktop-session, and runtime facts.

**Registered triggers** lists the script, trigger type, node ID, target, and recent dispatch health for every trigger currently loaded by the background runner. Serial Input registrations also show their live reader state, active port, reconnect behavior, last event, and last error.

Registrations refresh automatically after package, approval, enablement, or configuration changes. A missing registration usually means the script is disabled, unapproved, incompatible, invalid, or its trigger family is disabled. Authentication tokens are managed under Security and listener addresses remain under Config.

Treat a failed check as blocking for the related feature. A warning describes a limitation or inactive service that may be intentional. Use the reported path and fact values when requesting support, but remove usernames and never include secrets.

## Application updates

When automatic update checks are enabled under Config, BaudBound checks the official release endpoint after the configured interval has elapsed. When a newer compatible release exists, the update dialog displays its version and release notes. **View latest release** opens the official GitHub Release in every installation type.

- **Later** closes an available-update dialog without installing.
- **Download** begins a signed download and shows progress on Windows and AppImage installations.
- **Try again** repeats the failed check, download, or installation step when it is safe to retry.
- **Restart and install** asks the background runner to stop, installs the verified update, and restarts a Windows or AppImage installation.

Debian and RPM package files are owned by APT or DNF. Those installations never show **Download** or **Restart and install**. They show a copyable `get.baudbound.app` command and the steps required to fully stop BaudBound before updating through the package manager.

Do not terminate the application while installation is replacing files. If a check, download, verification, or installation repeatedly fails, use the matching recovery steps in [Installation and Updates](installation.md).

## About

<!-- desktop-tab:about -->

About identifies the installed BaudBound version and provides links to the website, documentation, source repository, and issue tracker. It also records the project owner, software license, content license, trademark notice, and copyright attribution.

The update panel uses the same signed updater as the automatic update dialog. **Check for updates** contacts the official release feed immediately. It does not install anything by itself. The panel reports whether the installed version is current, whether a newer version is available, or why a check failed.

When an update is available, the panel shows its version and GitHub release notes. Release notes support normal Markdown formatting. Embedded HTML is not executed, and links open through the operating system browser. Windows and AppImage installations can download and install a verified update from this panel. Debian and RPM installations receive package-manager instructions instead.
