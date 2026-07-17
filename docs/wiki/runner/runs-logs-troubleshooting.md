---
title: Runs, Logs, and Troubleshooting
description: Inspect execution history, correlate logs, and diagnose package, approval, trigger, device, network, or update failures.
tags: [runner, storage, logs, troubleshooting]
---
# Runs, Logs, and Troubleshooting

Use this page after an import, run, or listener behaves differently from what you expected. Start with the recorded error and move outward through script state, trigger state, configuration, and platform support. Do not edit SQLite or installed package files to make an error disappear.

## What a run records

Every terminal run record contains:

- a run ID and stable script ID;
- the trigger node ID;
- completion timestamp;
- status: `completed`, `failed`, or `cancelled`;
- structured log entries with their own emission timestamp, level, message, and optional node ID; and
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

1. Open **Scripts** and expand the affected script. Confirm package hash, compatibility, enablement, approval, and required-secret state.
2. Open **Runs**, select the relevant script, and choose the newest failed or cancelled run.
3. Copy the run ID, trigger ID, status, and first useful error.
4. Inspect its node logs and final variables. Each log time records when that entry was emitted, not when the run finished. A node ID connects a runtime failure to the editor graph.
5. Open **Triggers** when the run never started or a configured serial reader is disconnected. Use **Tools** to scan the serial ports currently detected by the machine. Open **Service** for listener state or **Doctor** for machine support.

The **Logs** tab searches messages across recent runs. Use a run ID to avoid mixing errors from two overlapping executions.

The shared clock setting changes human-readable desktop and CLI timestamps between 12-hour and 24-hour notation. Change it in the desktop Settings tab or with `baudbound settings set time-format`. It does not alter stored timestamps, log order, or CLI JSON values.

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
| No automatic event | Triggers and Service | Script enabled, registration, family toggle, OS prerequisite |
| Run failed at a node | Runs and Logs | Node config, resolved variables, native error, failure branch |
| Serial disconnected | Triggers, then Tools | Reader state, detected ports, access, protocol, USB identity, ambiguity |
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

1. the script is enabled, valid, compatible, approved, and secret-ready;
2. its trigger appears in **Triggers** or `baudbound script triggers SCRIPT`;
3. the trigger family is enabled in `config.toml`;
4. one runner service owns the runner home and listener port;
5. the watched path, process, serial device, hotkey session, or network route exists; and
6. service logs do not report registration or dispatch failure.

Script lifecycle changes reload automatically at `runner.trigger_reload_seconds`. Configuration changes require a service restart.

### Serial reconnect or rebinding fails

Check physical connectivity, port permissions, baud/data/parity/stop/flow settings, and `read_mode`. With identity validation enabled, a mismatch intentionally prevents the wrong device from opening. Auto rebind refuses zero or multiple matches. Identical devices should have distinct serial numbers in configuration.

On Linux, inspect ownership with `ls -l /dev/ttyUSB0` or the real device path and confirm the runner account belongs to the required group.

### Webhook or WebSocket is unavailable

Confirm the family toggle, bind address, port, route, and size limits. Loopback accepts only local clients. A public bind can still be blocked by the firewall or reverse proxy. A port conflict means another process, including a second BaudBound service, already owns it.

Use [Webhooks, WebSockets, and Network Access](network-listeners.md) for local request tests and exposure guidance.

### File or process action fails

Log the resolved path or executable identifier without logging credentials. Check the runner account's permissions and working environment. Desktop-only actions require an active desktop target/session. Windows-title lookup modes are Windows Desktop-only; use process-name or PID modes where documented on Linux.

### Update is rejected

Check system time, HTTPS access to the GitHub release endpoint, availability of the exact Windows or Linux artifact, and updater signature metadata. On Linux, the AppImage must remain writable by its owner. Do not bypass signature verification; use the documented manual fallback from [Installation and Updates](installation.md).

## Collect a useful support report

Include the runner version, operating system, target runtime, command or UI action, script ID/name, run ID, trigger/node ID, and exact redacted error. Include relevant `baudbound doctor --json`, `baudbound status --json`, or `baudbound script inspect SCRIPT --json` output only after removing usernames, private paths, network tokens, serial numbers, and other sensitive values.

Do not attach `.bbs` packages, `runner.db`, `config.toml`, environment files, or secret keys to a public report unless you have inspected and intentionally sanitized them.

For backups and database-level recovery boundaries, continue with [Storage, Backups, and Recovery](storage-backups.md).
