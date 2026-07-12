---
title: CLI Reference
description: Complete syntax and behavior reference for the baudbound command-line interface.
tags: [runner, cli, reference]
---
# CLI Reference

Install BaudBound first and confirm that the shell can find the executable with `baudbound --version`. [Installation and Updates](installation.md) creates the `baudbound` command for the supported installation paths.

```text
baudbound [--config PATH] [COMMAND]
baudbound --help
baudbound --version
```

`--config` is optional and global, so it can be used with any command. Without a command, BaudBound opens the desktop application when a graphical session is available. In a headless Linux session it prints runner status.

## Placeholders and quoting

Replace `SCRIPT`, `PACKAGE`, `TRIGGER`, `SECRET_NAME`, and `PATH` with real values. Do not type an uppercase placeholder literally.

Quote a path or name containing spaces. PowerShell and POSIX shells both accept ordinary double quotes around simple values:

```text
baudbound validate "PATH WITH SPACES"
baudbound script inspect "SCRIPT NAME"
```

JSON payload quoting differs between shells. Keep test payloads small, do not include secrets, and verify quoting in the actual shell before automating the command.

## Paths and configuration precedence

The runner resolves its configuration in this order:

1. `--config PATH`
2. `BAUDBOUND_CONFIG`
3. `<BAUDBOUND_HOME>/config.toml`

If `BAUDBOUND_HOME` is unset, the default home is `%LOCALAPPDATA%\BaudBound\runner` on Windows. On Linux it is `$XDG_DATA_HOME/BaudBound/runner`, or `~/.local/share/BaudBound/runner` when `XDG_DATA_HOME` is unset.

Normal startup creates a default config when the resolved file does not exist. Check the exact location with:

```text
baudbound config path
```

## General commands

### Configuration

| Command | Behavior |
| --- | --- |
| `baudbound config path` | Prints the resolved config path. |
| `baudbound config print` | Prints a complete example TOML template; it does not print the active file. |
| `baudbound config init` | Writes the starter template to the resolved path and refuses to replace an existing file. |
| `baudbound config init --force` | Replaces an existing config with the starter template. |

### Status and diagnostics

| Command | Behavior |
| --- | --- |
| `baudbound status` | Shows runner storage, supported targets, service health, and script totals. |
| `baudbound status --json` | Prints the same status as machine-readable JSON. |
| `baudbound doctor` | Checks configuration, storage, platform integration, and native desktop action support. |
| `baudbound doctor --json` | Prints diagnostic results as JSON. |
| `baudbound ui` | Explicitly opens the desktop application. |

### Package files

These commands operate directly on a `.bbs` file and do not install it:

```text
baudbound validate PATH_TO_PACKAGE
baudbound inspect PATH_TO_PACKAGE
baudbound inspect PATH_TO_PACKAGE --json
```

`baudbound validate` checks whether the package can be loaded and accepted by the runner. `baudbound inspect` prints package metadata and archive entries. To inspect an installed script instead, use `baudbound script inspect`.

## Script commands

An installed script can normally be identified by its manifest ID or installed name.

### Installation and state

| Command | Behavior |
| --- | --- |
| `baudbound script import PACKAGE` | Validates and installs a new `.bbs` package. |
| `baudbound script update PACKAGE` | Replaces the installed package with a new package carrying the same manifest ID. |
| `baudbound script list [--json]` | Lists installed scripts. |
| `baudbound script status [--json]` | Shows health across installed scripts, including loadability and approval state. |
| `baudbound script inspect SCRIPT [--json]` | Shows one installed script's manifest, nodes, access declarations, integrity, and state. |
| `baudbound script enable SCRIPT` | Allows the background service to register the script's listeners. |
| `baudbound script disable SCRIPT` | Stops background registration without removing the package. |
| `baudbound script remove SCRIPT` | Removes the installed script. |

### Approval

| Command | Behavior |
| --- | --- |
| `baudbound script approval SCRIPT [--json]` | Shows whether approval matches the installed package revision. |
| `baudbound script approve SCRIPT` | Approves the current package hash and declared access. |
| `baudbound script revoke-approval SCRIPT` | Removes the current approval. |

Updating package content invalidates its previous approval.

### Triggers and execution

```text
baudbound script triggers [SCRIPT] [--json]
baudbound script run SCRIPT [--trigger TRIGGER] [--payload-json JSON]
baudbound script dispatch-trigger SCRIPT TRIGGER [--payload-json JSON]
```

`baudbound script triggers` lists registered triggers for all installed scripts or one selected script. `baudbound script run` uses the script's manual trigger unless `--trigger` selects another trigger node ID. `baudbound script dispatch-trigger` always requires the trigger node ID.

`--payload-json` must contain valid JSON. Its value is exposed as trigger output data for the run. Shell quoting rules apply; in PowerShell, a payload can be passed as:

```powershell
baudbound script run my-automation --payload-json '{"message":"hello"}'
```

### Run history

```text
baudbound script logs [--script SCRIPT] [--limit NUMBER] [--json]
```

Without filters, this prints the 20 most recent stored runs. `--script` selects one installed script and `--limit` changes the maximum result count.

## Background service

```text
baudbound serve [OPTIONS]
```

Without overrides, `baudbound serve` uses `config.toml`, loads enabled and approved scripts, and remains active for listener-based triggers.

| Option | Behavior |
| --- | --- |
| `--dry-run` | Prints listener preflight status and exits without starting services. |
| `--json` | Uses JSON output for `--dry-run`; it is not a general service log format. |
| `--once` | Stops after the first due schedule batch. |
| `--run-schedules-immediately` | Dispatches all schedule triggers once before normal interval waiting. |
| `--hotkey-stdin` | Reads newline-delimited hotkey expressions from standard input. |
| `--webhooks` | Enables the webhook listener regardless of the config toggle. |
| `--webhook-bind ADDRESS` | Overrides the webhook bind address. |
| `--webhook-port PORT` | Overrides the webhook port. |
| `--max-webhook-body-bytes NUMBER` | Overrides the positive request-body limit. |
| `--websockets` | Enables the WebSocket listener regardless of the config toggle. |
| `--websocket-bind ADDRESS` | Overrides the WebSocket bind address. |
| `--websocket-port PORT` | Overrides the WebSocket port. |
| `--max-websocket-message-bytes NUMBER` | Overrides the positive message-size limit. |
| `--max-websocket-connections NUMBER` | Overrides the positive concurrent-connection limit. |
| `--reload-interval-seconds SECONDS` | Overrides how often installed trigger registrations are checked for changes. |

Command-line overrides apply to that service process only and do not rewrite `config.toml`. Use [Configuration and Serial Devices](configuration.md) for persistent settings.

## Hotkey commands

| Command | Behavior |
| --- | --- |
| `baudbound hotkey list [--json]` | Lists enabled hotkey trigger expressions. |
| `baudbound hotkey dispatch KEY [--json]` | Dispatches one expression, such as `Ctrl+Alt+B`, to matching enabled scripts. |
| `baudbound hotkey listen --stdin [--json]` | Reads newline-delimited expressions from standard input and dispatches each one. |

The CLI `listen` command currently requires `--stdin`; it does not install a native operating-system hotkey hook. With `--json`, it prints one JSON object per input event.

## Secret commands

| Command | Behavior |
| --- | --- |
| `baudbound secret generate-key` | Prints a new `BAUDBOUND_SECRET_KEY` assignment for headless secret encryption. |
| `baudbound secret list SCRIPT [--json]` | Lists declared secret names, types, requirements, and configured state without values. |
| `baudbound secret set SCRIPT NAME` | Prompts securely for a declared secret value. |
| `baudbound secret remove SCRIPT NAME` | Removes the configured value. |

Treat generated keys as credentials and do not store them in source control or command history. See [Secrets](secrets.md) for desktop and headless storage.

## JSON output and errors

`--json` is available only on commands that explicitly list it. Human-readable output is not a stable machine interface. JSON commands write their document to standard output; failures write an error and return a non-zero process exit code.

Validation, configuration, storage, policy, approval, secret, and execution failures also return non-zero. Use `baudbound COMMAND --help` to confirm syntax for the installed runner version.

Use [Script Management](script-management.md) for lifecycle procedures, [Background Service and Triggers](service-triggers.md) for `baudbound serve`, [Secrets](secrets.md) for key handling, and [Runs, Logs, and Troubleshooting](runs-logs-troubleshooting.md) when syntax is accepted but an operation is rejected.
