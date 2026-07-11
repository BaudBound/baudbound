---
title: Developer Overview
description: BaudBound architecture, ownership boundaries, state, communication, and trust model.
tags: [developers, architecture]
---
# Developer Overview

BaudBound is a TypeScript and Rust workspace. Changes should preserve explicit ownership boundaries, deterministic validation, native platform implementations, and tests proportional to behavior and risk.

## Applications

`apps/editor` is the Next.js visual editor. `apps/baudbound` is the unified runner CLI and Tauri desktop application; its React UI lives under `apps/baudbound/ui`.

## Rust crates

`baudbound-script` owns package and language models plus archive validation. `baudbound-security` derives permissions, capabilities, and policy decisions. `baudbound-storage` owns SQLite schema and durable repositories. `baudbound-runtime` executes validated graphs. `baudbound-actions` implements actions. `baudbound-triggers` implements listener services. `baudbound-core` coordinates these domains.

Crate roots expose public APIs and delegate implementation into domain folders. Split unrelated responsibilities before files become difficult to review, but avoid abstractions that obscure ownership.

## State and communication

SQLite is durable runner state and coordinates independent processes. Tauri commands bridge the desktop UI to Rust. TOML is operator configuration, and `.bbs` is the portable script artifact. JSON is used for cross-language package documents and explicit machine-readable output, not as a parallel mutable runner database.

## Trust boundary

The editor produces package declarations. The runner independently verifies package integrity, compatibility, requested access, approval, policy, secrets, and node configuration before native side effects. UI visibility never substitutes for backend authorization.

Unsupported platform behavior must be declared and rejected by both editor and runner rather than approximated with shell scripts.
