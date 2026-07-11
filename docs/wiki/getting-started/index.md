---
title: Getting Started
description: Create, verify, export, install, and run a BaudBound automation.
tags: [getting-started]
---
# Getting Started

## Before you begin

You need a current web browser, access to the [BaudBound editor](https://editor.baudbound.app/), and a supported Windows or Linux machine on which to install the runner. No account is required to build a project in the public editor.

This guide uses `automation` as an example installed script name. Replace it with the name printed by the import command or shown in the desktop Scripts view.

## 1. Build a workflow

Open the [public editor](https://editor.baudbound.app/). Add a trigger, connect actions or control-flow nodes, and configure each node in the inspector. A workflow begins at a trigger and follows directed edges through the graph.

Set the project name, author, version requirements, and target runtime before export. Target runtimes determine which nodes the editor permits and which runner can execute the package.

## 2. Verify and simulate

Open the Simulation tab and activate a trigger. The editor verifies the graph before execution. Fix blocking graph, configuration, variable, target-runtime, and secret-declaration errors. Simulation runs locally in the editor and substitutes simulated platform behavior where native access is unavailable.

Read [Editor simulation](../editor/simulation.md) before testing workflows with secrets or machine-specific actions.

## 3. Export the package

Export the project as a `.bbs` package. The package contains the manifest, executable graph, editor metadata, asset files, and integrity metadata. Keep the package intact; editing an installed package invalidates its hash and approval state.

## 4. Install the runner

Follow [Runner installation](../runner/installation.md), then open the desktop application. The first launch creates the configuration and runner storage automatically.

## 5. Import and approve

The simplest desktop workflow is:

1. Open **Scripts**.
2. Choose **Import package** and select the exported `.bbs` file.
3. Open the imported script's approval review.
4. Read the target runtime, risk level, requested capabilities, nodes, and required secrets.
5. Choose **Approve** only when the requested access matches the workflow you intended to build.

The equivalent CLI workflow is shown below. Replace the example user and filename with the actual downloaded package path.

### CLI workflow {.tabset}

#### Windows

```powershell
baudbound script import "C:\Users\Alice\Downloads\automation.bbs"
baudbound script list
baudbound script inspect automation
baudbound script approve automation
```

#### Linux

```text
baudbound script import ~/Downloads/automation.bbs
baudbound script list
baudbound script inspect automation
baudbound script approve automation
```

### What approval means

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

`serve` stays in the foreground and continues running until it is stopped. It is needed only for schedules and other triggers that wait for an event. A desktop user can instead start the background runner from the **Service** view. A headless Linux user should follow [Linux Background Service](../self-hosting/linux-background-service.md).

Continue with [Script Management](../runner/script-management.md) and [Service and Triggers](../runner/service-triggers.md).

## Key terms

**Project** is the editable graph and project settings in the editor. **Nodes** are triggers, control-flow operations, or actions; edges define their execution order and branches.

**Trigger** starts a **run**, which is one execution with its own result, logs, and variable snapshots. Manual triggers start on demand. Schedules, webhooks, serial input, hotkeys, and file watchers require a running background service.

**Variables** are named runtime values referenced with `{{variable_name}}`. **Secrets** are sensitive values declared by a package but supplied by the runner operator; they are never exported from the editor.

A **package** is the exported `.bbs` file. Its **target runtime** describes the required operating system and whether desktop interaction is needed. **Capabilities** describe requested access such as filesystem writes, process execution, network access, desktop input, or serial I/O.

An **approval** accepts one exact package revision and its capabilities. Updating package content invalidates that approval.
