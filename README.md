<div align="center">
  <img src="assets/logo.svg" alt="BaudBound" width="320" />
  <br /><br />
  <p><strong>Local-first visual automation scripting.</strong></p>
  <p>Build workflows in the web editor, export portable <code>.bbs</code> packages, and execute them with the native runner.</p>

  ![Editor](https://img.shields.io/badge/Editor-Next.js-111827?style=for-the-badge&logo=nextdotjs&logoColor=white)
  ![Runner](https://img.shields.io/badge/Runner-Rust-b7410e?style=for-the-badge&logo=rust&logoColor=white)
  ![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux-lightgrey?style=for-the-badge)
  ![License](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)
</div>

## Overview

BaudBound separates visual authoring from trusted native execution:

```text
Editor builds and simulates scripts.
Runner validates, approves, and executes them.
```

The [public editor](https://editor.baudbound.app/) is a browser-based Next.js application. The unified `baudbound` Rust application provides a Tauri desktop UI, CLI, background trigger service, package security, durable SQLite state, and native Windows and Linux execution.

Complete user, operator, deployment, and contributor documentation lives at [wiki.baudbound.app](https://wiki.baudbound.app/). Repository Markdown under `docs/wiki` is its source of truth.

## Repository

```text
apps/
  editor/                 Visual workflow editor
  baudbound/              Unified runner CLI and Tauri desktop app
crates/
  baudbound-actions/      Shared and native action implementations
  baudbound-core/         Runner orchestration
  baudbound-runtime/      Graph execution and runtime data
  baudbound-script/       Package and language contracts
  baudbound-security/     Capabilities, risk, approvals, and policy
  baudbound-storage/      SQLite durable state
  baudbound-triggers/     Background trigger services
schemas/                  JSON Schema package contracts
deploy/                   Container and service templates
docs/wiki/                Canonical public documentation
tools/                    Development, release, and wiki tooling
```

## Development

Requirements include Rust 1.95 or newer, Node.js 24, pnpm, and the platform dependencies required by Tauri 2.

Use the interactive development helper:

```powershell
./tools/development.ps1
```

Common verification commands:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
pnpm --dir apps/editor verify:release
pnpm --dir apps/baudbound/ui build
pnpm --dir tools/wiki-publisher test
pnpm --dir tools/wiki-publisher validate
```

Read the [developer documentation](https://wiki.baudbound.app/developers) before changing package contracts, node compatibility, native actions, security behavior, or release infrastructure.

## Documentation Policy

Detailed documentation belongs in `docs/wiki`. The only standalone Markdown outside that tree is this repository entry point and `docs/runner-release.md`, which is an internal release runbook. The publisher reconciles page content and the static navigation declared in `docs/wiki/navigation.json`.

## License

BaudBound is licensed under the MIT License.
