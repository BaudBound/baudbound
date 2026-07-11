---
title: Testing and CI
description: Select and run BaudBound editor, runner, desktop, schema, container, release, and wiki quality gates.
tags: [developers, testing, ci, quality]
---
# Testing and CI

BaudBound tests contracts at their owning layer and repeats cross-language checks where a change can break editor-to-runner compatibility. Run the narrow tests while developing and the complete affected quality gate before review.

## Rust workspace

From the repository root:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build -p baudbound --locked
```

The workspace includes unit and integration tests for package parsing, security abuse cases, SQLite migrations and lifecycle, graph execution, actions, triggers, core orchestration, CLI behavior, Tauri-facing application functions, and supported native adapters.

Windows and Linux CI both run the workspace. Platform-gated tests prove only the implementation compiled for that runner; they do not make unsupported native behavior portable.

## Editor

Install dependencies once:

```text
pnpm --dir apps/editor install --frozen-lockfile
```

Run:

```text
pnpm --dir apps/editor lint
pnpm --dir apps/editor typecheck
pnpm --dir apps/editor schemas:check
pnpm --dir apps/editor test
pnpm --dir apps/editor build
pnpm --dir apps/editor e2e
```

`schemas:check` regenerates node and program contracts in memory and fails when committed schemas or the Rust capability contract are stale. Editor contract tests cover package writing, schemas, verification, simulation, variables, runtime compatibility, and related serialization behavior.

Playwright exercises the built UI and should be used for interaction, responsive layout, canvas, modal, keyboard, and export changes.

## Desktop UI

```text
pnpm --dir apps/baudbound/ui install --frozen-lockfile
pnpm --dir apps/baudbound/ui typecheck
pnpm --dir apps/baudbound/ui test
pnpm --dir apps/baudbound/ui build
```

Vitest contracts cover view-model formatting and client behavior. Typecheck verifies Tauri payload assumptions on the TypeScript side. The production Vite build is also consumed by the Rust/Tauri bundle.

## Wiki and publisher

```text
pnpm --dir tools/wiki-publisher install --frozen-lockfile
pnpm --dir tools/wiki-publisher test
pnpm --dir tools/wiki-publisher validate
```

Publisher tests cover metadata, links, assets, navigation, GraphQL safety, ownership, adoption, deletion limits, and reconciliation. Validation checks every managed page and documentation coverage contract before publication.

## GitHub workflows

| Workflow | Purpose |
| --- | --- |
| **Runner CI** | Windows/Linux Rust workspace, editor contract, and desktop UI gates |
| **Runner Release** | Full quality gate, version verification, signed Windows/Linux packages, updater metadata, and draft release |
| **Editor Docker** | Build and publish the editor container image |
| **Schemas Docker** | Verify generated schemas and build the schema-host image |
| **Wiki Documentation** | Validate and reconcile repository pages and static navigation with Wiki.js |

### Why runner CI includes the editor contract

The editor produces the package language consumed by Rust. A runner change can expose a mismatch in action types, capabilities, schemas, runtime outputs, target names, or package versions. Running editor contract checks in Runner CI prevents either language from being treated as an isolated product.

## Change-to-test matrix

| Change | Minimum required gates |
| --- | --- |
| Editor UI only | editor lint, typecheck, tests, build; Playwright for interaction/layout |
| Node definition | editor schemas, tests, typecheck, build; Rust security/runtime tests; documentation coverage |
| Package or schema contract | editor schema and package tests; full Rust workspace; schema image |
| Rust crate | fmt, clippy, affected tests, then workspace tests |
| Native action or trigger | workspace tests on affected platforms; target compatibility; editor contract |
| SQLite schema | storage migration tests, core/CLI lifecycle tests, backup documentation review |
| CLI command | Clap tests, CLI integration tests, CLI documentation coverage |
| Tauri command or payload | Rust tests, desktop UI typecheck/tests/build |
| Desktop responsive UI | desktop tests/build plus manual narrow/wide inspection |
| Container/deployment | image build, health check, clean deployment test, rollback review |
| Wiki page/navigation/publisher | publisher tests and validate, rendered wide/narrow inspection |
| Release tooling/version | release verification helper and full release quality gate |

## Development helper

On Windows, `./tools/development.ps1` opens an arrow-key menu for common development loops. It can launch the desktop app, UI, editor, headless service, status, installs, checks, tests, editor E2E, schemas, and builds.

Direct commands remain authoritative and are preferable in CI or when diagnosing one failing tool.

## Local and CI limitations

CI cannot prove every physical-device, desktop-session, window-manager, audio, serial, firewall, reverse-proxy, or updater condition. Tests should isolate deterministic contracts and reject unavailable native features clearly.

Before release, maintainers separately perform platform acceptance checks described in [Release Engineering](releases.md). Do not weaken a deterministic test because a physical environment is unavailable; separate the adapter contract from manual acceptance instead.

## Failure discipline

Fix warnings rather than suppressing them globally. Preserve the first useful error and add context at ownership boundaries. A flaky test must be diagnosed as a product or test-isolation defect, not merely retried until green.

Do not update snapshots, schemas, or generated contracts without reviewing the semantic change they record.
