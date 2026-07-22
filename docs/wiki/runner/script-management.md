---
title: Script Management
description: Import, inspect, approve, enable, update, run, and remove BaudBound packages.
tags: [runner, scripts, getting-started]
---
# Script Management

The desktop Scripts view and `baudbound script` commands operate on the same installed scripts.

In the commands below, `SCRIPT` means either the script name shown by `baudbound script list` or its manifest ID. `PACKAGE` means the path to an exported `.bbs` file. Do not type these placeholders literally.

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

Import validates the archive, manifest, executable graph, node configuration, integrity hashes, minimum runner version, target runtime, permissions, and capabilities before installation. A successful import prints the installed script name and ID. Import does not create Webhook or WebSocket credentials. Those credentials are created only after you review and approve the package. A failed import does not install a partially accepted package.

In the desktop app, choose **View details** on a script row to read the descriptive information stored in the verified package manifest. The dialog shows the author, description, website, source, tags, creation details, minimum runner version, target runtime, package identity, health, permissions, triggers, and recent runs when those values are available. These fields describe the package but do not replace the approval review. Always review permissions, capabilities, risk, and package integrity separately.

The runner stages validation before durable installation. Rejection leaves existing script records, package files, approvals, variables, secrets, and registrations unchanged.

Display names are not guaranteed unique. The manifest ID is stable and unambiguous. When a name matches more than one installed script, use the ID printed by `baudbound script list` or inspection rather than guessing.

**Verified** means package content matches its integrity information. **Not verified** means the package lacks valid integrity proof or its bytes no longer match. Re-export an unverified package from the current editor instead of modifying its archive.

## Approve

```text
baudbound script approval SCRIPT
baudbound script approve SCRIPT
```

Run `approval` first and review its output. Check author information, target runtime, requested capabilities, risk, secret declarations, and included nodes. If the information matches the workflow you expect, run `approve`.

**Approved for this version** means the decision matches the installed content and capabilities. **Review required** means approval is missing or no longer matches. Approval does not fix validation, compatibility, policy, or missing-secret errors.

Approval belongs to an exact package revision, not a filename. Updating package content invalidates the previous approval.

When approval creates credentials for Webhook or WebSocket triggers, the CLI prints each new token and the desktop app opens a token dialog. Save the values immediately because the runner stores only their hashes. Approving an unchanged trigger again does not replace its existing token.

## Run and enable

Run a manual trigger:

```text
baudbound script run SCRIPT
baudbound script logs --script SCRIPT
```

`run` starts the script's manual trigger. It fails when the package has no manual trigger. Expand the script in the desktop Scripts view or run `baudbound script triggers SCRIPT` to see the available trigger node IDs.

To run a specific trigger with controlled JSON input:

```text
baudbound script run SCRIPT --trigger TRIGGER --payload-json '{"test":true}'
```

Replace `TRIGGER` with the node ID shown by trigger inspection. Listener-generated production events normally use the background service instead.

The JSON after `--payload-json` is test data for the selected trigger. It is not a runner setting. See [Supplying trigger test data](cli-reference.md#supplying-trigger-test-data) for a complete example and the variable names created from the JSON.

Enable the package when its listener-based triggers should load in the background:

```text
baudbound script enable SCRIPT
baudbound serve
```

Enablement does not bypass validation or approval. A disabled script remains installed and inspectable, but it cannot start a manual run, automatic run, queued trigger event, or sub-script run. Its schedule, webhook, serial, file, process, startup, WebSocket, and hotkey listeners are not loaded by the background runner. Disabling does not force-cancel a run that is already executing.

The foreground `baudbound serve` command is mainly for headless operation and testing. Desktop users can start the desktop-owned background runner from **Service** instead.

## Update or remove

Export the edited project from the editor, then update using the new package path:

```text
baudbound script update PACKAGE
baudbound script inspect SCRIPT
baudbound script approve SCRIPT
```

Update succeeds only when the package has the same manifest identity as an installed script. It invalidates the old approval because the content changed. Inspect and approve the replacement before running it.

Open the same saved editor project when creating an update. Its later exports retain the manifest identity automatically. Duplicating the editor project or choosing **Import copy** creates a different identity and must be imported as another script instead of used with Update.

Update preserves the script's enabled state, import identity, persistent variables, configured secret values, network tokens for unchanged trigger node IDs, and historical runs. A newly added Webhook or WebSocket trigger receives a token only after the updated package is approved. The CLI prints that token once and the desktop app shows it once. The previous approval remains recorded against its old hash and is reported as stale until the new revision is approved. When the imported filename changes, the old stored package copy is removed after the replacement is committed.

Remove an installed script with:

```text
baudbound script remove SCRIPT
```

Confirm the script is no longer listed with `baudbound script list`. Review or export any run information you need before removal.

Removal deletes the installed package copy and script row. Database foreign-key cleanup removes its approval, script-scoped persistent variables, and encrypted secret values. Global variables remain because they are runner-wide. Historical run records are retained by current storage so past diagnostics remain available, but they no longer make the removed script executable.

> Removal is not a temporary disable operation. Use `baudbound script disable SCRIPT` when you want to keep package state and stop unattended listeners.
{.is-warning}

## Remote import and update checks

The desktop **Import** command can use a local `.bbs` file, a direct public `.bbs` URL, or a public `update.json` URL. The desktop **Update** command accepts a local `.bbs` file or a direct public `.bbs` URL.

A remote package is downloaded into protected temporary storage. The runner validates it and displays its identity, version, SHA256, target, risk, permissions, and capabilities before installation. Installing does not approve or run it.

When an installed package declares an update URL, its details dialog contains an **Updates** section. Choose **Check for updates** to check only that script. Choose **Check updates** on the main Scripts page to check every configured script. Scripts without an update URL are listed as **Not configured** and are not contacted.

The main page can show these states:

| State | Meaning |
| --- | --- |
| **Not configured** | The package does not declare an update URL |
| **Not checked** | An update URL exists but no successful check matches it yet |
| **Up to date** | The descriptor is valid and does not publish a newer version |
| **Update available** | A valid descriptor publishes a newer version |
| **Check failed** | The descriptor could not be fetched or validated |
| **Unavailable** | The installed package metadata cannot be inspected |

Choose **Review update** to download the discovered package. The runner verifies the package against the descriptor again before showing the review. Replacing the installed package makes the previous approval stale. Review and approve the new exact hash before enabling or running it.

**Update all** creates a review queue. It does not approve packages as a group. Review, install, skip, or cancel each package separately.

Automatic checks are disabled for every script by default. Enabling **Automatic update checks** requires confirmation because the publisher's server can observe the runner's public IP address and request time. Automatic checks only discover metadata. They never download or install a package.

Remote URLs must use public HTTPS destinations. The runner rejects credentials in URLs, unsafe redirects, local addresses, private network addresses, oversized responses, unexpected filenames, hash mismatches, reused versions with changed bytes, and downgrades.

> A valid package is not automatically trustworthy. Install scripts only from publishers you trust. BaudBound checks structure, integrity, compatibility, declared access, and exact package bytes. It cannot determine the intentions of a publisher or script.
{.is-warning}

Authors can read [Publishing Script Updates](../editor/publishing-script-updates.md) for the complete `update.json` and GitHub publishing workflow.

## Desktop equivalents

| CLI task | Scripts view |
| --- | --- |
| `baudbound script import PACKAGE` | **Import** |
| `baudbound script list` / `baudbound script inspect SCRIPT` | Script row, **About**, and expandable details |
| `baudbound script approve SCRIPT` | Approval review dialog |
| `baudbound script enable SCRIPT` / `baudbound script disable SCRIPT` | Row action menu |
| `baudbound script run SCRIPT` | **Run** button |
| `baudbound script update PACKAGE` | **Update package** action |
| `baudbound script revoke-approval SCRIPT` | Revoke approval action |
| `baudbound script remove SCRIPT` | Remove action and confirmation dialog |

## Recovery links

- Package or hash problem: [Runs, Logs, and Troubleshooting](runs-logs-troubleshooting.md#package-hash-is-not-verified)
- Approval or risk question: [Approvals, Capabilities, and Risk](../security/approvals-capabilities.md)
- Missing secret: [Secrets](secrets.md)
- Trigger not active: [Background Service and Triggers](service-triggers.md)
- Backup before removal or migration: [Storage, Backups, and Recovery](storage-backups.md)
