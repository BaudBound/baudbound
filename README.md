<div align="center">
  <img src="https://raw.githubusercontent.com/BaudBound/.github/master/assets/logo.svg" alt="BaudBound" width="320" />
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
src/                     CLI, Tauri host, and desktop integration
ui/                      Desktop interface
scripts/                 Release and package verification scripts
contracts/               Pinned BaudBound contracts submodule
crates/                  Runtime, storage, security, actions, and triggers
```

The `contracts/` submodule points to one reviewed commit from
[BaudBound/contracts](https://github.com/BaudBound/contracts). Initialize it
when cloning so builds use the exact contracts selected by this repository.

## Development

Requirements include Rust 1.95 or newer, Node.js 24, pnpm, and the platform
dependencies required by Tauri 2.

Clone with submodules, or initialize them in an existing clone:

```text
git submodule update --init --recursive
```

Clone [BaudBound/tooling](https://github.com/BaudBound/tooling) beside this repository to use the interactive development helper:

```powershell
cd ../tooling
./development.ps1 -Action Runner
```

The same tooling repository contains guarded runner release operations and tasks that coordinate multiple BaudBound repositories.

Run the main quality gates from the repository root:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
pnpm --dir ui typecheck
pnpm --dir ui test
pnpm --dir ui build
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
