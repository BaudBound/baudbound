---
title: Approvals, Capabilities, and Risk
description: Review BaudBound permissions, capabilities, calculated risk, policy gates, and per-revision approval.
tags: [security, approval, permissions, capabilities, risk]
---
# Approvals, Capabilities, and Risk

BaudBound describes program access in three related forms. **Permissions** are operator-facing operations with risk. **Capabilities** are machine-checkable runtime feature categories. **Risk** is the highest review level calculated from required permissions. Approval accepts one exact package revision; it does not disable validation.

## Risk levels

| Level | Meaning | Examples |
| --- | --- | --- |
| **Low** | Local workflow behavior with limited external impact | logging, delay, calculation, text transformation |
| **Medium** | Reads, controlled writes, communication, or visible desktop effects | file read/copy, HTTP request, clipboard, notification, serial write |
| **High** | Broad mutation, process/input control, persistent or global data, secrets, listeners | process launch, keyboard/mouse, file write, webhook bind, secret read |
| **Dangerous** | Arbitrary command behavior or irreversible operation requiring focused review | shell command, file deletion |

Risk is a review priority, not a verdict. A low-risk workflow can still disclose data through careless logging, and a high-risk workflow can be legitimate when its scope is understood.

## Permission reference

The runner derives permissions from executable action types and variable scopes. It rejects missing declarations, unused declarations, duplicates, or a declared risk lower or higher than the calculated value.

| Permission | Risk | Behavior that requires it |
| --- | --- | --- |
| `beep` | Low | Generate a system beep |
| `calculate` | Low | Evaluate supported arithmetic |
| `delay` | Low | Pause execution |
| `log` | Low | Write a runner log entry |
| `set_local_variable` | Low | Write run-scoped data |
| `text_transform` | Low | Transform text or structured text input |
| `webhook_response` | Low | Complete the current waiting webhook |
| `download_file` | Medium | Download network content to a file |
| `file_copy` | Medium | Copy a file |
| `file_move` | Medium | Move or rename a file |
| `file_read` | Medium | Read file contents |
| `http_request` | Medium | Send an outbound HTTP request |
| `open_application` | Medium | Open an application through a native adapter |
| `play_sound` | Medium | Play package or filesystem audio |
| `process_query` | Medium | Inspect process state |
| `screen_pixel_read` | Medium | Read a screen pixel on supported desktops |
| `serial_write` | Medium | Send data to a configured serial device |
| `set_persistent_variable` | Medium | Store script data between runs |
| `show_message_box` | Medium | Display an interactive desktop dialog |
| `show_notification` | Medium | Display a desktop notification |
| `websocket_write` | Medium | Write to a connection associated with a WebSocket run |
| `window_query` | Medium | Read active-window information |
| `write_clipboard` | Medium | Replace native clipboard content |
| `file_write_limited` | High | Create, overwrite, or append file content |
| `keyboard_control` | High | Send native keys or text |
| `mouse_control` | High | Move or click the native pointer |
| `process_kill` | High | Terminate a matching process |
| `read_secret` | High | Resolve runner-supplied secret declarations |
| `run_process` | High | Start an executable with arguments |
| `serial_input` | High | Read unattended input from a serial mapping |
| `set_global_variable` | High | Change runner-global data |
| `startup_trigger` | High | Start a script automatically when its service loads |
| `sub_script_run` | High | Execute another installed and approved script |
| `webhook_public_bind` | High | Register an inbound HTTP route |
| `websocket_public_bind` | High | Register an inbound WebSocket route |
| `window_focus` | High | Change foreground-window focus |
| `delete_file` | Dangerous | Permanently remove a file |
| `read_sensitive_file` | Dangerous | Read from an absolute, sensitive, or runtime-selected filesystem path |
| `run_shell_command` | Dangerous | Execute a command through a shell interpreter |
| `write_any_file` | Dangerous | Write to an absolute, sensitive, or runtime-selected filesystem path |

Trigger nodes such as Manual, Schedule, File Watch, Hotkey, and Process Started can be permissionless while still declaring machine-checkable capabilities and requiring service prerequisites.

File permissions depend on the configured path. A bounded relative path uses `file_read` or `file_write_limited`. An absolute path, a sensitive system location, or a path containing runtime variables requires `read_sensitive_file` or `write_any_file`. This makes path selection visible during approval instead of treating every file action as equally broad.

## Capability reference

Capabilities describe runtime subsystems, not user consent by themselves. Current categories are:

| Family | Capabilities |
| --- | --- |
| Action and data | `action.calculate`, `action.delay`, `action.log`, `action.text`, `runtime.variables`, `runtime.persistent_storage`, `runtime.secrets` |
| Files and network clients | `action.file`, `action.http` |
| Processes and scripts | `action.process`, `action.sub_script` |
| Desktop | `action.clipboard`, `action.keyboard`, `action.message_box`, `action.mouse`, `action.notification`, `action.pixel`, `action.sound`, `action.window` |
| Serial | `action.serial`, `trigger.serial_input` |
| Network replies/listeners | `action.webhook_response`, `action.websocket`, `trigger.webhook`, `trigger.websocket` |
| Control flow | `runtime.for_each`, `runtime.if`, `runtime.loop`, `runtime.switch`, `runtime.while` |
| Other triggers | `trigger.file_watch`, `trigger.hotkey`, `trigger.manual`, `trigger.process_started`, `trigger.schedule`, `trigger.startup` |

The editor node registry generates the node-to-capability contract embedded in the Rust security crate. During import, the runner recalculates required capabilities from `program.json` and compares them with package declarations. A package cannot gain access by declaring an unrelated capability, and it cannot omit one required by its program.

## Per-revision approval

Approval records:

- stable script identity;
- exact package hash;
- calculated permissions accepted during review; and
- approval time.

An approval is **current** only while the installed package hash and permission set match. Updating package content invalidates the previous approval. An unchanged display name does not preserve trust.

Revocation removes consent for the current revision without removing the package. The runner removes its active trigger registrations and rejects new manual, automatic, queued, and sub-script execution attempts until that revision is approved again.

Disabling a script is separate from revoking approval. A disabled script remains approved and installed, but the runner removes its active trigger registrations and rejects every new execution attempt. A run that was already executing is not forcefully cancelled.

## Review in the desktop application

1. Open **Scripts** or **Security**.
2. Select the script requiring attention.
3. Confirm package integrity and stable identity.
4. Compare the target runtime with the actual machine and session.
5. Review every node, permission, capability, risk level, and secret declaration.
6. Inspect paths, URLs, process targets, shell text, serial IDs, and listener routes in context.
7. Approve only when the revision matches the intended behavior.

The approval button remains blocked when validation, integrity, target, or policy checks make approval unsafe.

## Review with the CLI

Replace `SCRIPT` with the installed name or ID:

```text
baudbound script inspect SCRIPT
baudbound script approval SCRIPT
baudbound secret list SCRIPT
baudbound script approve SCRIPT
```

To revoke approval:

```text
baudbound script revoke-approval SCRIPT
```

Use `--json` on supported inspection commands for automation. Do not parse decorative table spacing.

## Runner policy gates

Security validation accepts policy flags for:

- dangerous actions;
- shell commands; and
- network-server triggers.

Shell commands have both dangerous-action and shell-specific gates. Webhook and WebSocket listeners have a network-server gate. A current approval allows the runner's intended approved execution path, but unsupported platforms, malformed configuration, package mismatches, missing secrets, and unavailable native adapters remain blocking conditions.

Policy is defense in depth. It must not be used to make package declarations inaccurate.

## Operator review checklist

- [ ] **Filesystem:** Are read, write, move, copy, download, and delete paths limited to intended data?
- [ ] **Processes:** Are executables, arguments, working directories, match modes, and kill targets exact?
- [ ] **Shell:** Is a native action unavailable, and has every expansion and quoting boundary been reviewed?
- [ ] **Input control:** Could keyboard, mouse, clipboard, or focus actions affect the wrong foreground application?
- [ ] **Network client:** Are URLs, headers, bodies, redirects, and returned data trusted appropriately?
- [ ] **Network server:** Are bind address, route, method, authentication, proxy, firewall, limits, and TLS intentional?
- [ ] **Serial:** Does the logical ID resolve to hardware whose USB identity and protocol settings are verified?
- [ ] **Secrets:** Are values configured only on the runner and kept out of package assets, variables, logs, and commands?
- [ ] **Persistence:** Can stored or global data influence later unattended runs safely?
- [ ] **Sub-scripts:** Is the child script independently installed, current, approved, and compatible?

Continue with [Security Model](index.md), [Secrets](../runner/secrets.md), and [Network Access](../runner/network-listeners.md).
