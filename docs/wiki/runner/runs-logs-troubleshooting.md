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

Use this order instead of changing files or approvals at random:

```text
baudbound doctor
baudbound status
baudbound script status
baudbound script inspect SCRIPT
baudbound script approval SCRIPT
baudbound script logs --script SCRIPT --limit 20
```

Replace `SCRIPT` with the installed name or ID shown by `baudbound script list`. `doctor` checks the machine and configuration. `status` checks runner-wide health. `script status` identifies scripts needing attention. The remaining commands inspect one script, its approval, and its recent runs.

In the desktop application, the equivalent information is available in **Doctor**, **Service**, **Scripts**, **Runs**, and **Logs**. Copy the exact error message before changing configuration; it normally names the failing script, node, listener, or policy check.

### Package hash is not verified

The package was created without valid integrity metadata or changed after export. Open the original project in the current editor and export it again. Do not modify or repack `.bbs` contents.

Validate the newly exported file before replacing the installed revision:

```text
baudbound validate PATH_TO_NEW_PACKAGE
baudbound script update PATH_TO_NEW_PACKAGE
baudbound script inspect SCRIPT
baudbound script approve SCRIPT
```

### Script will not run

Run `baudbound script inspect SCRIPT` and read each reported state. Fix validation or target incompatibility in the editor. Approve the current revision only after review. Configure every required secret. For manual execution, the script also needs a manual trigger. Enablement is required for listener-based triggers but not for a direct manual run.

### Trigger does not fire

Confirm the script is enabled and approved, then open **Triggers** or run `baudbound script triggers SCRIPT`. The expected trigger must appear as registered. Confirm its listener family is enabled in `config.toml` and that exactly one runner service uses this runner home. Then inspect service logs for a port conflict, inaccessible path, unavailable desktop session, or device error.

### Serial reconnect fails

Check physical connectivity, port access, protocol settings, and identity fields. Identical devices need serial numbers to avoid ambiguous matches. An identity mismatch intentionally prevents connection to the wrong hardware.

### Webhook or WebSocket is unavailable

Confirm the listener family is enabled and the service owns its configured port. Loopback binds are local only; non-loopback binds may be blocked by the host firewall.

### Update is rejected

Check system time, access to the GitHub release, artifact availability, and signature metadata. Do not bypass signature verification.
