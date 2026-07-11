---
title: Contributing and Development Setup
description: Prepare the toolchain, run BaudBound locally, follow contribution standards, and execute quality gates.
tags: [developers, setup, contributing, testing]
---
# Contributing and Development Setup

## Requirements

Install Git, Rust 1.95 or newer, Node.js 24, pnpm, and platform dependencies required by Tauri 2. Windows development requires WebView2 and Microsoft C++ build tools. Linux development requires the appropriate WebKitGTK and system libraries.

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

## Contribution standards

Start from current `master` and keep changes scoped to one coherent behavior. Read the owning modules and tests before choosing an abstraction. Preserve unrelated changes in a dirty worktree.

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
