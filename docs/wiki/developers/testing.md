---
title: Testing and Quality Gates
description: Run editor, Rust, desktop UI, schema, wiki, and release verification checks.
tags: [developers, testing, ci]
---
# Testing and Quality Gates

Run Rust formatting, lint, and tests:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

Run editor release verification:

```text
pnpm --dir apps/editor verify:release
```

Run desktop UI checks:

```text
pnpm --dir apps/baudbound/ui typecheck
pnpm --dir apps/baudbound/ui test
pnpm --dir apps/baudbound/ui build
```

Validate schemas and wiki source:

```text
pnpm --dir apps/editor schemas:check
pnpm --dir tools/wiki-publisher test
pnpm --dir tools/wiki-publisher validate
```

Runner CI executes Windows and Linux matrices plus editor contract checks. Tests should cover success, rejection, persistence, restart, concurrency, and platform boundaries. Native actions need platform-specific tests; code that cannot be exercised on a supported platform must not be advertised as supported.
