---
title: Runs, Logs, and Troubleshooting
description: Inspect execution history, correlate logs, and diagnose package, approval, trigger, device, network, or update failures.
tags: [runner, storage, logs, troubleshooting]
---
# Runs, Logs, and Troubleshooting

Use this page after an import, run, or listener behaves differently from what you expected. Start with the recorded error and move outward through script state, trigger state, configuration, and platform support. Do not edit SQLite or installed package files to make an error disappear.

## What a run records

Every terminal run record contains:

- a run ID and stable script ID.
- the trigger node ID.
- completion timestamp.
- status: `completed`, `failed`, or `cancelled`. A completed run that emitted error logs also displays a separate `with errors` badge.
- structured log entries with their own emission timestamp, level, message, and optional node ID.
- the final non-secret variable snapshot available for inspection.

`completed` means graph execution reached a normal terminal state. A fallible action can take its failure branch and still produce a completed run when the graph handles that failure. `failed` means execution could not continue. `cancelled` means a stop request interrupted runtime execution.

Secret plaintext is not intended to appear in logs or stored snapshots. If it does, stop using the affected script, preserve a redacted reproduction, rotate the value, and report a security issue.

## Run-history retention

BaudBound bounds run history by both count and age. The default keeps at most 10,000 runs and removes records older than 30 days. Each run record contains its logs and final variable snapshot, so those historical details are removed together.

Configure the limits under `[runner]`:

```toml
[runner]
run_history_max_records = 10000
run_history_max_age_days = 30
```

Both values must be greater than zero. The runner applies both limits, so a record is removed when it exceeds either one. Lowering a limit prunes existing history as soon as the new configuration is applied. New runs are inserted and old runs are pruned in one SQLite transaction.

Retention never deletes installed scripts, approvals, persistent variables, global variables, or encrypted secrets. Increase the limits before an investigation when you need a longer history window.

## Inspect in the desktop app

1. Open **Scripts** and choose **View details** for the affected script. Confirm package hash, compatibility, enablement, approval, and required-secret state.
2. Open **Monitor** and choose **Start monitoring** when an automatic trigger does not appear to work. Reproduce the event and check whether it was queued or rejected. A missing event means the trigger did not reach the execution queue. A rejected event includes the reason.
3. Open **Runs** and check **Currently running** when the script has not finished. The live entry shows which trigger started it and displays new log messages as they are emitted.
4. Choose **Stop** on an active entry when the run must end. Cancellation is cooperative, so the entry remains visible until the current action reaches a safe point.
5. For a finished run, select the relevant script and choose the newest failed or cancelled record.
6. Copy the run ID, trigger ID, status, and first useful error.
7. Inspect its node logs and final variables. Each log time records when that entry was emitted, not when the run finished. A node ID connects a runtime failure to the editor graph.
8. Open **Doctor** when the event is missing or a configured serial reader is disconnected. It lists registered triggers and live serial readers. Use **Tools** to scan the serial ports currently detected by the machine. Open **Service** for listener state.

Monitor is a temporary live view. It keeps the latest 500 events in memory and does not replace Runs or Logs. A monitor omission warning means the UI copy could not be captured without slowing execution. The script event was not rejected. A row marked **Rejected** is different because it means the real execution queue did not accept the event.

The **Logs** tab searches messages across all retained runs. Use a run ID to avoid mixing errors from two overlapping executions. Results are paginated, so a search is applied before the page is selected.

For a support case, select the relevant records in Runs and choose **Export selected**. A single run produces one JSON file. Multiple runs produce a ZIP archive with one JSON file for every run and a manifest. The files include complete stored logs, final variable snapshots, scopes, script identity, trigger identity, timestamps, runner version, platform, and storage schema version. Managed secret values are excluded.

Use **Export JSON** or **Export CSV** in Logs when the investigation needs messages from many runs. The current search controls which records are exported. Export includes all matching messages rather than only the visible page.

Open **Variables** to inspect current persistent and global values together with defaults declared by installed packages. Choose **Export variables** to save the complete variable inventory as JSON. The export includes stored values, declared defaults, scopes, types, script ownership, runner information, and declaration warnings. Managed secrets are not included. Runtime and node output values belong to one execution and remain in that run's detail view.

JSON exports identify their document structure with a `format` field. Run files use `baudbound.run`, log files use `baudbound.logs`, and variable files use `baudbound.variables`. The separate `format_version` number allows BaudBound tools to recognize future structural changes.

Use **Clear runs** on the Runs page to delete every completed run record stored by the runner. This includes records that are older than the ones currently shown. BaudBound asks for confirmation before deleting them. Running scripts are not stopped and can create new records when they finish.

Use **Clear logs** on the Logs page when you want to remove stored messages but keep the run records. Run status, identifiers, completion times, and final variable values remain available. BaudBound asks for confirmation before clearing the messages. Running scripts can add new messages after the operation completes.

The desktop receives active run changes directly from the Rust runner. A run does not need to remain active until the next refresh interval. Finished history is refreshed only after the runner has committed its terminal record to SQLite.

The shared clock setting changes human readable desktop and CLI timestamps between 12 hour and 24 hour notation. Change it in the desktop Config page or with `baudbound config set display.time-format`. It does not alter stored timestamps, log order, or CLI JSON values.

## Inspect with the CLI

Replace `SCRIPT` with the installed name or ID shown by `baudbound script list`:

```text
baudbound doctor
baudbound status
baudbound script status
baudbound script inspect SCRIPT
baudbound script approval SCRIPT
baudbound script triggers SCRIPT
baudbound script logs --script SCRIPT --limit 20
```

Use `--json` only on commands that document it when collecting structured diagnostics. Human-readable output is intended for people and can change between versions.

## Diagnostic order

| Symptom | First check | Then inspect |
| --- | --- | --- |
| Import rejected | `baudbound validate PACKAGE` | Package format, target, version, graph, declarations |
| Script needs attention | Scripts or `baudbound script status` | Hash, approval, secrets, compatibility, enablement |
| Manual run rejected | `baudbound script inspect` | Manual trigger, approval, policy, target, secrets |
| No automatic event | Monitor, Doctor, and Service | Script enabled, registration, family toggle, OS prerequisite |
| Run failed at a node | Runs and Logs | Node config, resolved variables, native error, failure branch |
| Serial disconnected | Doctor, then Tools | Reader state, detected ports, access, protocol, USB identity, ambiguity |
| Webhook unavailable | Service listener | Bind, port, route, firewall/proxy, body limit |
| Update unavailable | Update dialog/error | HTTPS, release metadata, platform artifact, signature, clock |

## Common problems

### Package hash is not verified

The managed `.bbs` bytes no longer match the SHA-256 hash stored at import/update, or installation did not complete normally. Never replace the managed package in the runner home.

Return to the original editor project, export a new package, and validate it:

```text
baudbound validate PACKAGE
baudbound script update PACKAGE
baudbound script inspect SCRIPT
```

Review and approve the new revision after the status reports a valid package hash.

### Script will not run

Read every problem reported by `baudbound script inspect`. A run requires a valid installed package, compatible target, current approval, allowed policy, all required secrets, and a usable trigger. A direct manual run also needs a Manual Trigger unless `--trigger TRIGGER` explicitly selects another valid trigger node.

Enablement controls long-lived trigger registration. It is not required for a direct `baudbound script run`, but it is required for schedules, listeners, watchers, startup registration, and serial readers.

### Approval became stale

An update changed the package hash. Inspect the new access declarations and graph, then approve the current revision. Do not attempt to copy or edit approval rows. See [Approvals, Capabilities, and Risk](../security/approvals-capabilities.md).

### Trigger does not fire

Confirm all of these:

1. the script is enabled, valid, compatible, approved, and secret-ready.
2. its trigger appears in **Doctor** or `baudbound script triggers SCRIPT`.
3. the trigger family is enabled in `config.toml`.
4. one runner service owns the runner home and listener port.
5. the watched path, process, serial device, hotkey session, or network route exists.
6. service logs do not report registration or dispatch failure.

Script lifecycle changes reload automatically at `runner.trigger_reload_seconds`. Configuration changes require a service restart.

### Serial reconnect or rebinding fails

Check physical connectivity, port permissions, baud/data/parity/stop/flow settings, and `read_mode`. With identity validation enabled, a mismatch intentionally prevents the wrong device from opening. Auto rebind refuses zero or multiple matches. Identical devices should have distinct serial numbers in configuration.

If a scanner beeps, restarts, or disconnects when the runner starts, set `dtr_on_open` to `deasserted`. Increase `open_stabilization_ms` if the device needs more than 500 milliseconds before it is ready. Bytes received during this wait are discarded. Doctor shows the configured DTR policy and wait time for each reader.

Open **Doctor** and find the affected Serial Input row. **Active port** is the port currently owned by the shared device connection. **Message framing** shows the selected mode and how many bytes are waiting for completion. **Last error** shows either the latest native connection failure or framing failure.

If Idle gap produces several runs for one device message, increase `message_gap_ms`. If it waits too long after a complete message, decrease the value. If Line mode never starts a run, confirm that the device sends `CR`, `LF`, or `CRLF`. If the device sends no ending, use Idle gap. Raw mode can split one logical message into several runs because operating system chunks are not message boundaries.

An exceeded message limit means the device sent more than `max_message_bytes` without a valid boundary. BaudBound discards that oversized message and resumes at the next idle gap or line ending. Increase the limit only after confirming that the larger message is expected.

On Linux, inspect ownership with `ls -l /dev/ttyUSB0` or the real device path and confirm the runner account belongs to the required group.

### Webhook or WebSocket is unavailable

Confirm the family toggle, bind address, port, route, token status, allowed browser origin, and size limits. Use `baudbound trigger-auth list SCRIPT` or the desktop Security view to inspect token protection without exposing the token. Generate a replacement for a lost token and update the client with the new value. Use Doctor to confirm that the trigger is registered and healthy.

A Webhook `401` response means the token is missing. A `403` response means the token is invalid or the browser Origin is not allowed. A `503` response means authentication storage or the run executor is temporarily unavailable. WebSocket clients receive the equivalent error during the opening handshake.

Loopback accepts only local clients. A public bind is refused when a matching trigger has authentication disabled unless the unsafe override is enabled. It can still be blocked by the firewall or reverse proxy. A port conflict means another process, including a second BaudBound service, already owns it.

Use [Webhooks, WebSockets, and Network Access](network-listeners.md) for local request tests and exposure guidance.

### Encrypted secret storage is unavailable

Open the Security page and check whether BaudBound reports that encrypted secret storage is unavailable.

BaudBound connects to the credential vault in the background. A slow or broken credential service does not delay the desktop window from opening. Secret actions remain unavailable while the Security page displays Connecting.

On Linux, check Secret Service from the same terminal and graphical session used to launch BaudBound:

```bash
busctl --user list | grep org.freedesktop.secrets
```

A process ID means the service is running. A result containing only `activatable` means the provider is installed but did not start.

Test the provider independently from BaudBound:

```bash
printf 'baudbound-test' | secret-tool store --label='BaudBound test' application baudbound-test
```

Remove the test value after a successful check:

```bash
secret-tool clear application baudbound-test
```

If the storage command fails, repair the desktop credential service before troubleshooting BaudBound. If it succeeds, return to the Security page and select Retry. The page updates automatically when the connection attempt finishes.

VNC and other remote desktop sessions may require their own credential service startup configuration. Follow [Linux encrypted secret storage](installation.md#linux-encrypted-secret-storage) for installation and verification instructions.

### File or process action fails

Log the resolved path or executable identifier without logging credentials. Check the runner account's permissions and working environment. Desktop-only actions require an active desktop target/session. Windows-title lookup modes are Windows Desktop-only. Use process-name or PID modes where documented on Linux.

HTTP Response, File Download, and File Read fail before returning oversized data. Check the three values under `[limits]`. An oversized download is removed before it can replace an existing destination file.

### Update is rejected

Check system time, HTTPS access to the GitHub release endpoint, availability of the correct artifact, and updater metadata. An AppImage must remain writable by its owner. Debian and RPM installations must be updated through APT or DNF and never through the AppImage updater. Do not bypass signature or checksum verification. Use the matching recovery steps from [Installation and Updates](installation.md).

## Collect a useful support report

Include the runner version, operating system, target runtime, command or UI action, script ID/name, run ID, trigger/node ID, and exact redacted error. Include relevant `baudbound doctor --json`, `baudbound status --json`, or `baudbound script inspect SCRIPT --json` output only after removing usernames, private paths, network tokens, serial numbers, and other sensitive values.

Do not attach `.bbs` packages, `runner.db`, `config.toml`, environment files, or secret keys to a public report unless you have inspected and intentionally sanitized them.

For backups and database-level recovery boundaries, continue with [Storage, Backups, and Recovery](storage-backups.md).
