---
title: Script Management
description: Import, inspect, approve, enable, update, run, and remove BaudBound packages.
tags: [runner, scripts, getting-started]
---
# Script Management

The desktop Scripts view and `baudbound script` commands operate on the same installed scripts.

In the commands below, `SCRIPT` means either the script name shown by `script list` or its manifest ID. `PACKAGE` means the path to an exported `.bbs` file. Do not type these placeholders literally.

For normal desktop use, open **Scripts** to perform these operations without entering commands. Use the CLI examples when operating a headless runner or diagnosing a problem.

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
./BaudBound_*.AppImage script import "$HOME/Downloads/desk-lights.bbs"
```

Import validates the archive, manifest, executable graph, node configuration, integrity hashes, minimum runner version, target runtime, permissions, and capabilities before installation. A successful import prints the installed script name and ID. A failed import does not install a partially accepted package.

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

Remove an installed script with:

```text
baudbound script remove SCRIPT
```

Confirm the script is no longer listed with `baudbound script list`. Review or export any run information you need before removal.
