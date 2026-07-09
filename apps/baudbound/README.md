# BaudBound Runner

Combined runner for validating, inspecting, importing, serving, running, and logging installed scripts. The same binary is used for desktop and headless operation.

Command name:

```bash
baudbound
```

Current implemented commands:

```bash
cargo run -p baudbound -- validate path/to/script.bbs
cargo run -p baudbound -- inspect path/to/script.bbs
cargo run -p baudbound -- inspect path/to/script.bbs --json
cargo run -p baudbound -- doctor
cargo run -p baudbound -- doctor --json
cargo run -p baudbound -- ui
cargo run -p baudbound -- config path
cargo run -p baudbound -- config print
cargo run -p baudbound -- config init
cargo run -p baudbound -- config init --force
cargo run -p baudbound -- serve
cargo run -p baudbound -- serve --dry-run
cargo run -p baudbound -- serve --dry-run --json
cargo run -p baudbound -- serve --once --run-schedules-immediately
cargo run -p baudbound -- serve --webhooks --webhook-bind 127.0.0.1 --webhook-port 43891
cargo run -p baudbound -- serve --reload-interval-seconds 5
cargo run -p baudbound -- --config /etc/baudbound/runner.toml serve
```

Installed script lifecycle and execution commands live under the grouped `script` namespace:

```bash
cargo run -p baudbound -- script import path/to/script.bbs
cargo run -p baudbound -- script update path/to/script.bbs
cargo run -p baudbound -- script list
cargo run -p baudbound -- script status
cargo run -p baudbound -- script inspect <script-id-or-name>
cargo run -p baudbound -- script enable <script-id-or-name>
cargo run -p baudbound -- script disable <script-id-or-name>
cargo run -p baudbound -- script remove <script-id-or-name>
cargo run -p baudbound -- script approval <script-id-or-name>
cargo run -p baudbound -- script approve <script-id-or-name>
cargo run -p baudbound -- script revoke-approval <script-id-or-name>
cargo run -p baudbound -- script triggers
cargo run -p baudbound -- script triggers <script-id-or-name>
cargo run -p baudbound -- script dispatch-trigger <script-id-or-name> <trigger-node-id>
cargo run -p baudbound -- script dispatch-trigger <script-id-or-name> <trigger-node-id> --payload-json '{"body":"ok"}'
cargo run -p baudbound -- script run <script-id-or-name>
cargo run -p baudbound -- script run <script-id-or-name> --trigger <trigger-node-id>
cargo run -p baudbound -- script run <script-id-or-name> --trigger <trigger-node-id> --payload-json '{"body":"ok"}'
cargo run -p baudbound -- script logs --script <script-id-or-name>
cargo run -p baudbound -- script logs --json
```

Runner storage defaults to the platform user data directory. Set `BAUDBOUND_HOME` to use a custom storage root:

```bash
BAUDBOUND_HOME=/tmp/baudbound-runner cargo run -p baudbound -- script list
```

Runner configuration defaults to `<BAUDBOUND_HOME>/config.toml`. The runner creates this file automatically on first start. Set `BAUDBOUND_CONFIG` or pass `--config` to use a specific file:

```bash
cargo run -p baudbound -- config path
cargo run -p baudbound -- config print
cargo run -p baudbound -- config init --force
```

```toml
[runner]
name = "Main PC Runner"
trigger_reload_seconds = 2
# Empty means this OS default headless and desktop targets.
# For a Linux headless service, use ["Generic Headless", "Linux Headless"].
target_runtimes = []

[triggers]
schedules_enabled = true
file_watch_enabled = true
process_watch_enabled = true
serial_enabled = true
startup_enabled = true
webhooks_enabled = false
websockets_enabled = false

[serial.devices.main_controller]
port = "COM3"
baud_rate = 115200
data_bits = 8
parity = "none"
stop_bits = "1"
flow_control = "none"
read_mode = "line"
auto_reconnect = true
validate_usb_identity = false
auto_rebind_port = false
# vendor_id = "1A86"
# product_id = "7523"
# serial_number = ""
# manufacturer = ""
# product = ""

[webhooks]
bind = "127.0.0.1"
port = 43891
max_body_bytes = 1048576

[websockets]
bind = "127.0.0.1"
port = 43892
max_message_bytes = 1048576
```

Missing config files are initialized from the built-in template. Webhook hosting remains disabled unless `webhooks_enabled = true` is set or `serve --webhooks` is passed. `runner.trigger_reload_seconds` controls how often `serve` checks installed scripts for import/update/remove/enable/disable changes; it defaults to 2 seconds and can be overridden with `serve --reload-interval-seconds`.
The runner rejects packages whose `capabilities.json` target runtime is not in `runner.target_runtimes`. When `target_runtimes` is empty, the runner uses this operating system's default headless and desktop targets. For unattended headless deployments, set it explicitly, for example `["Generic Headless", "Linux Headless"]`, so desktop packages cannot be imported or started by mistake.
Serial Input Trigger nodes only store the logical `deviceId`. The runner resolves that id through `[serial.devices.<deviceId>]`, so the same package can map to different local COM/tty ports on different machines. If `auto_rebind_port = true`, the runner can recover when the OS moves a USB serial device to a different COM/tty port. That mode requires `validate_usb_identity = true`, `vendor_id`, and `product_id`; add `serial_number`, `manufacturer`, or `product` when multiple identical devices may be connected so the runner does not have to guess.

The `doctor` command reports native desktop backend support for the current machine and lists the node action types covered by each backend. Desktop actions are intentionally native-only: if a platform has no native backend for a node or a specific node option, the package is rejected during editor verification and runner import instead of falling back to shell-script hacks. Current Windows-only desktop nodes are Get Pixel Color, Get Active Window, and Window Focus. macOS also rejects the Mouse Click `back` and `forward` buttons until a native backend exists for those buttons.

The `script run` command starts from the script manual trigger by default. Use `--trigger` to execute from another trigger node such as webhook, file watch, serial input, or hotkey. Use `--payload-json` to pass simulated trigger input. Object payload fields are exposed as runtime output references, for example payload `{"body":"ok","json":{"status":"healthy"}}` for trigger `n-webhook` can be read as `{{n-webhook.body}}` and `{{n-webhook.json.status}}`.

Current execution supports trigger-selected graph execution, control flow, variable operations, delay, calculate, file read/write/copy/move/delete, process execution, shell commands, and text transform.

The `ui` command opens the Tauri desktop shell. The desktop screen shows runner health, installed scripts, script package details, declared permissions, trigger registrations, recent run history, service heartbeat data, trigger reload requests, service reload/stop requests, package import/update, script removal, script approval, enable/disable, and manual script runs. It uses the same runner core and storage paths as the CLI, so actions taken in the UI are immediately visible to `script status`, `script logs`, and the long-lived `serve` process.

The desktop UI frontend lives in `apps/baudbound/ui` and uses Vite, TypeScript, React, Tailwind, and shadcn-style local components:

```bash
pnpm --dir apps/baudbound/ui install
pnpm --dir apps/baudbound/ui build
cargo run -p baudbound -- ui
```

The `script status` command summarizes runner storage health. It checks installed package hashes, package loadability, approval freshness, enabled/disabled script counts, and active trigger counts. Use `script status --json` when another tool needs structured status data.

The `script enable` and `script disable` commands control whether a script participates in long-lived trigger services. Disabled scripts remain installed and can still be inspected, approved, updated, removed, or run explicitly from the CLI.

The `script triggers` command validates installed package hashes and lists the trigger registrations that the service and desktop app can use. Without a script name, only enabled installed scripts are included.

The `script dispatch-trigger` command is a development harness for trigger adapters. It resolves the installed script, builds a trigger event, and dispatches it through the same core path that schedule, webhook, serial, and desktop adapters will use.

The `serve` command hosts long-lived trigger listeners. Use `serve --dry-run` to preview which listener services would be active without opening ports, file watchers, serial devices, or background polling threads. It dispatches enabled `trigger.startup` nodes once when the runner service starts. It registers enabled `trigger.schedule` nodes and dispatches them on their configured intervals when schedules are enabled. It registers enabled `trigger.file_watch` nodes when file watching is enabled. It polls enabled `trigger.process_started` nodes and dispatches when a matching new process appears. It opens enabled `trigger.serial_input` nodes when serial triggers are enabled and the trigger `deviceId` has a matching runner-side serial device entry, then dispatches when serial data arrives. Webhook and WebSocket hosting are opt-in through config or `--webhooks` / `--websockets`; when enabled, matching webhook requests are available as `{{webhook-node.method}}`, `{{webhook-node.path}}`, `{{webhook-node.headers}}`, `{{webhook-node.query}}`, `{{webhook-node.body}}`, and `{{webhook-node.json}}`, while WebSocket messages expose `{{websocket-node.connection_id}}`, `{{websocket-node.message}}`, `{{websocket-node.headers}}`, `{{websocket-node.query}}`, and `{{websocket-node.json}}`. Pressing Ctrl+C requests graceful shutdown and lets active listener services stop cleanly.

For headless servers, run `baudbound serve` under your chosen process manager, then use the same binary and `BAUDBOUND_HOME` for `script import` / `script update` / `script list` commands. Importing, updating, removing, enabling, or disabling a script writes a reload signal into runner storage, so a running `serve` process reloads listener registrations on its next loop tick. The service also periodically reloads trigger registrations as a fallback, so manual storage changes are picked up without restarting the process. While it runs, `serve` writes `service-status.json` with its process id, heartbeat, reload time, and active listener service counts. Use your process manager's own commands to inspect, restart, and stop the background runner.

Approvals are bound to the exact installed package hash and declared permissions. Updating a package clears the previous approval, so changed scripts must be reviewed again before the runner applies permissive policy.

The `script logs` command reads persisted successful and failed run history from runner storage.
