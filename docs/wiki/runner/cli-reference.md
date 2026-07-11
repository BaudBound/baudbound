---
title: CLI Reference
description: Commands and global options exposed by the baudbound executable.
tags: [runner, cli, reference]
---
# CLI Reference

```text
baudbound [--config PATH] <command>
baudbound --version
```

`--config` selects an alternate runner TOML file. Without a command, BaudBound opens the desktop UI when a graphical session is available.

## General commands

| Command | Purpose |
| --- | --- |
| `config path` | Print the active configuration path. |
| `config print` | Print validated configuration. |
| `config init` | Explicitly write a default config; normal startup already creates one. |
| `status [--json]` | Report runner and installed-script status. |
| `ui` | Explicitly open the desktop UI. |
| `doctor [--json]` | Run environment and configuration diagnostics. |
| `validate PACKAGE` | Validate a `.bbs` package without installing it. |
| `inspect PACKAGE` | Print package metadata and security information. |
| `serve` | Run listener-based trigger services. |

## Script commands

`baudbound script` provides `import`, `update`, `list`, `status`, `inspect`, `enable`, `disable`, `remove`, `approval`, `triggers`, `dispatch-trigger`, `approve`, `revoke-approval`, `run`, and `logs`.

Package arguments may be an installed script name or ID where the command supports installed scripts. Import preserves the original package filename in the scripts directory while SQLite holds canonical identity and state.

## Secret commands

`baudbound secret generate-key` creates a headless encryption key. `secret list`, `secret set`, and `secret remove` manage declared values. Secret values are never printed by list operations.

## Hotkey commands

`baudbound hotkey list` shows registered hotkeys. `hotkey dispatch` triggers a matching binding. `hotkey listen` runs the native desktop listener.

## Serve options

`serve` supports dry-run and JSON preflight output, single-pass startup, immediate schedule testing, hotkey input, listener overrides, webhook bind/port/body limits, WebSocket bind/port/message/connection limits, and reload interval overrides. Run `baudbound serve --help` for the exact options supported by the installed version.

Commands return a non-zero status on validation, policy, storage, configuration, or execution failure. Use `--json` only on commands that declare it; human output is not a stable machine protocol.
