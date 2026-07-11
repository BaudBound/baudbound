---
title: Script Management
description: Import, inspect, approve, enable, update, run, and remove BaudBound packages.
tags: [runner, scripts, getting-started]
---
# Script Management

The desktop Scripts view and `baudbound script` commands operate on the same installed scripts.

## Import and inspect

```text
baudbound script import C:\path\to\automation.bbs
baudbound script list
baudbound script inspect automation
```

Import validates the archive, manifest, executable graph, node configuration, integrity hashes, minimum runner version, target runtime, permissions, and capabilities before installation.

**Verified** means package content matches its integrity information. **Not verified** means the package lacks valid integrity proof or its bytes no longer match. Re-export an unverified package from the current editor instead of modifying its archive.

## Approve

```text
baudbound script approval automation
baudbound script approve automation
```

Review author information, target runtime, requested capabilities, risk, secret declarations, and included nodes. **Approved for this version** means the decision matches the installed content and capabilities. **Review required** means approval is missing or no longer matches.

Approval belongs to an exact package revision, not a filename. Updating package content invalidates the previous approval.

## Run and enable

Run a manual trigger:

```text
baudbound script run automation
baudbound script logs automation
```

Enable the package when its listener-based triggers should load in the background:

```text
baudbound script enable automation
baudbound serve
```

Enablement does not bypass validation or approval. A disabled script remains installed and inspectable but is not loaded by the service.

## Update or remove

Use `baudbound script update PATH` to replace an installed package with another export of the same script identity. Review and approve the new revision before running it.

Use `baudbound script remove SCRIPT` to remove a package from the installed script list. Review any run information you need before removal.
