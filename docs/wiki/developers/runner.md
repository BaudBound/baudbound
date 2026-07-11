---
title: Runner Development
description: Extend the Rust runner, Tauri bridge, native actions, triggers, storage, and CLI.
tags: [developers, runner, rust]
---
# Runner Development

Keep `apps/baudbound/src/main.rs` limited to top-level parsing and dispatch. Commands live under `commands/`; service options, runtime, status, webhooks, and trigger loading live under `service/`. Tauri commands bridge UI requests into shared application services rather than reimplementing CLI behavior.

Crates define ownership boundaries. Avoid large crate-root implementation files and organize source into domain folders. Public APIs should expose validated types and narrow operations; storage, policy, and native adapters remain replaceable behind explicit interfaces.

Native actions must use Rust crates or operating-system APIs. Do not construct PowerShell, Bash, xdotool, or similar command scripts to simulate a native feature. If no production-quality implementation exists on a target, mark the node unsupported there and update editor definitions, generated contracts, runner checks, and tests.

Every run path must validate package integrity, compatibility, approval, policy, secrets, and node configuration before side effects. Error messages should identify script, run, and node without leaking secret values.

## Application modules

| Module | Responsibility |
| --- | --- |
| `cli.rs` | Clap command and option contract |
| `commands/` | Config, package, script, status, hotkey, and doctor task handlers |
| `service/` | Long-lived options, preflight, runtime loop, status, listeners, and trigger loading |
| `desktop_ui.rs` and modules | Tauri command state, dashboard models, background runner, updater bridge |
| `desktop_actions/` | Desktop-only native adapters and platform modules |
| `paths.rs` | Config and runner-home resolution |
| `output.rs` | Human and JSON CLI presentation |

Command handlers and Tauri commands should call shared core/application services. Do not maintain a desktop implementation and CLI implementation of the same lifecycle rule.

## Common change procedures

### Add an action

1. Define and document the editor contract.
2. Add security derivation and target restrictions.
3. Implement the action in the owning `baudbound-actions` domain module or a desktop native adapter.
4. Register it with core runtime dispatch.
5. Validate all config before side effects and return structured fallible outputs where the node contract requires them.
6. Add unit, runtime integration, rejection, cancellation, and supported-platform tests.

### Add a trigger

Define registration config and output payload, implement its service in `baudbound-triggers`, integrate loading/reloading in the app service, isolate listener failure, and test dispatch identity plus graceful shutdown. Network and device services require explicit resource limits.

### Add configuration

Add a serde field with a safe default to `baudbound-core::config`, validate cross-field invariants, include it in the generated template and desktop simple/advanced editor, decide whether service restart is required, and update [Configuration and Devices](../runner/configuration.md). Coverage fails when a serialized config field is absent from that page.

### Add durable state

Add a forward SQLite migration in `baudbound-storage`, repository methods, row conversions, transaction/error tests, backup implications, and UI/CLI access through shared services. Do not create JSON sidecar state or let callers edit SQL rows directly.

### Add a CLI or Tauri command

Clap owns command syntax and typed options. Tauri commands use serializable request/response DTOs and authorize in Rust. Update the typed TypeScript client and tests. CLI options are coverage-checked against [CLI Reference](../runner/cli-reference.md).

## Concurrency and lifecycle

Long-lived services must support cancellation, bounded work, graceful listener shutdown, and deterministic resource ownership. One runner home can be accessed by independent CLI processes through SQLite, while the active `serve` process receives live reload/control signals through authenticated loopback IPC. Tauri is the desktop UI bridge, not the service transport for headless clients.

Avoid holding database transactions across native calls or network waits. Trigger registration reload should replace changed registrations without duplicating active listeners. Errors in one script or registration must not terminate unrelated services.

## Error and secret handling

Add context at ownership boundaries: action type, node ID, script ID, listener, path category, or device ID. Do not include secret values, encryption keys, authorization headers, complete private payloads, or unredacted command environments. Test redaction both for success-path logs and nested errors.

Use [Repository Architecture](architecture.md) for the full execution and process flow and [Testing and CI](testing-ci.md) to select mandatory gates.
