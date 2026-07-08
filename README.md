<div align="center">
  <img src="assets/logo.svg" alt="BaudBound" width="320"/>
  <br/><br/>

  <p><strong>Local-first visual automation scripting.</strong></p>
  <p>Build scripts in the web editor, export portable <code>.bbs</code> packages, and run them locally with the BaudBound runner.</p>

  ![Editor](https://img.shields.io/badge/Editor-Next.js-111827?style=for-the-badge&logo=nextdotjs&logoColor=white)
  ![Runner](https://img.shields.io/badge/Runner-Rust-b7410e?style=for-the-badge&logo=rust&logoColor=white)
  ![Package](https://img.shields.io/badge/Package-.bbs-e62d3e?style=for-the-badge)
  ![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey?style=for-the-badge)
  ![License](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)
</div>

---

## What Is BaudBound?

BaudBound is a visual scripting automation platform for building local automation scripts without turning the browser into a trusted runtime.

The project is split into two parts:

- **BaudBound Editor**: a hosted or self-hosted web app for visually creating, validating, simulating, and exporting `.bbs` script packages.
- **BaudBound Runner**: a local runtime that imports `.bbs` packages, validates them again, asks for required permissions, and executes scripts on the user's machine.

The core rule is simple:

```text
Editor builds scripts.
Runner owns trusted execution.
```

## V2 Direction

This repository is the V2 architecture. It replaces the older V1 serial-event Java application with a package-based editor and runner model.

V1 focused on mapping serial input directly to actions. V2 keeps serial automation as one use case, but expands BaudBound into a general visual automation system with:

- node-based script graphs
- reusable `.bbs` package format
- local-first runner execution
- explicit permissions, capabilities, and risk analysis
- browser-only simulation for previewing graph flow
- asset packaging for script resources such as audio files

## Current Status

The editor is the most complete part of V2. The Rust runner workspace now has shared crates, CLI and desktop app entry points, strict `.bbs` package loading, filesystem-backed import/update storage, approval-bound package hashes, persisted run logs, manual and selected-trigger execution, runtime variables, control flow, calculations, file/process/network actions, and headless action dispatch.

Do not treat exported editor packages as inherently trusted. The runner must enforce the schema, permissions, capabilities, filesystem safety, and platform rules.

## BaudBound Editor

The editor is a Next.js application in `apps/editor`.

Public hosted editor:

```text
https://editor.baudbound.app/
```

It currently supports:

- React Flow based visual node editor
- editor-only canvas comments for documenting graph intent
- project settings and manifest metadata
- `.bbs` import and export
- package assets managed fully client-side
- package verification before export
- browser-only simulator with runtime variables
- node runtime output references using `{{node-id.field}}`
- built-in read-only variables such as manifest and system values
- Docker image and Compose deployment
- browser E2E and package contract tests

### Run Locally

```bash
cd apps/editor
pnpm install
pnpm dev
```

The editor is browser-local and does not require a backend service.

### Verify The Editor

```bash
cd apps/editor
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm e2e
```

For the full release gate:

```bash
pnpm verify:release
```

### Docker

Build and run the editor image:

```bash
docker build -f apps/editor/Dockerfile apps/editor -t baudbound-editor
docker run --rm -p 3000:3000 -e NEXT_PUBLIC_EDITOR_URL=http://localhost:3000 baudbound-editor
```

Or use the Compose template:

```bash
docker compose -f apps/editor/compose.yaml up -d
```

The GitHub Actions workflow in `.github/workflows/editor-docker.yml` builds the editor container and publishes it to GitHub Container Registry for non-PR builds.

## Script Packages

`.bbs` means **BaudBound Script**.

A `.bbs` package is a zip-based script package that contains the script manifest, graph program, permissions, capabilities, editor metadata, and optional assets.

The schemas live in `schemas/`:

- `manifest.schema.json`
- `program.schema.json`
- `permissions.schema.json`
- `capabilities.schema.json`
- `editor.schema.json`

The editor validates packages for authoring quality and export safety. The runner must validate packages again before import and execution.

## Repository Layout

```text
apps/
  editor/           Web editor for building and exporting .bbs packages
  baudbound/        Combined desktop/headless runner app
crates/
  baudbound-core/       Shared runner orchestration
  baudbound-script/     .bbs package models and package reader
  baudbound-runtime/    Runtime context and execution primitives
  baudbound-security/   Permission, capability, and risk policy
  baudbound-actions/    Action adapter traits
  baudbound-triggers/   Trigger adapter traits
  baudbound-storage/    Storage abstractions
schemas/            JSON schemas for package contracts
assets/             Project logos and shared visual assets
```

### Runner Workspace

```bash
cargo check --workspace
cargo test --workspace
cargo run -p baudbound -- validate path/to/script.bbs
cargo run -p baudbound -- inspect path/to/script.bbs --json
cargo run -p baudbound -- script import path/to/script.bbs
cargo run -p baudbound -- script update path/to/script.bbs
cargo run -p baudbound -- script list
cargo run -p baudbound -- script status
cargo run -p baudbound -- script enable <script-id-or-name>
cargo run -p baudbound -- script disable <script-id-or-name>
cargo run -p baudbound -- script approval <script-id-or-name>
cargo run -p baudbound -- script approve <script-id-or-name>
cargo run -p baudbound -- script revoke-approval <script-id-or-name>
cargo run -p baudbound -- script triggers
cargo run -p baudbound -- script triggers <script-id-or-name>
cargo run -p baudbound -- script dispatch-trigger <script-id-or-name> <trigger-node-id>
cargo run -p baudbound -- serve
cargo run -p baudbound -- serve --webhooks
cargo run -p baudbound -- --config /etc/baudbound/runner.toml serve
cargo run -p baudbound -- script run <script-id-or-name>
cargo run -p baudbound -- script run <script-id-or-name> --trigger <trigger-node-id>
cargo run -p baudbound -- script run <script-id-or-name> --trigger <trigger-node-id> --payload-json '{"body":"ok"}'
cargo run -p baudbound -- script logs --script <script-id-or-name>
cargo run -p baudbound -- script remove <script-id-or-name>
```

The runner split is intentional:

- `apps/baudbound` owns user interface, CLI, desktop shell, and process/service entrypoints
- `baudbound-core` coordinates runner behavior
- `baudbound-script` owns package reading and script contract models
- `baudbound-storage` owns installed package storage
- action, trigger, storage, runtime, and security crates keep implementation families isolated

Runner storage defaults to the platform data directory and can be overridden with `BAUDBOUND_HOME`. Runner daemon configuration defaults to `<BAUDBOUND_HOME>/config.toml`, can be overridden with `BAUDBOUND_CONFIG` or `--config`, and currently supports trigger service toggles plus webhook bind settings:

```toml
[runner]
name = "Main PC Runner"

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
# vendor_id = "1A86"
# product_id = "7523"

[webhooks]
bind = "127.0.0.1"
port = 43891
max_body_bytes = 1048576

[websockets]
bind = "127.0.0.1"
port = 43892
max_message_bytes = 1048576
```

Serial Input Trigger nodes only store the logical `deviceId`. Runner TOML maps that id to the local serial port and hardware settings, which keeps exported packages portable across machines.

Use `baudbound script status` to check installed package hash health, package loadability, approval freshness, enabled script counts, and trigger counts.

For headless machines, run `baudbound serve` under your chosen process manager and use the same `BAUDBOUND_HOME` for CLI commands such as `script import`, `script update`, `script enable`, `script disable`, `script approve`, `script status`, and `script logs`. Script imports, updates, removals, enables, and disables write a reload signal into runner storage, so the background runner refreshes listener registrations on its next loop tick. The runner also periodically reloads registrations as a fallback, so manual storage changes are picked up without restarting the process. A running background process writes `service-status.json`; use your process manager's own commands to inspect or control it.

## Security Model

BaudBound is designed around explicit trust boundaries:

- The editor may help users build and simulate scripts, but it is not a trusted executor.
- The runner owns local execution and must reject invalid or unsafe packages.
- Script assets are packaged locally and should be validated by content, not just extension.
- Permissions and capabilities are part of the package contract and must be surfaced before execution.
- User-facing automation should be understandable, auditable, and reversible where possible.

---

<div align="center">
  <img src="assets/logo-notext.svg" alt="BaudBound" width="64"/>
</div>
