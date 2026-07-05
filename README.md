<div align="center">
  <img src="assets/logo.svg" alt="BaudBound" width="320"/>
  <br/><br/>

  <p><strong>Local-first visual automation scripting.</strong></p>
  <p>Build scripts in the web editor, export portable <code>.bbs</code> packages, and run them locally with the BaudBound runner.</p>

  ![Editor](https://img.shields.io/badge/Editor-Next.js-111827?style=for-the-badge&logo=nextdotjs&logoColor=white)
  ![Runner](https://img.shields.io/badge/Runner-Rust-b7410e?style=for-the-badge&logo=rust&logoColor=white)
  ![Package](https://img.shields.io/badge/Package-.bbs-e62d3e?style=for-the-badge)
  ![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey?style=for-the-badge)
  ![License](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)
</div>

---

## What Is BaudBound?

BaudBound is a visual scripting automation platform for building local automation scripts without turning the browser into a trusted runtime.

The project is split into two parts:

- **BaudBound Editor**: a hosted or self-hosted web app for visually creating, validating, simulating, and exporting `.bbs` script packages.
- **BaudBound Runner**: a local runtime that imports `.bbs` packages, validates them again, asks for required permissions, and executes scripts on the user's machine.

The core rule is simple:

```text
Editor builds scripts.
Runner owns trusted execution.
```

## V2 Direction

This repository is the V2 architecture. It replaces the older V1 serial-event Java application with a package-based editor and runner model.

V1 focused on mapping serial input directly to actions. V2 keeps serial automation as one use case, but expands BaudBound into a general visual automation system with:

- node-based script graphs
- reusable `.bbs` package format
- local-first runner execution
- explicit permissions, capabilities, and risk analysis
- browser-only simulation for previewing graph flow
- asset packaging for script resources such as audio files

## Current Status

The editor is the most complete part of V2. The runner folders and crates define the intended Rust runtime structure, but trusted execution belongs in the runner and must continue to validate every imported package.

Do not treat exported editor packages as inherently trusted. The runner must enforce the schema, permissions, capabilities, filesystem safety, and platform rules.

## BaudBound Editor

The editor is a Next.js application in `apps/editor`.

It currently supports:

- React Flow based visual node editor
- project settings and manifest metadata
- `.bbs` import and export
- package assets managed fully client-side
- package verification before export
- browser-only simulator with runtime variables
- node runtime output references using `{{node-id.field}}`
- built-in read-only variables such as manifest and system values
- Docker image and Compose deployment
- browser E2E and package contract tests

### Run Locally

```bash
cd apps/editor
pnpm install
pnpm dev
```

The editor is browser-local and does not require a backend service.

### Verify The Editor

```bash
cd apps/editor
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm e2e
```

For the full release gate:

```bash
pnpm verify:release
```

### Docker

Build and run the editor image:

```bash
docker build -f apps/editor/Dockerfile apps/editor -t baudbound-editor
docker run --rm -p 3000:3000 -e NEXT_PUBLIC_EDITOR_URL=http://localhost:3000 baudbound-editor
```

Or use the Compose template:

```bash
docker compose -f apps/editor/compose.yaml up -d
```

The GitHub Actions workflow in `.github/workflows/editor-docker.yml` builds the editor container and publishes it to GitHub Container Registry for non-PR builds.

## Script Packages

`.bbs` means **BaudBound Script**.

A `.bbs` package is a zip-based script package that contains the script manifest, graph program, permissions, capabilities, editor metadata, and optional assets.

The schemas live in `schemas/`:

- `manifest.schema.json`
- `program.schema.json`
- `permissions.schema.json`
- `capabilities.schema.json`
- `editor.schema.json`

The editor validates packages for authoring quality and export safety. The runner must validate packages again before import and execution.

## Repository Layout

```text
apps/
  editor/           Web editor for building and exporting .bbs packages
  runner-cli/       Planned command-line runner interface
  runner-desktop/   Planned desktop runner shell and tray app
crates/
  baudbound-core/
  baudbound-runtime/
  baudbound-script/
  baudbound-security/
  baudbound-actions/
  baudbound-triggers/
  baudbound-storage/
schemas/            JSON schemas for package contracts
assets/             Project logos and shared visual assets
```

## Security Model

BaudBound is designed around explicit trust boundaries:

- The editor may help users build and simulate scripts, but it is not a trusted executor.
- The runner owns local execution and must reject invalid or unsafe packages.
- Script assets are packaged locally and should be validated by content, not just extension.
- Permissions and capabilities are part of the package contract and must be surfaced before execution.
- User-facing automation should be understandable, auditable, and reversible where possible.

---

<div align="center">
  <img src="assets/logo-notext.svg" alt="BaudBound" width="64"/>
</div>
