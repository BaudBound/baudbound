---
title: Runner Quick Start
description: Import, approve, and execute a BaudBound package.
tags:
  - getting-started
  - runner
---
# Runner Quick Start

## Desktop application

1. Open BaudBound and select **Scripts**.
2. Import the `.bbs` package exported by the editor.
3. Review its target runtime, package hash, risk level, permissions, and capabilities.
4. Approve the current package when the requested access is acceptable.
5. Run the script or start the desktop background runner for listener-based triggers.

Approval is tied to the exact package hash. Updating a package invalidates its previous approval and requires another review.

## Command line

```bash
baudbound script import ./automation.bbs
baudbound script inspect automation
baudbound script approve automation
baudbound script run automation
baudbound script logs --script automation
```

Use `baudbound serve` for long-running schedules, file watchers, serial input, webhooks, WebSockets, and process-start listeners in a headless environment. Your process manager is responsible for starting and supervising that command.
