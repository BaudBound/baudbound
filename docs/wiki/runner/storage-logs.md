---
title: Runs, Logs, and Troubleshooting
description: Inspect execution history, locate runner data, and diagnose common failures.
tags: [runner, storage, logs]
---
# Runs, Logs, and Troubleshooting

Runner data is stored beneath `BAUDBOUND_HOME`, or the platform application-data directory when no override is set. Installed packages, approvals, run history, logs, and trigger state are managed by the runner. Configuration remains in `config.toml`.

Each run records script and trigger identity, start and completion time, terminal status, and error summary. Structured log entries include time, level, script, run, node, and message. Variable snapshots support run inspection but exclude secret plaintext.

Do not edit runner-managed state directly. Use runner commands or the desktop UI. Stop the runner before making a filesystem-level backup of its home directory.

## Troubleshooting

Start with:

```text
baudbound doctor
baudbound status
baudbound script status SCRIPT
baudbound script logs SCRIPT
```

### Package hash is not verified

Re-export from the current editor. Do not modify or repack `.bbs` contents. Validate the source with `baudbound validate`, update the installed script, and approve the new revision.

### Script will not run

Check package validation, target runtime, enablement, approval, required secrets, and requested capabilities. Approval must match the exact installed revision.

### Trigger does not fire

Confirm the script is enabled and approved, its listener family is enabled, and only one runner service owns the home directory. Check port conflicts, bind addresses, file permissions, desktop-session availability, and service logs.

### Serial reconnect fails

Check physical connectivity, port access, protocol settings, and identity fields. Identical devices need serial numbers to avoid ambiguous matches. An identity mismatch intentionally prevents connection to the wrong hardware.

### Webhook or WebSocket is unavailable

Confirm the listener family is enabled and the service owns its configured port. Loopback binds are local only; non-loopback binds may be blocked by the host firewall.

### Update is rejected

Check system time, access to the GitHub release, artifact availability, and signature metadata. Do not bypass signature verification.
