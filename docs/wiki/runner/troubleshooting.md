---
title: Runner Troubleshooting
description: Diagnose package, approval, service, trigger, serial, UI, and update failures.
tags: [runner, troubleshooting]
---
# Runner Troubleshooting

Start with:

```text
baudbound doctor
baudbound status
baudbound script status SCRIPT
baudbound script logs SCRIPT
```

## Package hash is not verified

Re-export the package from the current editor. Do not modify or repack `.bbs` contents. Validate the source file with `baudbound validate` and update the installed script. A changed revision needs a new approval.

## Script will not run

Inspect validation, target runtime, enablement, approval, required secrets, and requested capabilities. Approval must match the exact installed revision.

## Trigger does not fire

Confirm the script is enabled and approved, the listener family is enabled, and one runner service owns the home directory. Check port conflicts, bind addresses, file permissions, desktop-session availability, and trigger service logs. Reload triggers after configuration changes when immediate reconciliation is needed.

## Serial reconnect fails

Check physical connectivity, port access, protocol settings, and identity fields. Ambiguous identical devices require serial numbers. An identity mismatch intentionally prevents connection to the wrong hardware.

## Webhook or WebSocket unavailable

Confirm its trigger family is enabled and the service owns the configured port. Loopback binds are local only. Non-loopback binds may be blocked by host firewall policy.

## Update is rejected

Verify system time, network access to GitHub releases, `latest.json`, artifact availability, and signature metadata. Do not bypass signature verification.
