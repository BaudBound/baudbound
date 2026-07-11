---
title: Architecture
description: BaudBound repository layout, ownership boundaries, and process communication.
tags: [developers, architecture]
---
# Architecture

## Applications

`apps/editor` is the Next.js visual editor. `apps/baudbound` is the unified Rust CLI and Tauri desktop application. Its React UI lives under `apps/baudbound/ui` and is compiled into production bundles by Tauri.

## Rust crates

`baudbound-script` owns package and language models plus archive validation. `baudbound-security` derives permissions, capabilities, and policy decisions. `baudbound-storage` owns SQLite schema and durable repositories. `baudbound-runtime` executes validated graphs. `baudbound-actions` implements shared and native action behavior. `baudbound-triggers` implements listener services. `baudbound-core` coordinates package, storage, security, runtime, secrets, serial devices, and sub-scripts.

Crate roots expose public APIs and delegate implementation into domain folders. Large unrelated modules should be split by responsibility, while avoiding abstractions that obscure ownership.

## State and communication

SQLite is durable runner state and the coordination point between independent processes. Tauri commands are the desktop React-to-Rust bridge. Desktop background control uses in-process or local IPC owned by the app. TOML is operator configuration; `.bbs` is the portable script artifact.

JSON remains appropriate inside the cross-language package contract and for explicit machine-readable CLI responses. It is not used as a parallel mutable runner database.

## Trust boundary

The editor produces declarations. The runner verifies them independently before native execution. UI visibility never substitutes for backend authorization.
