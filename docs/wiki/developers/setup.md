---
title: Development Setup
description: Prepare Rust, Node.js, pnpm, editor, runner, and desktop UI development environments.
tags: [developers, setup]
---
# Development Setup

Install Git, Rust 1.95 or newer, Node.js 24, pnpm, and platform build dependencies required by Tauri 2. Windows development requires WebView2 and Microsoft C++ build tools. Linux development requires the WebKitGTK and system libraries documented for the selected Tauri release.

Install editor dependencies:

```text
pnpm --dir apps/editor install
```

Install desktop UI dependencies:

```text
pnpm --dir apps/baudbound/ui install
```

The interactive PowerShell helper launches common applications and checks:

```powershell
./tools/development.ps1
```

Use `pnpm --dir apps/editor dev` for the editor and `cargo run -p baudbound -- COMMAND` for runner CLI work. The development helper launches the complete desktop development stack. Set `BAUDBOUND_HOME` to a disposable directory during manual runner testing so development data does not mix with a normal installation.

The editor uses Next.js 16. When changing framework behavior, consult the versioned documentation shipped under its installed `node_modules/next/dist/docs` and address deprecation warnings rather than relying on older examples.
