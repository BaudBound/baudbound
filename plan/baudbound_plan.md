# BaudBound Project Plan

BaudBound is a local-first visual scripting automation platform.

Users create scripts in a hosted or self-hosted web editor, export them as `.bbs` files, and manually import them into a cross-platform Rust runner. The runner validates, stores, and executes scripts locally.

`.bbs` means **BaudBound Script**.

---

## 1. Core Product Direction

### Main Idea

BaudBound has two separate parts:

1. **BaudBound Editor**
   - Hosted by the project or self-hosted by users with Docker.
   - Runs as a web app.
   - Used to visually create scripts.
   - Exports `.bbs` script packages.
   - Does not connect to runners.
   - Does not execute scripts.

2. **BaudBound Runner**
   - Rust-based runtime.
   - Imports `.bbs` script packages manually.
   - Validates packages and script logic.
   - Shows permissions, capabilities, and risk level.
   - Executes scripts locally.
   - Must support headless systems, background service mode, and desktop agent mode.
   - Must work cross-platform: Windows, Linux, and macOS.

### Core Rule

```text
Editor builds scripts.
Runner owns execution.
```

The editor should never be treated as trusted. The runner must validate every imported script, even if it was exported from the official hosted BaudBound Editor.

---

## 2. Terminology

Use these names consistently throughout the codebase, UI, docs, and APIs.

| Term | Meaning |
|---|---|
| BaudBound | The whole platform/app ecosystem |
| BaudBound Editor | The web-based visual script builder |
| BaudBound Runner | The Rust script runtime |
| Script | An automation built by the user |
| `.bbs` | BaudBound Script package file |
| Script Package | A `.bbs` file containing script metadata, program, permissions, capabilities, and assets |
| Installed Script | A script imported into a runner |
| Script Runtime | The runner execution engine |
| Capability | Something the current runner mode/platform can technically support |
| Permission | Something the script wants to do that may require user approval |
| Risk Level | Low, medium, high, or dangerous classification calculated by the runner |

Avoid the word **macro** in UI and user-facing docs.

---

## 3. High-Level Architecture

```text
┌──────────────────────────────┐
│ BaudBound Editor              │
│                              │
│ Hosted or self-hosted web app │
│ Visual programming editor     │
│ No runner connection          │
│ No execution                  │
└──────────────┬───────────────┘
               │
               │ Export .bbs
               ▼
┌──────────────────────────────┐
│ .bbs Script Package           │
│                              │
│ manifest.json                 │
│ program.json                  │
│ permissions.json              │
│ capabilities.json             │
│ README.md                     │
│ assets/                       │
└──────────────┬───────────────┘
               │
               │ Manual import
               ▼
┌──────────────────────────────┐
│ BaudBound Runner              │
│                              │
│ Rust runtime                  │
│ Headless-first                │
│ CLI-first                     │
│ Service/daemon capable        │
│ Optional desktop agent/tray   │
└──────────────────────────────┘
```

---

## 4. Security Model

### Main Assumption

Every imported `.bbs` file may be malicious, corrupted, broken, outdated, or manually edited.

The runner must never trust:

- The hosted editor.
- A self-hosted editor.
- Browser-side validation.
- Package-declared permissions.
- Package-declared capabilities.
- The package file name.
- Asset paths inside the package.
- Any script downloaded from the internet.

### Import Security Pipeline

When importing a `.bbs` package, the runner must do this:

```text
Open .bbs safely
  ↓
Validate zip/package structure
  ↓
Check required files exist
  ↓
Validate JSON schemas
  ↓
Check format version
  ↓
Check minimum runner version
  ↓
Validate program AST
  ↓
Validate all node/action/trigger types
  ↓
Validate all config values
  ↓
Recalculate required capabilities
  ↓
Recalculate permissions
  ↓
Calculate risk level
  ↓
Check platform and runner mode support
  ↓
Check global runner security settings
  ↓
Show import summary to user
  ↓
Import disabled or import enabled
```

### Important Default Security Rules

- No live editor-to-runner connection by default.
- No exposed local API by default.
- No account system required.
- No LAN pairing system is required for the first production architecture.
- Runner validates all imported scripts.
- Imported high-risk scripts are disabled by default.
- Shell commands are blocked by default.
- File deletion is blocked by default.
- Startup triggers require explicit approval.
- Network/webhook server is disabled by default.
- Any network service binds to `127.0.0.1` by default.
- Runner must prevent path traversal in `.bbs` files.
- Runner must not execute files from `assets/` directly.
- Dangerous actions require explicit user approval.
- Headless runners must reject desktop-only scripts when enabled.

---

## 5. `.bbs` Package Format

A `.bbs` file is a zip-based package with a custom extension.

Example file names:

```text
work-startup.bbs
server-health-check.bbs
discord-helper.bbs
backup-monitor.bbs
```

Internal structure:

```text
work-startup.bbs
├─ manifest.json
├─ program.json
├─ permissions.json
├─ capabilities.json
├─ README.md
└─ assets/
```

### 5.1 `manifest.json`

Contains human-readable and versioning metadata.

Example:

```json
{
  "format_version": 1,
  "script_language_version": 1,
  "id": "67df0f35-5646-4384-9299-7533cc053e07",
  "name": "Work Startup",
  "description": "Opens work apps and prepares the desktop.",
  "author": "local-user",
  "website": "https://example.com",
  "repository": "https://github.com/username/scriptname",
  "created_with": "BaudBound Editor 1.0.0",
  "created_at": "2026-07-01T20:00:00Z",
  "updated_at": "2026-07-01T20:00:00Z",
  "tags": ["work", "startup", "desktop"],
  "minimum_runner_version": "0.1.0"
}
```

Required fields:

- `format_version`
- `script_language_version`
- `id`
- `name`
- `created_with`
- `created_at`
- `minimum_runner_version`

Optional fields:

- `description`
- `author`
- `website`
- `repository`
- `updated_at`
- `tags`

### 5.2 `program.json`

Contains the actual script program.

This must be a structured program format, not raw JavaScript, Python, Lua, or shell.

Example:

```json
{
  "entry": {
    "trigger": {
      "type": "manual"
    },
    "program": {
      "type": "block",
      "steps": [
        {
          "type": "let",
          "name": "status",
          "value": {
            "type": "string",
            "value": "warning"
          }
        },
        {
          "type": "switch",
          "value": {
            "type": "variable",
            "name": "status"
          },
          "cases": [
            {
              "match": {
                "type": "string",
                "value": "ok"
              },
              "steps": [
                {
                  "type": "action",
                  "action": "log",
                  "config": {
                    "message": "Everything is okay."
                  }
                }
              ]
            },
            {
              "match": {
                "type": "string",
                "value": "warning"
              },
              "steps": [
                {
                  "type": "action",
                  "action": "show_notification",
                  "config": {
                    "title": "Warning",
                    "message": "Status is warning."
                  }
                }
              ]
            }
          ],
          "default": [
            {
              "type": "action",
              "action": "log",
              "config": {
                "message": "Unknown status."
              }
            }
          ]
        }
      ]
    }
  }
}
```

### 5.3 `permissions.json`

Contains permissions as declared by the editor.

The runner must recalculate permissions itself and compare the result.

Example:

```json
{
  "declared_permissions": [
    "show_notification",
    "log"
  ],
  "risk_level": "low"
}
```

If declared permissions do not match runner-calculated permissions, the runner should show a warning.

Example warning:

```text
The package-declared permissions do not match the runner-calculated permissions.
This script may have been modified.
```

### 5.4 `capabilities.json`

Contains capabilities required by the script.

The runner must recalculate required capabilities itself and compare the result.

Example desktop script:

```json
{
  "required_capabilities": [
    "trigger.hotkey",
    "action.keyboard",
    "action.mouse",
    "action.window",
    "action.clipboard"
  ]
}
```

Example headless script:

```json
{
  "required_capabilities": [
    "trigger.schedule",
    "action.http",
    "action.file",
    "action.log"
  ]
}
```

### 5.5 `README.md`

Optional human-readable documentation.

Example:

```md
# Work Startup

This script opens work apps, checks clipboard content, and shows a notification.

## Required permissions

- Open applications
- Keyboard control
- Clipboard read
```

### 5.6 `assets/`

Optional directory for non-executable supporting files.

Examples:

```text
assets/
├─ sounds/notify.wav
├─ icons/work.png
└─ templates/message.txt
```

Asset rules:

- Assets must stay inside the package sandbox.
- No path traversal allowed.
- Reject paths containing `..` traversal.
- Reject absolute paths inside package entries.
- Runner must not execute assets directly.
- Assets should only be used as data.

---

## 6. Script Language Design

BaudBound scripts are a visual programming language represented as structured JSON.

The editor shows a node/block UI, but the exported package contains a clean AST-like program.

### 6.1 Core Script Concepts

A script contains:

- Trigger
- Program block
- Variables
- Expressions
- Actions
- Control flow
- Permissions
- Capabilities
- Logs

### 6.2 Data Types

Editor variable types:

- `string`
- `number`
- `boolean`
- `list`
- `http_response`
- `datetime`
- `duration`
- `file_path`

Runtime output data may also expose structured objects that are read-only and referenced directly from the node id.

Runtime output types:

- `process_result`
- `window_handle`
- `pixel_color`
- `serial_data`
- `file_content`
- `secret`
- `binary`

### 6.3 Variables

Variable scopes:

1. **Runtime variables**
   - Exist only during one script run.
   - Current editor default.

2. **Persistent variables**
   - Stored between runs.
   - Supported by the production variable model.

3. **Global variables**
   - Configured per runner and available to scripts.
   - Supported by the production variable model.

4. **Secret variables**
   - Sensitive values like API keys.
   - Supported by the production variable model with encryption.

Variable references use double braces:

```text
{{status}}
{{manifest_name}}
{{system_os}}
{{n-mr3zyt6f-12.status_code}}
```

Built-in variables and node output variables are read-only.

Reserved user variable prefixes:

- `manifest_`
- `system_`

Variable operations:

- Set variable
- Increment variable
- Append to list
- Set object field
- Get object field
- Clear variable

### 6.4 Expressions

Expressions are used in if/else, switch, loops, and action configs.

Boolean/comparison operations:

- `==`
- `!=`
- `>`
- `>=`
- `<`
- `<=`
- `and`
- `or`
- `not`
- `contains`
- `starts_with`
- `ends_with`
- `regex_match`
- `in_list`
- `is_null`
- `is_empty`

Math operations:

- `+`
- `-`
- `*`
- `/`
- `%`
- `round`
- `floor`
- `ceil`
- `min`
- `max`
- `random`

String operations:

- `trim`
- `lowercase`
- `uppercase`
- `replace`
- `split`
- `join`
- `substring`
- `format`

Object/JSON operations:

- Get field
- Set field
- JSON path

### 6.5 Control Flow

Current control flow:

- If / Else
- Switch
- Loop

Production control flow backlog:

- For-each

### 6.6 Sub-Scripts

Sub-scripts allow one installed script to call another script on the same runner.

Sub-script execution should support:

- Name
- Description
- Script selector
- Optional input values
- Runtime output data
- Success and failed outputs

Example concept:

```text
Sub-script: Send Notification If Error
Inputs:
  - status: string
  - message: string
Output:
  - was_error: boolean
```

### 6.7 Error Handling

Current editor direction:

- Nodes that can fail expose `success` and `failed` outputs.
- Failed nodes expose a read-only `error` runtime object.
- `error` contains `message`, `code`, `type`, `retryable`, and `details`.
- Runner implementations may still stop a script on unhandled failures.

---

## 7. Capabilities

Capabilities describe what the runner can technically support.

Capabilities are not the same as permissions.

- Capability: Can this runner technically do this?
- Permission: Should this script be allowed to do this?

### 7.1 Trigger Capabilities

- `trigger.manual`
- `trigger.schedule`
- `trigger.hotkey`
- `trigger.file_watch`
- `trigger.webhook`
- `trigger.websocket`
- `trigger.serial_input`
- `trigger.startup`
- `trigger.process_started`

### 7.2 Action Capabilities

- `action.log`
- `action.delay`
- `action.notification`
- `action.message_box`
- `action.http`
- `action.file`
- `action.process`
- `action.keyboard`
- `action.mouse`
- `action.clipboard`
- `action.window`
- `action.sound`
- `action.text`
- `action.calculate`
- `action.pixel`
- `action.serial`
- `action.sub_script`

### 7.3 Runtime Capabilities

- `runtime.variables`
- `runtime.if`
- `runtime.switch`
- `runtime.loop`
- `runtime.for_each`
- `runtime.sub_script`
- `runtime.error_handling`
- `runtime.persistent_storage`
- `runtime.secrets`

### 7.4 Example Headless Linux Runner Capabilities

```json
{
  "mode": "headless",
  "platform": "linux",
  "capabilities": [
    "trigger.manual",
    "trigger.schedule",
    "trigger.file_watch",
    "trigger.webhook",
    "trigger.websocket",
    "trigger.serial_input",
    "action.log",
    "action.delay",
    "action.http",
    "action.file",
    "action.process",
    "action.serial",
    "runtime.variables",
    "runtime.if",
    "runtime.switch",
    "runtime.loop",
    "runtime.for_each"
  ]
}
```

### 7.5 Example Windows Desktop Agent Capabilities

```json
{
  "mode": "desktop_agent",
  "platform": "windows",
  "capabilities": [
    "trigger.manual",
    "trigger.schedule",
    "trigger.hotkey",
    "trigger.file_watch",
    "trigger.startup",
    "trigger.process_started",
    "action.log",
    "action.delay",
    "action.http",
    "action.file",
    "action.process",
    "action.keyboard",
    "action.mouse",
    "action.clipboard",
    "action.window",
    "action.pixel",
    "action.sound",
    "action.notification",
    "runtime.variables",
    "runtime.if",
    "runtime.switch",
    "runtime.loop",
    "runtime.for_each"
  ]
}
```

---

## 8. Permissions and Risk Levels

### 8.1 Permission Categories

Low risk permissions:

- `log`
- `delay`
- `beep`
- `math`
- `calculate`
- `text_transform`
- `set_local_variable`
- `read_runtime_data`

Medium risk permissions:

- `show_notification`
- `show_message_box`
- `http_request`
- `download_file`
- `file_read`
- `file_copy`
- `file_move`
- `write_clipboard`
- `open_application`
- `window_query`
- `process_query`
- `serial_write`
- `keyboard_control`
- `mouse_control`
- `screen_pixel_read`
- `play_sound`
- `file_write_limited`

High risk permissions:

- `run_shell_command`
- `delete_file`
- `read_sensitive_file`
- `write_any_file`
- `read_clipboard`
- `startup_trigger`
- `webhook_public_bind`
- `websocket_public_bind`
- `serial_input`
- `window_focus`
- `process_kill`
- `sub_script_run`

### 8.2 Risk Levels

Use four risk levels:

1. `low`
2. `medium`
3. `high`
4. `dangerous`

Suggested mapping:

| Risk | Examples |
|---|---|
| Low | Log, delay, calculate, format text, if/switch |
| Medium | HTTP request, notification, message box, file read/copy/move, clipboard write |
| High | Keyboard/mouse control, file write, startup trigger, serial input, process kill |
| Dangerous | Shell command, file deletion, write-anywhere behavior, read sensitive files |

### 8.3 Import Behavior by Risk

Low risk:

- Can import enabled if user chooses.

Medium risk:

- Show warning.
- Can import enabled after confirmation.

High risk:

- Import disabled by default.
- User can manually enable after reviewing.

Dangerous:

- Import disabled.
- Requires explicit advanced approval.
- Some categories disabled globally by default.

---

## 9. BaudBound Runner Design

The runner must be headless-first and CLI-first.

Desktop UI is optional and should be layered on top of the core runtime.

### 9.1 Runner Layers

```text
baudbound-core
- Script parser
- Package reader
- Schema validator
- Program validator
- Permission analyzer
- Capability checker
- Execution engine
- Logging abstractions

baudbound-storage
- SQLite storage
- Installed scripts
- Logs
- Settings
- Permissions
- Script hashes

baudbound-cli
- Import
- Validate
- Run
- Enable/disable
- Logs
- Config

baudbound-daemon
- Headless service mode
- Schedule triggers
- File watch triggers
- Webhook triggers

baudbound-agent
- Desktop user-session mode
- Hotkeys
- Keyboard/mouse/window/clipboard

baudbound-tray
- Optional tray UI
- Pause/resume
- Quick logs
- Import script
```

### 9.2 Runner Modes

#### CLI Mode

Used for manual execution and headless management.

Examples:

```bash
baudbound validate ./server-health-check.bbs
baudbound import ./server-health-check.bbs
baudbound list
baudbound run server-health-check
baudbound logs server-health-check
baudbound enable server-health-check
baudbound disable server-health-check
```

#### Headless Daemon/Service Mode

For servers, NAS machines, VPS machines, Docker-like server environments, and background automation.

Supports:

- Manual CLI runs
- Schedule triggers
- File watch triggers
- Webhook triggers
- HTTP actions
- File actions
- Process actions if allowed
- Logs
- Variables
- If/switch/loops

Does not require GUI.

Does not support normal desktop automation unless a desktop session/agent exists.

#### Desktop Agent Mode

For normal desktop automation.

Supports everything headless supports, plus:

- Global hotkeys
- Keyboard input
- Mouse input
- Window control
- Clipboard access
- Desktop notifications
- Tray icon
- Local approval dialogs

This should run in the logged-in user session.

#### Tray/Background Mode

For normal desktop users.

- Starts on login.
- Sits in the tray.
- Executes enabled scripts.
- Can pause all scripts.
- Can import scripts.
- Can show logs/settings.

---

## 10. Runner CLI Commands

Command name:

```bash
baudbound
```

Recommended command set:

```bash
baudbound --version
baudbound help

baudbound validate ./script.bbs
baudbound inspect ./script.bbs
baudbound import ./script.bbs
baudbound import ./script.bbs --disabled
baudbound import ./script.bbs --enable

baudbound list
baudbound show <script-id-or-name>
baudbound enable <script-id-or-name>
baudbound disable <script-id-or-name>
baudbound remove <script-id-or-name>

baudbound run <script-id-or-name>
baudbound stop <run-id>
baudbound stop-all

baudbound logs
baudbound logs <script-id-or-name>
baudbound logs --follow
baudbound logs --since 24h

baudbound config show
baudbound config path
baudbound config set <key> <value>
baudbound config edit

baudbound service install
baudbound service uninstall
baudbound service start
baudbound service stop
baudbound service status

baudbound safe-mode enable
baudbound safe-mode disable

baudbound backup create
baudbound backup restore <file>
```

---

## 11. Runner Configuration

Use a TOML config file.

File name:

```text
baudbound.toml
```

### 11.1 Example Headless Config

```toml
[runner]
name = "NutVault Runner"
mode = "headless"
start_paused = false

[storage]
database_path = "default"
script_package_dir = "default"
backup_dir = "default"

[execution]
max_concurrent_scripts = 4
default_script_timeout_seconds = 300
default_step_timeout_seconds = 30
same_script_policy = "ignore"

[triggers]
manual_enabled = true
schedules_enabled = true
file_watch_enabled = true
webhooks_enabled = false
hotkeys_enabled = false
startup_enabled = false

[network]
webhook_bind_ip = "127.0.0.1"
webhook_port = 43891
local_api_enabled = false
local_api_bind_ip = "127.0.0.1"
local_api_port = 43892

[security]
safe_mode = false
block_shell_commands = true
block_network_requests = false
block_file_delete = true
require_approval_for_risky_scripts = true
disable_imported_risky_scripts = true

[logs]
level = "info"
retention_days = 30
max_size_mb = 250
```

### 11.2 Example Desktop Agent Config

```toml
[runner]
name = "Main PC Runner"
mode = "desktop_agent"
start_paused = false

[desktop]
tray_enabled = true
notifications_enabled = true
keyboard_mouse_enabled = true
clipboard_enabled = true
window_control_enabled = true

[triggers]
manual_enabled = true
schedules_enabled = true
hotkeys_enabled = true
file_watch_enabled = true
webhooks_enabled = false

[security]
safe_mode = false
block_shell_commands = true
block_file_delete = true
require_approval_for_risky_scripts = true
disable_imported_risky_scripts = true
```

---

## 12. Runner Desktop UI

The runner must not contain a script editor.

Desktop UI should only manage installed scripts, settings, logs, and imports.

Pages:

- Installed Scripts
- Import Script
- Script Details
- Permissions
- Logs
- Settings
- About

### Installed Scripts Page

```text
[Enabled]  Work Startup
[Disabled] Server Health Check
[Enabled]  Discord Helper

Actions:
- Import
- Run
- Enable
- Disable
- Remove
- View logs
```

### Script Details Page

Show:

- Name
- Description
- Version
- Risk level
- Required capabilities
- Required permissions
- Triggers
- Last run
- Last error
- Package hash
- Program hash

### Settings Page

Sections:

- General
- Execution
- Security
- Triggers
- Network
- Storage
- Logs
- Desktop integration

---

## 13. Storage Design

Use SQLite.

Suggested tables:

```sql
installed_scripts
- id
- script_id
- name
- description
- version
- enabled
- risk_level
- package_hash
- program_hash
- manifest_json
- program_json
- permissions_json
- capabilities_json
- imported_at
- updated_at

script_permissions
- id
- script_id
- permission
- risk_level
- approved
- approved_hash
- approved_at

script_runs
- id
- script_id
- status
- started_at
- finished_at
- error_message

script_logs
- id
- run_id
- script_id
- level
- message
- created_at

runner_settings
- key
- value

persistent_variables
- id
- script_id
- name
- value_json
- updated_at

secrets
- id
- name
- encrypted_value
- created_at
- updated_at
```

Production storage should support persistent variables and secrets from the beginning, even if some runner modes
initially leave them disabled by configuration.

---

## 14. BaudBound Editor Design

The editor is a web app.

It can be used as:

1. Public hosted version.
2. Self-hosted Docker container.

The hosted/self-hosted editor should not require an application backend. Browser-local state is acceptable for editor
drafts, but exported `.bbs` packages must remain the portable source of truth.

### 14.1 Recommended Stack

- React or Svelte
- TypeScript
- Vite
- React Flow or Svelte Flow
- Zod or TypeBox for schema validation
- JSZip for `.bbs` export/import
- Docker image served by nginx or Caddy

### 14.2 Docker Example

```yaml
services:
  baudbound-editor:
    image: baudbound/editor:latest
    container_name: baudbound-editor
    restart: unless-stopped
    ports:
      - "8080:80"
```

### 14.3 Editor Surfaces

- Editor canvas
- Block library
- Properties inspector
- Simulator panel
- Bottom console with output, variables, and serial tabs
- Project settings modal
- Help/documentation modal
- Verification modal
- Export wizard
- Import flow for `.bbs` packages

Optional supporting surfaces:

- Local browser library
- Script templates
- Theme/settings

### 14.4 Current Editor Baseline

- Visual script editor
- React Flow based node canvas
- Resizable block library, inspector, and bottom console
- Project settings modal for manifest metadata
- Target runtime selector
- Verification status badge
- Verification progress modal
- Export wizard with project, access, and verification steps
- Import verification before loading `.bbs` packages
- Custom canvas/node/edge context menus
- Node copy, paste, duplicate, and delete controls
- Keyboard shortcuts for node copy/paste
- Runtime context references using `{{name}}` and `{{node-id.data_name}}`
- Read-only built-in variables and node output variables
- Permission preview
- Capability preview
- Risk calculation
- Simulator tab
- Serial console tab

### 14.5 Production Editor Feature Set

The editor should be designed as the real production tool from the start.

Required production features:

- Clean node catalog with trigger, control, runtime, file, network, desktop, sound, serial, and process groups.
- Reusable, consistent shadcn-based controls.
- Structured node configuration, not one-off field handling.
- Type-safe variable creation and validation.
- Read-only protection for built-in variables and node output references.
- Verification rules that block invalid export.
- Import flow that verifies and reconstructs editable graph state from valid `.bbs` packages.
- Export flow that compiles graph state into a runner-oriented program format.
- Simulator panel for validating behavior without a runner connection.
- Serial console panel for serial input/output tooling.
- Help modal with controls, reference formats, built-in variables, and runtime data documentation.
- Production-quality accessibility, focus states, resize behavior, and responsive sizing.

### 14.6 Target Runtime Selector

The editor should ask what type of script the user is building.

Options:

- Generic headless
- Linux headless
- Windows headless/service
- macOS background
- Generic desktop
- Windows desktop
- Linux desktop
- macOS desktop

When user selects a headless target, editor should hide or warn for:

- Keyboard nodes
- Mouse nodes
- Window control nodes
- Hotkey trigger
- Desktop notification nodes if unsupported
- Clipboard nodes if unsupported

When user selects a desktop target, allow desktop nodes.

---

## 15. Import Flow in Runner

When user imports a `.bbs` file:

```text
1. User selects .bbs file
2. Runner opens package safely
3. Runner checks required files
4. Runner validates JSON schemas
5. Runner checks format version
6. Runner checks minimum runner version
7. Runner calculates package hash
8. Runner validates program AST
9. Runner calculates permissions
10. Runner calculates capabilities
11. Runner checks compatibility with current runner mode
12. Runner checks global security settings
13. Runner shows import summary
14. User chooses import disabled or import enabled
15. Runner stores script in SQLite
16. Runner registers triggers if enabled
```

### Compatible Import Example

```text
Script: Server Health Check
Format: BaudBound Script v1
Risk: Medium
Target: Generic headless

Required capabilities:
- Schedule trigger
- HTTP request
- Log

Required permissions:
- Send HTTP requests
- Write logs

Compatibility:
✓ This runner supports all required capabilities.

Actions:
[Import disabled] [Import and enable] [Cancel]
```

### Incompatible Import Example

```text
Script: Discord Hotkey Reply
Risk: High

Required capabilities:
- Hotkey trigger
- Keyboard control
- Window control

Compatibility:
✗ This runner is running in headless mode.

Missing:
- Hotkey trigger
- Keyboard control
- Window control

Actions:
[Import disabled] [Cancel]
```

---

## 16. Execution Engine

The runner should not execute visual graph data directly.

Execution flow:

```text
program.json
  ↓
parse into AST
  ↓
validate AST
  ↓
compile execution plan
  ↓
run with runtime context
```

Runtime context should contain:

- Script ID
- Run ID
- Variables
- Permissions
- Logger
- Timeout settings
- Platform capabilities
- Trigger payload
- Cancellation token

Execution pipeline:

```text
Start script run
  ↓
Create runtime context
  ↓
Execute block
  ↓
Execute each step
  ↓
Evaluate expressions
  ↓
Branch if needed
  ↓
Log results
  ↓
Finish success/error
```

---

## 17. Triggers

### Current Triggers

- `manual`
- `schedule`
- `file_watch`
- `webhook`
- `hotkey`

### Production Trigger Backlog

- `startup`
- `process_started`
- `serial_input`
- `websocket`

---

## 18. Actions

### Current Runtime and Control Actions

- `set_variable`
- `if_else`
- `switch`
- `loop`

### Production Variable Operators

- `increment_variable`
- `append_list`
- `set_object_field`
- `get_object_field`
- `clear_variable`

### Current Core Actions

- `log`
- `delay`
- `show_notification`
- `http_request`
- `run_process`
- `open_application`
- `keyboard`
- `mouse_click`
- `clipboard`
- `shell_command`

### Production Data and Utility Actions

- `format_text`
- `calculate`
- `get_pixel_color`

### Production File Actions

- `read_file`
- `write_file`
  - Supports overwrite and append mode.
- `copy_file`
- `move_file`
- `delete_file`
- `download_file`

### Production Network and Serial Actions

- `serial_write`

### Production Desktop Actions

- `type_text`
- `move_mouse`
- `get_active_window`
- `focus_window`
- `get_pixel_color`
- `message_box`
- `beep`
- `play_sound`

### Production Process and Script Actions

- `check_process`
- `kill_process`
- `run_sub_script`
- `for_each`

Dangerous actions should be strongly gated by default and always rechecked by the runner on import.

### 18.1 Production Node Behavior Notes

`get_pixel_color`
- Reads a screen pixel at user-defined `x` and `y` coordinates.
- Supports absolute coordinates.
- Saves runtime output in multiple formats, including `hex`, `rgb`, `rgba`, and numeric channel values.

`read_file`
- Reads the selected file path.
- Saves content to runtime context.
- Runner must classify sensitive paths and apply stronger permissions when needed.

`write_file`
- Writes text to a selected file path.
- Supports overwrite mode and append mode.

`serial_input`
- Trigger that fires when a configured serial device outputs data.
- Runtime context should include raw bytes/text, port name, timestamp, and parsed line when line mode is enabled.

`serial_write`
- Writes configured data to a serial device.
- Supports variables in the payload.

`websocket`
- Trigger similar to webhook but backed by a WebSocket endpoint.
- Runtime context should include message payload, connection metadata, and headers where available.

`startup`
- Trigger that runs when the computer or runner service starts.
- Requires explicit approval on import.

`process_started`
- Trigger that runs when a configured app or process starts.

`move_mouse`
- Moves the mouse to `x` and `y`.
- Supports absolute and relative movement modes.

`type_text`
- Types configured text through the desktop input layer.

`get_active_window`
- Captures the active window metadata into runtime context.

`focus_window`
- Focuses a selected or matched window.

`check_process`
- Checks whether a selected process exists or matches expected state.

`kill_process`
- Stops a selected process.
- High risk and blocked unless explicitly allowed.

`run_sub_script`
- Runs another installed script on the same machine.
- Exposes success, failed, and sub-script output data.

`for_each`
- Iterates through each item in a list.
- Exposes the current item and index in runtime context while the loop is running.

`format_text`
- Produces formatted text from templates and string operations.

`calculate`
- Evaluates a configured expression and saves the result to runtime context.

`beep`
- Uses the computer or runner platform's built-in beeper function when available.

`message_box`
- Shows a user-facing message box.
- Supports configurable title, message, buttons, and type such as info, warning, or error.

`play_sound`
- Plays an audio file from a file path or from packaged assets.

`download_file`
- Downloads a file from a URL to a selected destination.

`delete_file`
- Deletes the selected file.
- Dangerous and blocked unless explicitly allowed.

`copy_file`
- Copies a file from one location to another.

`move_file`
- Moves a file from one location to another.

---

## 19. Platform Support Plan

### Windows

Provide two modes:

1. Windows Service
   - Runs at boot.
   - Good for headless/server-style scripts.
   - Should not be used for keyboard/mouse/window automation.

2. User-session Agent
   - Runs on login.
   - Supports tray icon.
   - Supports keyboard/mouse/window automation.

Use cases:

```text
Desktop automation → user-session agent
Server-style automation → Windows service
```

### Linux

Provide:

1. systemd system service
   - Headless mode.
   - Good for servers/NAS.

2. systemd user service
   - Desktop-user mode.
   - Better for hotkeys/desktop actions.

Also support direct CLI:

```bash
baudbound start --headless
```

### macOS

Provide:

1. LaunchDaemon
   - Background/headless-style tasks.

2. LaunchAgent
   - User-session desktop tasks.

Desktop automation requires user approval for accessibility-style permissions.

### Docker

Docker is useful for the editor.

Docker runner mode only makes sense for headless/server scripts.

Docker runner can support:

- Manual
- Schedule
- Webhook
- HTTP
- File actions inside mounted volumes
- Logs

Docker runner should not attempt normal keyboard/mouse automation.

---

## 20. Versioning

Version these independently:

- BaudBound Editor version
- BaudBound Runner version
- `.bbs` package format version
- Script language version
- Node/action schema version

Example manifest fields:

```json
{
  "format_version": 1,
  "script_language_version": 1,
  "minimum_runner_version": "0.1.0"
}
```

Runner import rules:

- Unsupported format version: reject import.
- Runner version too old: reject or warn.
- Unknown node type: reject import.
- Unknown action type: reject import.
- Invalid action config: reject import.

---

## 21. Recommended Repository Structure

Start with a monorepo because the editor, runner, schemas, and examples are tightly connected.

```text
baudbound/
├─ apps/
│  ├─ editor/
│  ├─ runner-cli/
│  ├─ runner-desktop/
│  └─ docs/
├─ crates/
│  ├─ baudbound-core/
│  ├─ baudbound-script/
│  ├─ baudbound-runtime/
│  ├─ baudbound-storage/
│  ├─ baudbound-actions/
│  ├─ baudbound-triggers/
│  └─ baudbound-security/
├─ schemas/
│  ├─ manifest.schema.json
│  ├─ program.schema.json
│  ├─ permissions.schema.json
│  └─ capabilities.schema.json
└─ examples/
   ├─ hello-world.bbs
   ├─ server-health-check.bbs
   ├─ work-startup.bbs
   └─ simple-if-switch.bbs
```

---

## 22. Recommended Tech Stack

### Editor

- React or Svelte
- TypeScript
- Vite
- React Flow or Svelte Flow
- Zod or TypeBox
- JSZip
- nginx or Caddy Docker image

### Runner

- Rust
- serde
- serde_json
- tokio
- sqlx or rusqlite
- clap
- tracing
- zip crate
- notify crate
- reqwest
- scheduler/cron crate

### Desktop Runtime Dependencies

- tray-icon
- global-hotkey
- enigo or platform-specific input libraries
- arboard for clipboard
- notify-rust or platform-specific notifications

---

## 23. Production Build Plan

### Stage 1: Production Script Package Format

Deliverables:

- `manifest.json` schema
- `program.json` schema
- `permissions.json` schema
- `capabilities.json` schema
- Example `.bbs` packages
- Script language specification
- Graph-to-program compiler contract
- Program-to-graph import contract

The script language should support the full production editor direction, including:

- Manual trigger
- Schedule trigger
- File watch trigger
- Webhook trigger
- Hotkey trigger
- Startup trigger
- Process started trigger
- Serial input trigger
- WebSocket trigger
- Runtime variables
- Persistent variables
- Global variables
- Secret variables
- If/else
- Switch
- Loop
- For-each
- Log
- Delay
- Show notification
- HTTP request
- File operations
- Process operations
- Desktop automation actions
- Serial actions
- Sound actions
- Sub-script execution

### Stage 2: Production Editor

Deliverables:

- Web editor app
- Docker image
- Visual node editor
- Target runtime selector
- Permission preview
- Capability preview
- Verification system
- Export wizard
- Export `.bbs`
- Import `.bbs` for editing
- Simulator panel
- Serial console panel
- Help/documentation modal
- Project settings modal
- Production node catalog

No accounts.
No backend.
No runner connection.

### Stage 3: Runner CLI

Deliverables:

```bash
baudbound validate <file.bbs>
baudbound inspect <file.bbs>
baudbound import <file.bbs>
baudbound list
baudbound run <script>
baudbound logs
```

Runtime support:

- Variables
- If/else
- Switch
- Loop
- For-each
- Log
- Delay
- HTTP request
- File actions
- Process actions
- Runtime context outputs
- Error branches

### Stage 4: Runner Storage and Logs

Deliverables:

- SQLite database
- Installed scripts table
- Run history
- Script logs
- Enable/disable scripts
- Config file
- Persistent variables
- Secret storage

### Stage 5: Headless Daemon

Deliverables:

- `baudbound start --headless`
- Schedule trigger
- File watch trigger
- Webhook trigger
- WebSocket trigger
- Serial input trigger
- Service install command
- Linux systemd support

This makes BaudBound useful on servers.

### Stage 6: Security System

Deliverables:

- Permission analyzer
- Capability analyzer
- Risk levels
- Block dangerous actions
- Import warnings
- Script hash tracking
- Global allow/block config
- Sensitive path detection
- Read-only built-in/runtime variable enforcement

### Stage 7: Desktop Agent

Deliverables:

- User-session agent
- Tray icon
- Hotkey trigger
- Keyboard action
- Mouse action
- Clipboard action
- Get pixel color
- Window/app actions
- Desktop notifications
- Message boxes
- Beep and sound playback

### Stage 8: Production Visual Programming

Deliverables:

- For-each
- Variable operators
- Format text
- Calculate
- Sub-scripts
- Persistent variables
- Secrets

---

## 24. First Example Scripts

Create these examples early:

### 1. Hello World

```text
Manual trigger -> Log message
```

### 2. Server Health Check

```text
Schedule trigger
  -> HTTP request
  -> If status ok/warning/error
  -> Log result
```

### 3. JSON API Monitor

HTTP request nodes expose parsed JSON as runtime output when the response is JSON, so a separate parse step is only
needed for non-HTTP text sources.

```text
  -> HTTP request
  -> Use HTTP json output
  -> Switch response json field value
  -> Log or notify
```

### 4. File Watch Logger

```text
File changed
  -> Log path
```

### 5. Work Startup

```text
Hotkey
  -> Open application
  -> Type text
```

### 6. Clipboard Formatter

```text
Hotkey
  -> Clipboard
  -> Format text
  -> Clipboard
```

### 7. Serial Monitor

```text
Serial input trigger
  -> Format text
  -> Log
  -> Serial write
```

### 8. Desktop Pixel Watch

```text
Hotkey
  -> Get pixel color
  -> If color matches
  -> MessageBox or Beep
```

### 9. File Download And Archive

```text
Schedule trigger
  -> Download file
  -> Copy file
  -> Move file
  -> Log result
```

### 10. Process Guard

```text
Startup trigger
  -> Check process
  -> If missing
  -> Open application or Run process
```

Examples should cover headless, serial, desktop, file, and process automation so the editor and runner are validated
against the full production direction.

---

## 25. Implementation Priorities

Build production foundations first, then expand each node family against those foundations.

Priority order:

1. `.bbs` package format
2. Script schema
3. Graph-to-program export compiler
4. Program-to-graph import loader
5. Verification rules
6. Permission/capability/risk analyzers
7. Runtime context and typed variables
8. Core trigger/control/action nodes
9. File, network, serial, process, desktop, and sound node families
10. Runner validate/import/execute pipeline
11. Runner CLI and daemon modes
12. SQLite logs, storage, persistent variables, and secrets

Desktop automation, serial support, and file/process actions should be designed with the same production contracts as
headless actions. Risky nodes can be gated by permissions and runner settings without removing them from the product
direction.

---

## 26. Final Direction

BaudBound should be built as:

```text
A local-first visual scripting automation platform.
```

Final structure:

```text
BaudBound Editor
- Web app
- Hosted by project or self-hosted with Docker
- Creates visual scripts
- Exports .bbs files
- No live runner connection
- No accounts required

BaudBound Runner
- Rust runtime
- Imports .bbs files
- Headless-first
- CLI-first
- Cross-platform
- Service/daemon capable
- Optional desktop agent/tray mode
- Validates and executes scripts safely
```

This design gives BaudBound:

- Lower security risk
- No complicated account system
- No LAN pairing system
- No browser-to-runner control channel
- Self-hostable editor
- Portable script files
- Headless support
- Desktop automation support
- Clear separation between building and running scripts
