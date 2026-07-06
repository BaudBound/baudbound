# BaudBound Runner CLI

Command-line interface for validating, inspecting, importing, listing, running, and logging installed scripts.

Command name:

```bash
baudbound
```

Current implemented commands:

```bash
cargo run -p baudbound-runner-cli -- validate path/to/script.bbs
cargo run -p baudbound-runner-cli -- inspect path/to/script.bbs
cargo run -p baudbound-runner-cli -- inspect path/to/script.bbs --json
cargo run -p baudbound-runner-cli -- import path/to/script.bbs
cargo run -p baudbound-runner-cli -- update path/to/script.bbs
cargo run -p baudbound-runner-cli -- list
cargo run -p baudbound-runner-cli -- list --json
cargo run -p baudbound-runner-cli -- inspect <script-id-or-name> --installed
cargo run -p baudbound-runner-cli -- approval <script-id-or-name>
cargo run -p baudbound-runner-cli -- approve <script-id-or-name>
cargo run -p baudbound-runner-cli -- revoke-approval <script-id-or-name>
cargo run -p baudbound-runner-cli -- triggers
cargo run -p baudbound-runner-cli -- triggers --script <script-id-or-name>
cargo run -p baudbound-runner-cli -- triggers --json
cargo run -p baudbound-runner-cli -- dispatch-trigger <script-id-or-name> <trigger-node-id>
cargo run -p baudbound-runner-cli -- dispatch-trigger <script-id-or-name> <trigger-node-id> --payload-json '{"body":"ok"}'
cargo run -p baudbound-runner-cli -- serve
cargo run -p baudbound-runner-cli -- serve --once --run-schedules-immediately
cargo run -p baudbound-runner-cli -- serve --webhooks --webhook-bind 127.0.0.1 --webhook-port 43891
cargo run -p baudbound-runner-cli -- --config /etc/baudbound/runner.toml serve
cargo run -p baudbound-runner-cli -- run <script-id-or-name>
cargo run -p baudbound-runner-cli -- run <script-id-or-name> --trigger <trigger-node-id>
cargo run -p baudbound-runner-cli -- run <script-id-or-name> --trigger <trigger-node-id> --payload-json '{"body":"ok"}'
cargo run -p baudbound-runner-cli -- logs
cargo run -p baudbound-runner-cli -- logs --script <script-id-or-name> --limit 5
cargo run -p baudbound-runner-cli -- logs --json
cargo run -p baudbound-runner-cli -- remove <script-id-or-name>
```

Runner storage defaults to the platform user data directory. Set `BAUDBOUND_HOME` to use a custom storage root:

```bash
BAUDBOUND_HOME=/tmp/baudbound-runner cargo run -p baudbound-runner-cli -- list
```

Runner configuration defaults to `<BAUDBOUND_HOME>/config.toml`. Set `BAUDBOUND_CONFIG` or pass `--config` to use a specific file:

```toml
[runner]
name = "Main PC Runner"

[triggers]
schedules_enabled = true
file_watch_enabled = true
serial_enabled = true
webhooks_enabled = false

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
# vendor_id = "1A86"
# product_id = "7523"

[webhooks]
bind = "127.0.0.1"
port = 43891
max_body_bytes = 1048576
```

Missing config files are allowed and use safe defaults. Webhook hosting remains disabled unless `webhooks_enabled = true` is set or `serve --webhooks` is passed.
Serial Input Trigger nodes only store the logical `deviceId`. The runner resolves that id through `[serial.devices.<deviceId>]`, so the same package can map to different local COM/tty ports on different machines.

The `run` command starts from the script manual trigger by default. Use `--trigger` to execute from another trigger node such as webhook, file watch, serial input, or hotkey. Use `--payload-json` to pass simulated trigger input. Object payload fields are exposed as runtime output references, for example payload `{"body":"ok","json":{"status":"healthy"}}` for trigger `n-webhook` can be read as `{{n-webhook.body}}` and `{{n-webhook.json.status}}`.

Current execution supports trigger-selected graph execution, control flow, variable operations, delay, calculate, file read/write/copy/move/delete, process execution, shell commands, and text transform.

The `triggers` command validates installed package hashes and lists the trigger registrations that a future daemon or desktop app would register. Without `--script`, only enabled installed scripts are included.

The `dispatch-trigger` command is a development harness for trigger adapters. It resolves the installed script, builds a trigger event, and dispatches it through the same core path that schedule, webhook, serial, and desktop adapters will use.

The `serve` command hosts long-lived trigger listeners. It registers enabled `trigger.schedule` nodes and dispatches them on their configured intervals when schedules are enabled. It registers enabled `trigger.file_watch` nodes when file watching is enabled. It opens enabled `trigger.serial_input` nodes when serial triggers are enabled and the trigger `deviceId` has a matching runner-side serial device entry, then dispatches when serial data arrives. Webhook hosting is opt-in through config or `--webhooks`; when enabled, matching requests are available to scripts as `{{webhook-node.method}}`, `{{webhook-node.path}}`, `{{webhook-node.headers}}`, `{{webhook-node.query}}`, `{{webhook-node.body}}`, and `{{webhook-node.json}}`.

For headless servers, run `baudbound serve` under a process manager such as `systemd`, then use the same binary and `BAUDBOUND_HOME` for import/update/list commands. The service periodically reloads trigger registrations, so imported, updated, removed, enabled, or disabled scripts are picked up without restarting the process.

Linux `systemd` templates are available in `deploy/runner/systemd`.

Approvals are bound to the exact installed package hash and declared permissions. Updating a package clears the previous approval, so changed scripts must be reviewed again before the runner applies permissive policy.

The `logs` command reads persisted successful and failed run history from runner storage.
