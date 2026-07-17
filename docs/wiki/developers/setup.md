---
title: Contributing and Development Setup
description: Prepare the toolchain, run BaudBound locally, follow contribution standards, and execute quality gates.
tags: [developers, setup, contributing, testing]
---
# Contributing and Development Setup

## Requirements

Install Git, Rust 1.95 or newer, Node.js 24, pnpm, and platform dependencies required by Tauri 2. Windows development requires WebView2 and Microsoft C++ build tools. Linux development requires the appropriate WebKitGTK and system libraries.

Use pnpm 11.10.0 to match CI. The Cargo workspace pins Rust 1.95 as its minimum supported toolchain.

## Platform prerequisites {.tabset}

### Windows

Install Git, Rust through `rustup` with the stable MSVC toolchain, Visual Studio Build Tools with **Desktop development with C++** and a current Windows SDK, Node.js 24, pnpm 11.10.0, and Microsoft Edge WebView2 Runtime.

```powershell
git --version
rustc --version
cargo --version
node --version
pnpm --version
```

### Linux

Install Git, Rust 1.95, Node.js 24, pnpm 11.10.0, a C/C++ toolchain, pkg-config, WebKitGTK 4.1 development headers, AppIndicator, ALSA, udev, SVG, xdo, and patchelf packages. Package names differ by distribution. The authoritative Ubuntu package list used by CI is in `.github/workflows/runner-ci.yml`.

```bash
git --version
rustc --version
node --version
pnpm --version
pkg-config --modversion webkit2gtk-4.1
```

Linux native desktop tests require a graphical session. Headless CI can compile the desktop application but cannot prove every input, tray, notification, or window interaction.

## Clone and install

```text
git clone https://github.com/NATroutter/BaudBound.git
cd BaudBound
pnpm --dir apps/editor install --frozen-lockfile
pnpm --dir apps/baudbound/ui install --frozen-lockfile
pnpm --dir tools/wiki-publisher install --frozen-lockfile
```

Install JavaScript dependencies:

```text
pnpm --dir apps/editor install
pnpm --dir apps/baudbound/ui install
```

Use the interactive helper to launch the editor, desktop application, runner service, tests, schemas, or builds:

```powershell
./tools/development.ps1
```

For direct work, use `pnpm --dir apps/editor dev` or `cargo run -p baudbound -- COMMAND`. Set `BAUDBOUND_HOME` to a disposable directory during runner development.

The helper menu offers **Desktop**, **Desktop UI**, **Editor**, **Service**, **Status**, **Install**, **Checks**, **Tests**, **Editor E2E**, **Schemas**, and **Build**. Direct invocation is useful in automation:

```powershell
./tools/development.ps1 -Action Checks
./tools/development.ps1 -Action Tests
```

## Use disposable runner state {.tabset}

### PowerShell

```powershell
$env:BAUDBOUND_HOME = Join-Path $env:TEMP "baudbound-development"
cargo run -p baudbound -- status
```

### POSIX shell

```bash
export BAUDBOUND_HOME="$(mktemp -d)"
cargo run -p baudbound -- status
```

The first runner command creates a default `config.toml` and SQLite database in that directory. Remove the disposable directory only after all related runner and desktop processes have stopped.

## Development loops

| Work | Start command | Main fast checks |
| --- | --- | --- |
| Editor | `pnpm --dir apps/editor dev` | lint, typecheck, focused tests, schemas check |
| Desktop UI only | `pnpm --dir apps/baudbound/ui dev` | typecheck and Vitest. Tauri APIs need the desktop shell |
| Tauri desktop | Choose **Desktop** in the helper | UI tests plus focused Rust tests |
| Headless service | `cargo run -p baudbound -- serve` | crate and command integration tests |
| Schemas | `pnpm --dir apps/editor schemas:generate` | inspect diff, then `schemas:check` |
| Wiki | `pnpm --dir tools/wiki-publisher validate` | publisher tests and rendered dry run |

## Contribution standards

Start from current `master` and keep changes scoped to one coherent behavior. Read the owning modules and tests before choosing an abstraction. Preserve unrelated changes in a dirty worktree.

By intentionally submitting a contribution, you agree to license it under the license that applies to the part of the repository you changed. Code contributions use the PolyForm Noncommercial License 1.0.0. Documentation and non-code creative contributions use CC BY-NC-SA 4.0. You must have the right to submit the work and must identify third-party material and its license. See [Licensing and Attribution](../licensing.md) for the complete scope.

Do not add placeholders, ignored configuration, shell-based native-action shortcuts, duplicated sources of truth, or platform support without an implementation. Treat package parsing, secrets, approvals, filesystem changes, network listeners, and process control as security-sensitive.

Update public documentation whenever user behavior, operations, contracts, configuration, or contribution workflow changes.

## Quality gates

Run Rust formatting, lint, and tests:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

Run the editor gate:

```text
pnpm --dir apps/editor verify:release
```

Run desktop UI checks:

```text
pnpm --dir apps/baudbound/ui typecheck
pnpm --dir apps/baudbound/ui test
pnpm --dir apps/baudbound/ui build
```

Validate schemas and wiki content:

```text
pnpm --dir apps/editor schemas:check
pnpm --dir tools/wiki-publisher test
pnpm --dir tools/wiki-publisher validate
```

Tests should cover success, rejection, persistence, restart, concurrency, and platform boundaries. Native actions need platform-specific coverage.

The editor uses Next.js 16. For framework changes, consult the versioned documentation shipped in its installed `node_modules/next/dist/docs` and address deprecation warnings.

## Prepare a pull request

1. Keep the change focused and preserve unrelated worktree changes.
2. Add tests at the owning layer and at any changed cross-language boundary.
3. Update schemas and public wiki pages in the same change when behavior changed.
4. Run the gates listed by [Testing and CI](testing-ci.md).
5. Review `git diff --check` and the complete diff before committing.
6. Describe behavior, risk, platform coverage, and tests in the pull request.

Do not commit generated build directories, private signing keys, runner state, private `.bbs` packages, or local plan files.
