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
