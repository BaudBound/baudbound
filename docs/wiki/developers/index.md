---
title: Developer Overview
description: BaudBound architecture, ownership boundaries, state, communication, and trust model.
tags: [developers, architecture]
---
# Developer Overview

This section is for people who want to build BaudBound from source, contribute a change, review architecture, publish documentation, or prepare a release. Product users do not need the developer tools described here.

BaudBound is a TypeScript and Rust workspace. A workspace is one repository containing several applications, libraries, tests, and build tools that are developed together. Changes should preserve explicit ownership boundaries, deterministic validation, native platform implementations, and tests proportional to behavior and risk.

## Applications

`apps/editor` is the Next.js visual editor. `apps/baudbound` is the unified runner CLI and Tauri desktop application. Its React UI lives under `apps/baudbound/ui`.

| Area | Owns | Start here |
| --- | --- | --- |
| Editor | Project authoring, registry definitions, verification, simulation, package export, generated schemas | [Editor and Package Development](editor.md) |
| Unified runner app | Clap commands, service process, Tauri commands, tray, updater, desktop native adapters | [Runner Development](runner.md) |
| Desktop UI | Operator views and typed Tauri client | [Runner Development](runner.md) |
| Schemas | Public package-document and per-node JSON Schemas | [BBS Package Format](../package-format/index.md) |
| Tools | Development menu, release helper, and Wiki.js publisher | [Contributing and Setup](setup.md) |
| Workflows | Cross-platform CI, release packaging, containers, schemas, and wiki publication | [Testing and CI](testing-ci.md) |

## Rust crates

`baudbound-script` owns package and language models plus archive validation. `baudbound-security` derives permissions, capabilities, and policy decisions. `baudbound-storage` owns SQLite schema and durable repositories. `baudbound-runtime` executes validated graphs. `baudbound-actions` implements actions. `baudbound-triggers` implements listener services. `baudbound-core` coordinates these domains.

Crate roots expose public APIs and delegate implementation into domain folders. Split unrelated responsibilities before files become difficult to review, but avoid abstractions that obscure ownership.

The normal dependency direction is:

```text
package models -> security/storage/runtime -> actions and triggers -> core orchestration -> CLI/Tauri
```

`baudbound-core` may coordinate lower-level crates. Lower-level crates must not reach upward into the application UI or command modules. See [Repository Architecture](architecture.md) for process and runtime flows.

## State and communication

SQLite is durable runner state and coordinates independent processes. Tauri commands bridge the desktop UI to Rust. TOML is operator configuration, and `.bbs` is the portable script artifact. JSON is used for cross-language package documents and explicit machine-readable output, not as a parallel mutable runner database.

## Trust boundary

The editor produces package declarations. The runner independently verifies package integrity, compatibility, requested access, approval, policy, secrets, and node configuration before native side effects. UI visibility never substitutes for backend authorization.

Unsupported platform behavior must be declared and rejected by both editor and runner rather than approximated with shell scripts.

## Sources of truth

- Editor node definitions own authoring metadata, config fields, outputs, access declarations, and supported target runtimes.
- Generated node schemas are build artifacts checked against those definitions.
- Rust package and security code independently enforce the execution contract. It does not trust editor output.
- `RunnerConfig` Rust structs own `config.toml` shape and defaults.
- Clap enums own CLI commands and options.
- Desktop `navigationGroups` owns operator tab IDs and labels.
- SQLite migrations own durable state. JSON files are not a mutable runner database.
- `docs/wiki/coverage.json` maps these product surfaces to mandatory public documentation.

When two layers need the same contract, derive or test it across the boundary. Do not add another handwritten compatibility table merely because it is convenient locally.

## Choose your next guide

- Set up a clean checkout with [Contributing and Development Setup](setup.md).
- Understand module and process boundaries in [Repository Architecture](architecture.md).
- Add a node or package field through [Editor and Package Development](editor.md).
- Add native runtime behavior through [Runner Development](runner.md).
- Select the required gates using [Testing and CI](testing-ci.md).
- Prepare signed artifacts with [Release Engineering](releases.md).
- Change public documentation with [Wiki Documentation](wiki-documentation.md).
