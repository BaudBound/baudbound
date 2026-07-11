---
title: Script Management
description: Import, inspect, approve, enable, update, run, and remove BaudBound packages.
tags: [runner, scripts, getting-started]
---
# Script Management

The desktop Scripts view and `baudbound script` commands operate on the same installed scripts.

In the commands below, `SCRIPT` means either the script name shown by `script list` or its manifest ID. `PACKAGE` means the path to an exported `.bbs` file. Do not type these placeholders literally.

For normal desktop use, open **Scripts** to perform these operations without entering commands. Use the CLI examples when operating a headless runner or diagnosing a problem.

## Lifecycle

```text
.bbs file -> validated import -> installed -> reviewed -> approved -> enabled -> triggered
                                      |                         |
                                      +---- manual run ---------+
                                      |
                                      +---- update or remove
```

**Installed** means the runner has accepted and copied a package. **Approved** means the operator accepts that exact hash and permission set. **Enabled** means eligible listener triggers may register. These states are independent.

## Import and inspect

```text
baudbound script import PACKAGE
baudbound script list
baudbound script inspect SCRIPT
```

Run the commands in that order. Import validates and stores the package. List shows the name or ID accepted by later commands. Inspect displays the exact imported revision, target runtime, nodes, requested access, integrity state, and approval state.

Example package paths:

```powershell
baudbound script import "C:\Users\Alice\Downloads\desk-lights.bbs"
```

```text
baudbound script import "$HOME/Downloads/desk-lights.bbs"
```

Import validates the archive, manifest, executable graph, node configuration, integrity hashes, minimum runner version, target runtime, permissions, and capabilities before installation. A successful import prints the installed script name and ID. A failed import does not install a partially accepted package.

The runner stages validation before durable installation. Rejection leaves existing script records, package files, approvals, variables, secrets, and registrations unchanged.

Display names are not guaranteed unique. The manifest ID is stable and unambiguous. When a name matches more than one installed script, use the ID printed by `script list` or inspection rather than guessing.

**Verified** means package content matches its integrity information. **Not verified** means the package lacks valid integrity proof or its bytes no longer match. Re-export an unverified package from the current editor instead of modifying its archive.

## Approve

```text
baudbound script approval SCRIPT
baudbound script approve SCRIPT
```

Run `approval` first and review its output. Check author information, target runtime, requested capabilities, risk, secret declarations, and included nodes. If the information matches the workflow you expect, run `approve`.

**Approved for this version** means the decision matches the installed content and capabilities. **Review required** means approval is missing or no longer matches. Approval does not fix validation, compatibility, policy, or missing-secret errors.

Approval belongs to an exact package revision, not a filename. Updating package content invalidates the previous approval.

## Run and enable

Run a manual trigger:

```text
baudbound script run SCRIPT
baudbound script logs --script SCRIPT
```

`run` starts the script's manual trigger. It fails when the package has no manual trigger; use the desktop Triggers view or `baudbound script triggers SCRIPT` to see the available trigger node IDs.

To run a specific trigger with controlled JSON input:

```text
baudbound script run SCRIPT --trigger TRIGGER --payload-json '{"test":true}'
```

Replace `TRIGGER` with the node ID shown by trigger inspection. Listener-generated production events normally use the background service instead.

Enable the package when its listener-based triggers should load in the background:

```text
baudbound script enable SCRIPT
baudbound serve
```

Enablement does not bypass validation or approval. A disabled script remains installed and inspectable but its schedule, webhook, serial, file, process, startup, WebSocket, and hotkey listeners are not loaded by the background runner.

The foreground `serve` command is mainly for headless operation and testing. Desktop users can start the desktop-owned background runner from **Service** instead.

## Update or remove

Export the edited project from the editor, then update using the new package path:

```text
baudbound script update PACKAGE
baudbound script inspect SCRIPT
baudbound script approve SCRIPT
```

Update succeeds only when the package has the same manifest identity as an installed script. It invalidates the old approval because the content changed. Inspect and approve the replacement before running it.

Update preserves the script's enabled state, import identity, persistent variables, configured secret values, and historical runs. The previous approval remains recorded against its old hash and is reported as stale until the new revision is approved. When the imported filename changes, the old stored package copy is removed after the replacement is committed.

Remove an installed script with:

```text
baudbound script remove SCRIPT
```

Confirm the script is no longer listed with `baudbound script list`. Review or export any run information you need before removal.

Removal deletes the installed package copy and script row. Database foreign-key cleanup removes its approval, script-scoped persistent variables, and encrypted secret values. Global variables remain because they are runner-wide. Historical run records are retained by current storage so past diagnostics remain available, but they no longer make the removed script executable.

> Removal is not a temporary disable operation. Use `baudbound script disable SCRIPT` when you want to keep package state and stop unattended listeners.
{.is-warning}

## Desktop equivalents

| CLI task | Scripts view |
| --- | --- |
| `script import PACKAGE` | **Import package** |
| `script list` / `inspect` | Script row and expandable details |
| `script approve` | Approval review dialog |
| `script enable` / `disable` | Row action menu |
| `script run` | **Run** button |
| `script update PACKAGE` | **Update package** action |
| `script revoke-approval` | Revoke approval action |
| `script remove` | Remove action and confirmation dialog |

## Recovery links

- Package or hash problem: [Runs, Logs, and Troubleshooting](runs-logs-troubleshooting.md#package-hash-is-not-verified)
- Approval or risk question: [Approvals, Capabilities, and Risk](../security/approvals-capabilities.md)
- Missing secret: [Secrets](secrets.md)
- Trigger not active: [Background Service and Triggers](service-triggers.md)
- Backup before removal or migration: [Storage, Backups, and Recovery](storage-backups.md)
