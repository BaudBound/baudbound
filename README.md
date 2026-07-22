<div align="center">
  <img src="assets/logo.svg" alt="BaudBound" width="320" />
  <br /><br />
  <p><strong>Native execution for BaudBound visual automation scripts.</strong></p>

  ![Runner](https://img.shields.io/badge/Runner-Rust-b7410e?style=for-the-badge&logo=rust&logoColor=white)
  ![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux-lightgrey?style=for-the-badge)
  [![License](https://img.shields.io/badge/Code-PolyForm%20Noncommercial-blue?style=for-the-badge)](LICENSE.md)
</div>

## Overview

This repository contains the BaudBound runner. One Rust application provides
the native desktop interface, command line interface, background trigger
service, package validation, approvals, durable SQLite state, and supported
Windows and Linux actions.

Scripts are created with the [BaudBound editor](https://editor.baudbound.app/)
and exported as `.bbs` packages. The runner recalculates package permissions,
capabilities, and risk before approval and execution.

User and contributor documentation is available at
[wiki.baudbound.app](https://wiki.baudbound.app/). Its source is maintained in
the [documentation repository](https://github.com/BaudBound/documentation).

## Repository layout

```text
apps/baudbound/          CLI, Tauri host, desktop UI, and release scripts
crates/                  Runtime, storage, security, actions, and triggers
schemas/                 Pinned package contract snapshots consumed by Rust
tools/                   Runner development and release helpers
docs/runner-release.md   Maintainer release runbook
```

Shared public contracts are published from
[BaudBound/contracts](https://github.com/BaudBound/contracts). The snapshots in
this repository are pinned so builds remain reproducible and do not download
moving contracts from the network.

## Development

Requirements include Rust 1.95 or newer, Node.js 24, pnpm, and the platform
dependencies required by Tauri 2.

Use the interactive Windows development helper:

```powershell
./tools/development.ps1
```

Run the main quality gates from the repository root:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
pnpm --dir apps/baudbound/ui typecheck
pnpm --dir apps/baudbound/ui test
pnpm --dir apps/baudbound/ui build
```

## Related repositories

The visual editor is in [BaudBound/editor](https://github.com/BaudBound/editor).
Installation scripts are in [BaudBound/get](https://github.com/BaudBound/get).
The public website is in [BaudBound/website](https://github.com/BaudBound/website).

## License

BaudBound is source available and accepts community contributions.

Software is licensed under the [PolyForm Noncommercial License 1.0.0](LICENSE.md).
The BaudBound name and logos are reserved under the [brand notice](TRADEMARKS.md).
Third-party components remain subject to their own licenses.
