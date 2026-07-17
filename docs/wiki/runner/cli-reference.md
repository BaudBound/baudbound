---
title: CLI Reference
description: Complete syntax and behavior reference for the baudbound command-line interface.
tags: [runner, cli, reference]
---
# CLI Reference

CLI means command line interface. It lets you control BaudBound by typing commands in PowerShell on Windows or a terminal on Linux. The desktop application and CLI use the same installed scripts and runner data when they run under the same user account.

You do not need the CLI for normal desktop use. Use it for a headless Linux runner, repeatable administration, troubleshooting, or integration with another local tool.

Install BaudBound first and confirm that the shell can find the executable with `baudbound --version`. [Installation and Updates](installation.md) creates the `baudbound` command for the supported installation paths.

```text
baudbound [--config PATH] [COMMAND]
baudbound --help
baudbound --version
```

`--config` is optional and global, so it can be used with any command. Without a command, BaudBound opens the desktop application when a graphical session is available. In a headless Linux session it prints runner status.

Type only the command text shown inside a code block. Do not type labels such as `PowerShell`, `Linux shell`, or the placeholder words described below. Press Enter after typing a command and wait for it to finish before entering the next one.

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
| `baudbound config print` | Prints a complete example TOML template. It does not print the active file. |
| `baudbound config init` | Writes the starter template to the resolved path and refuses to replace an existing file. |
| `baudbound config init --force` | Replaces an existing config with the starter template. |
| `baudbound config set display.time-format 12-hour` | Uses a 12 hour clock for human readable desktop and CLI timestamps. |
| `baudbound config set display.time-format 24-hour` | Uses a 24 hour clock for human readable desktop and CLI timestamps. |

The time format command updates the active `config.toml` with an atomic file replacement. It does not alter stored Unix timestamps or JSON command output.

### Status and diagnostics

| Command | Behavior |
| --- | --- |
| `baudbound status` | Shows runner storage, supported targets, service health, and script totals. |
| `baudbound status --json` | Prints the same status as machine-readable JSON. |
| `baudbound doctor` | Checks configuration, storage, platform integration, and native desktop action support. |
| `baudbound doctor --json` | Prints diagnostic results as JSON. |
| `baudbound ui` | Explicitly opens the desktop application. |

### Updates

| Command | Behavior |
| --- | --- |
| `baudbound update check` | Checks the official BaudBound release feed and explains whether a newer version is available. |
| `baudbound update check --json` | Prints the current version, latest version, check time, and availability as JSON. |

This command only checks and reports. It does not replace the executable or launch an installer. Successful results are cached so automatic checks can respect `updates.check_interval_hours`.

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
| `baudbound script import PACKAGE` | Validates and installs a new `.bbs` package. It does not create network credentials. |
| `baudbound script update PACKAGE` | Replaces the installed package with a new package carrying the same manifest ID. Existing network tokens are preserved. |
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
| `baudbound script approve SCRIPT` | Approves the current package hash and declared access. New Webhook and WebSocket tokens are printed once. |
| `baudbound script revoke-approval SCRIPT` | Removes the current approval. |

Updating package content invalidates its previous approval.

### Triggers and execution

```text
baudbound script triggers [SCRIPT] [--json]
baudbound script run SCRIPT
baudbound script run SCRIPT --trigger TRIGGER [--payload-json JSON]
baudbound script dispatch-trigger SCRIPT TRIGGER [--payload-json JSON]
```

`baudbound script triggers` lists the triggers that belong to installed scripts. A trigger node ID identifies one specific trigger inside a script. It looks similar to `n-mrowrsh5`. Use the ID shown by this command instead of typing the trigger name or guessing an ID.

`baudbound script run SCRIPT` starts the script's Manual trigger. Use the second form when you need to start another trigger. `baudbound script dispatch-trigger` is an explicit form intended for tools that always provide a trigger ID. It performs the same installed script, approval, and runtime checks.

### Supplying trigger test data

A payload is the data delivered to a trigger when an event occurs. For example, a Webhook trigger normally receives request data from an HTTP request. The `--payload-json` option lets you provide test data yourself when starting that trigger from the CLI. This is useful when you want to test the graph without sending the real external event.

The option must be used together with `--trigger`. First list the script's trigger IDs:

```text
baudbound script triggers my-automation
```

Suppose the Webhook trigger ID is `n-webhook`. The following command works in PowerShell and a normal Linux shell. It starts that trigger and supplies two fields:

```powershell
baudbound script run my-automation --trigger n-webhook --payload-json '{"message":"hello","count":3}'
```

The JSON object becomes output data from the selected trigger. Nodes can read its fields with these variable references:

```text
{{n-webhook.message}}
{{n-webhook.count}}
```

Nested JSON objects can also be read. This payload:

```json
{
  "request": {
    "status": "ready"
  }
}
```

makes the value `ready` available as:

```text
{{n-webhook.request.status}}
```

An object exposes each top level field by name. A JSON array, string, number, or boolean does not have named fields, so the complete value is available as `{{n-webhook.payload}}`. Omitting `--payload-json` supplies no extra test data.

The JSON must be valid and must be passed as one shell argument. JSON quoting differs between PowerShell and Linux shells. Run history stores the resulting variable values. Do not put passwords, tokens, encryption keys, or other secrets in this option or in command history.

### Run history

```text
baudbound script logs [--script SCRIPT] [--limit NUMBER] [--json]
```

Without filters, this prints the 20 most recent stored runs. `--script` selects one installed script and `--limit` changes the maximum result count.

Human-readable run and log timestamps follow the shared time format. `--json` continues to return Unix timestamps for stable automation.

## Background service

```text
baudbound serve [OPTIONS]
```

Without overrides, `baudbound serve` uses `config.toml`, loads enabled and approved scripts, and remains active for listener-based triggers.

| Option | Behavior |
| --- | --- |
| `--dry-run` | Prints listener preflight status and exits without starting services. |
| `--json` | Uses JSON output for `--dry-run`. It is not a general service log format. |
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

The CLI `listen` command requires `--stdin` and accepts explicit test input. It does not install an operating-system hook. The Windows desktop background runner installs and reloads the native hotkey listener automatically. With `--json`, the CLI prints one JSON object per input event.

## Secret commands

| Command | Behavior |
| --- | --- |
| `baudbound secret generate-key` | Prints a new `BAUDBOUND_SECRET_KEY` assignment for headless secret encryption. |
| `baudbound secret list SCRIPT [--json]` | Lists declared secret names, types, requirements, and configured state without values. |
| `baudbound secret set SCRIPT NAME` | Prompts securely for a declared secret value. |
| `baudbound secret remove SCRIPT NAME` | Removes the configured value. |

Treat generated keys as credentials and do not store them in source control or command history. See [Secrets](secrets.md) for desktop and headless storage.

## Network trigger authentication commands

Webhook and WebSocket tokens belong to the installed trigger on this runner. The `.bbs` package does not contain them. Import and update do not create tokens. The approve command prints tokens for newly approved network triggers once. BaudBound then stores only a token hash, so the plaintext value cannot be requested later. The rotate command replaces a token and prints the replacement once.

| Command | Behavior |
| --- | --- |
| `baudbound trigger-auth list SCRIPT [--json]` | Lists Webhook and WebSocket node IDs, authentication state, and a short token ending. It never prints the token. |
| `baudbound trigger-auth rotate SCRIPT NODE_ID webhook [--json]` | Replaces one Webhook token and prints the new value once. |
| `baudbound trigger-auth rotate SCRIPT NODE_ID websocket [--json]` | Replaces one WebSocket token and prints the new value once. |
| `baudbound trigger-auth enable SCRIPT NODE_ID TYPE` | Requires the current token for the selected trigger again. |
| `baudbound trigger-auth disable SCRIPT NODE_ID TYPE --yes` | Removes token protection from the selected trigger after explicit confirmation. |

`TYPE` must be `webhook` or `websocket`. Use `list` to copy the exact `NODE_ID`. Save a rotated token in the client that calls the trigger. Do not put it in the script package or source control.

Disabling authentication does not automatically make a public listener start. A non-loopback bind still refuses unprotected triggers unless the matching unsafe public-bind override is enabled in `config.toml`.

## JSON output and errors

`--json` is available only on commands that explicitly list it. Human-readable output is not a stable machine interface. JSON commands write their document to standard output. Failures write an error and return a non-zero process exit code.

Validation, configuration, storage, policy, approval, secret, and execution failures also return non-zero. Use `baudbound COMMAND --help` to confirm syntax for the installed runner version.

Use [Script Management](script-management.md) for lifecycle procedures, [Background Service and Triggers](service-triggers.md) for `baudbound serve`, [Secrets](secrets.md) for key handling, and [Runs, Logs, and Troubleshooting](runs-logs-troubleshooting.md) when syntax is accepted but an operation is rejected.
