---
title: Runner Quick Start
description: Import, inspect, approve, run, and monitor a BaudBound package.
tags: [runner, getting-started]
---
# Runner Quick Start

## Import a package

Use the Scripts tab in the desktop UI, or run:

```text
baudbound script import C:\Downloads\my-automation.bbs
baudbound script list
```

The runner validates the archive, manifest, program schema, node configurations, integrity hashes, minimum runner version, and target runtime before installation.

## Review and approve

```text
baudbound script inspect my-automation
baudbound script approval my-automation
baudbound script approve my-automation
```

The approval view shows whether the installed revision is approved and whether the capability decision still matches its content. Read [Scripts and approvals](scripts-approvals.md) before approving untrusted packages.

## Run manually

```text
baudbound script run my-automation
baudbound script logs my-automation
```

Manual execution selects an applicable manual trigger. Trigger dispatch and run output are recorded in durable storage.

## Enable background triggers

```text
baudbound script enable my-automation
baudbound serve
```

The service loads enabled, valid, approved scripts. Listener families must also be enabled in `config.toml`. Use `baudbound status`, the desktop Service tab, and the Logs or Runs tabs to monitor execution.
