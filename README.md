# BaudBound

BaudBound is a local-first visual scripting automation platform.

The project is organized as a monorepo with a strict split between script authoring and script execution:

- `apps/editor`: web-based visual script builder.
- `apps/runner-cli`: command-line entry point for the runner.
- `apps/runner-desktop`: future desktop agent/tray UI.
- `crates`: Rust crates for package validation, runtime, storage, actions, triggers, and security.
- `schemas`: JSON schemas for `.bbs` package files.
- `examples`: sample `.bbs` packages and source fixtures.
- `docs`: shared project documentation.
- `plan`: product and architecture planning notes.

Core rule:

```text
Editor builds scripts.
Runner owns execution.
```

