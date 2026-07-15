---
title: Repository Architecture
description: Understand BaudBound applications, Rust crates, contracts, process boundaries, and state flow.
tags: [developers, architecture, rust, editor]
---
# Repository Architecture

BaudBound is one repository with a browser editor, a unified Rust runner, a React/Tauri desktop interface, seven Rust crates, shared JSON Schemas, deployment definitions, and development/release tools.

## Repository map

| Path | Responsibility |
| --- | --- |
| `apps/editor/` | Next.js visual editor, node registry, verification, simulation, package export, and editor container |
| `apps/baudbound/` | Unified `baudbound` CLI, headless service, native adapters, Tauri host, and desktop UI |
| `crates/` | Reusable package, security, storage, runtime, action, trigger, and orchestration libraries |
| `schemas/` | Public package and per-node JSON Schemas generated from editor definitions |
| `deploy/schemas/` | Schema-host container and Compose deployment |
| `tools/` | Development menu, guarded release helper, and Wiki.js publisher |
| `.github/workflows/` | Editor/schema images, runner CI/release, and wiki publication |
| `docs/wiki/` | Public documentation source and static navigation |

## Rust crate ownership

| Crate | Owns | Must not own |
| --- | --- | --- |
| `baudbound-script` | `.bbs` archive parsing, package documents, path/asset validation, and language model | Execution policy, installed package hashing, or platform side effects |
| `baudbound-security` | Permission/capability recalculation, risk, and policy validation | UI decisions or action implementation |
| `baudbound-storage` | SQLite schema, installed package copies, approvals, runs, variables, signals, and encrypted secret records | Package semantics or trigger loops |
| `baudbound-runtime` | Graph execution, templates, conditions, loops, variables, cancellation, reports, and redaction | OS-specific side effects |
| `baudbound-actions` | Cross-platform action adapters and action-handler contracts | Tauri UI or package approval |
| `baudbound-triggers` | Schedule, file, process, webhook, WebSocket, hotkey, startup, and serial listener services | Package installation or approval decisions |
| `baudbound-core` | Validation and orchestration across package, security, storage, runtime, actions, and triggers | Desktop presentation |

`apps/baudbound` composes the crates and supplies application-specific native desktop adapters, CLI output, Tauri commands, runner paths, service lifecycle, and update integration.

## Sources of truth

Avoid parallel hard-coded compatibility or contract lists.

| Contract | Source of truth | Derived consumers |
| --- | --- | --- |
| Editor nodes | `apps/editor/data/nodes/definitions/**` and registry | Palette, inspector, verification, simulation, node schemas, capability contract, docs coverage |
| Package shape | `schemas/*.schema.json`, package writer, and Rust package model | Editor export, runner parser, schema host |
| Node capabilities | Generated `crates/baudbound-security/contracts/node-capabilities.json` | Rust security recalculation |
| Platform support | Node `supportedTargetRuntimes` plus native runner adapters | Editor compatibility and runner validation |
| CLI | Clap structures in `apps/baudbound/src/cli.rs` | Help output and CLI documentation coverage |
| Runner configuration | `RunnerConfig` and serialized nested settings | Default TOML, desktop Config UI, documentation |
| Durable state | SQLite schema in `baudbound-storage` | Desktop dashboard, CLI, service coordination |
| Wiki navigation | `docs/wiki/navigation.json` | Wiki.js static navigation |

When a source changes, update generation, validation, tests, and documentation in the same change.

## Package-to-run execution flow

1. The editor derives program steps and security declarations from registered node definitions.
2. Export writes manifest, program, permissions, capabilities, optional editor data, and declared assets into `.bbs`; schemas remain external contract files rather than archive entries.
3. `baudbound-script` parses the archive with normalized paths and bounded documents.
4. `baudbound-security` recalculates permissions, capabilities, and risk from the program instead of trusting declarations.
5. `baudbound-core` checks package identity, target compatibility, installed hash, policy, approval, and required secret state.
6. A manual command or `baudbound-triggers` event selects an exact trigger node and payload.
7. `baudbound-runtime` validates edge execution orders and executes each selected output's destinations sequentially in that explicit order, delegating external actions through an action-handler trait.
8. `baudbound-actions` or an application native adapter performs the supported side effect.
9. Runtime cancellation and errors stop or branch execution according to graph contracts.
10. The final redacted report is persisted through `baudbound-storage` and exposed to CLI or desktop views.

This order ensures security and compatibility checks occur before native side effects.

## Process boundaries

### Desktop application

The installed `baudbound` executable hosts Tauri and the React UI. Tauri commands call typed Rust functions for dashboard reads and actions. The UI does not open SQLite or parse packages itself.

The desktop background runner is supervised inside the desktop application. Closing the window hides it to the tray; quitting stops the background runner and exits the process.

### Headless service

`baudbound serve` runs trigger listeners in the foreground. An external service manager may supervise that process. BaudBound does not install or control systemd, OpenRC, or runit automatically.

### Independent CLI commands

Commands such as import, approve, and enable run as separate processes. Durable changes go through SQLite. A running service polls durable reload signals and registration state.

### Live service control

The service publishes a loopback-only control descriptor in SQLite status. Reload and stop requests use a bounded, authenticated `baudbound-control-v1` TCP protocol with a random token, strict message size, loopback enforcement, and short timeouts.

SQLite remains the durable source of status and signals; IPC provides immediate live control. Tauri commands are the desktop UI bridge. These layers are intentionally separate.

## Configuration and reload

`config.toml` contains machine-specific settings. Startup initializes it atomically when missing and rejects invalid TOML or invalid field relationships.

Installed script changes can be detected through the configured trigger reload interval or an explicit reload signal. Configuration changes that affect listener construction require a service restart. Reload preserves eligible schedule deadlines where the registration is unchanged and rebuilds listener state for changed registrations.

## Storage and concurrency

`baudbound-storage` uses bundled SQLite with WAL mode, foreign keys, a busy timeout, and schema versioning. It stores package metadata and references while package bytes remain under `scripts/`.

The application serializes operations that must not overlap, while SQLite handles durable transactions between processes. This does not make duplicate service instances safe: listeners and hardware remain external exclusive resources.

See [Storage, Backups, and Recovery](../runner/storage-backups.md) for operator behavior.

## Cancellation and shutdown

Runtime execution checks a cancellation token before and after external actions and during waits. Trigger services own worker threads or sockets and expose orderly shutdown paths. Service stop prioritizes stopping new dispatch, closing listeners, and recording final status.

New listeners and actions must have bounded waits, cancellation behavior, contextual errors, and tests for shutdown and reload.

## Native platform boundary

Native actions use Rust libraries or operating-system APIs. BaudBound does not emulate unsupported platform behavior by generating PowerShell, shell scripts, or other command text.

If no production-quality native implementation exists for a platform, the node definition must narrow `supportedTargetRuntimes`, the runner must reject it, and tests and documentation must describe the limitation.

Continue with [Editor and Package Development](editor.md), [Runner Development](runner.md), and [Testing and CI](testing-ci.md).
