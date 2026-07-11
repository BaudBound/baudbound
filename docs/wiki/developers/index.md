---
title: Developer Documentation
description: Architecture and contribution guidance for BaudBound maintainers and contributors.
tags: [developers, contributing]
---
# Developer Documentation

BaudBound is a mixed TypeScript and Rust workspace. The editor owns visual authoring and node-definition contracts. The Rust crates own package trust, durable state, orchestration, execution, triggers, security, and native operations. The Tauri application combines runner CLI and desktop UI in one binary.

Production quality requires contract-first changes, focused modules, deterministic validation, native platform implementations, and tests proportional to behavior and risk. Unsupported platform behavior must be declared in node definitions and rejected by editor and runner rather than approximated with shell scripts.

Start with [Architecture](architecture.md), [Development setup](setup.md), [Testing](testing.md), and [Contributing](contributing.md). Package-language changes require [contract work](package-contract.md). Documentation changes follow [Wiki documentation](wiki-documentation.md). Releases follow [Release engineering](releases.md).
