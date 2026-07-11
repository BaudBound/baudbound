---
title: Getting Started
description: Create, verify, export, install, and run a BaudBound automation.
tags: [getting-started]
---
# Getting Started

## 1. Build a workflow

Open the [public editor](https://editor.baudbound.app/). Add a trigger, connect actions or control-flow nodes, and configure each node in the inspector. A workflow begins at a trigger and follows directed edges through the graph.

Set the project name, author, version requirements, and target runtime before export. Target runtimes determine which nodes the editor permits and which runner can execute the package.

## 2. Verify and simulate

Open the Simulation tab and activate a trigger. The editor verifies the graph before execution. Fix blocking graph, configuration, variable, target-runtime, and secret-declaration errors. Simulation runs locally in the editor and substitutes simulated platform behavior where native access is unavailable.

Read [Editor simulation](../editor/simulation.md) before testing workflows with secrets or machine-specific actions.

## 3. Export the package

Export the project as a `.bbs` package. The package contains the manifest, executable graph, editor metadata, asset files, and integrity metadata. Keep the package intact; editing an installed package invalidates its hash and approval state.

## 4. Install the runner

Follow [Runner installation](../runner/installation.md). On a desktop, launching `baudbound` without a command opens the UI. On a headless system, use the CLI.

## 5. Import and approve

```powershell
baudbound script import C:\path\to\automation.bbs
baudbound script inspect automation
baudbound script approve automation
```

Approval applies to the exact imported package and its requested capabilities. Updating package content requires a new review.

## 6. Run or serve

Run a manual trigger:

```powershell
baudbound script run automation
```

Keep listener-based triggers active:

```powershell
baudbound serve
```

Continue with the [runner quick start](../runner/quick-start.md), [script lifecycle](../runner/scripts-approvals.md), and [background service guide](../runner/background-service.md).
