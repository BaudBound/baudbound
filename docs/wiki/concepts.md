---
title: Concepts and Glossary
description: Definitions for BaudBound projects, packages, scripts, runs, security, and services.
tags: [concepts, glossary]
---
# Concepts and Glossary

Use this page when a BaudBound term is unfamiliar or when two similar terms seem interchangeable. The distinctions matter because editing, validation, approval, and execution happen at different stages.

## Workflow lifecycle

| Term | Meaning |
| --- | --- |
| **Project** | The editable workflow in the visual editor, including its graph, project settings, variables, secret declarations, comments, and assets. |
| **Graph** | The complete set of executable nodes and the edges connecting them. |
| **Node** | One trigger, action, or control-flow operation in a graph. Each executable node has a stable ID used by edges, output references, logs, and runner reports. |
| **Edge** | A directed connection from one node output to another node input. Edges determine execution order and branch routing. |
| **Trigger** | A node that begins a run. A trigger can be manual or can wait for an event such as a schedule, request, file change, hotkey, process, startup, or serial input. |
| **Action** | A node that performs work, such as logging, transforming data, calling a URL, managing a file, starting a process, or interacting with supported hardware or desktop APIs. |
| **Control flow** | A node that chooses or repeats execution paths, such as If/Else, Switch, Loop, While, or For Each. |
| **Run** | One execution of a script, beginning at a trigger and ending in success, failure, cancellation, or another recorded terminal state. A run has its own ID, logs, node results, and variable snapshots. |
| **Simulation** | An editor-side test run using supplied trigger data and simulated or controlled side effects. Simulation does not grant runner access or replace production validation. |

## Packages and installed scripts

| Term | Meaning |
| --- | --- |
| **Package** | A portable `.bbs` archive exported by the editor. It contains the manifest, executable program, editor data, assets, schemas, and integrity metadata. |
| **Manifest** | Package metadata such as script identity, display name, version requirements, target runtime, permissions, capabilities, and secrets. |
| **Script identity** | The stable package ID used to recognize updates to the same installed script. It is different from the display name and filename. |
| **Installed script** | A package revision imported into one runner's storage. The runner tracks whether it is enabled, approved, healthy, and eligible for trigger registration. |
| **Package hash** | A digest of the package contents used to detect changes and bind approval to one exact revision. |
| **Asset** | A file embedded in a package for a node to use at runtime, such as an audio file. Assets are distinct from arbitrary files already present on the runner machine. |

### Project, package, and installed script

These are three stages of the same automation:

1. Edit the **project** in the editor.
2. Export an immutable **package** from that project.
3. Import the package as an **installed script** on a runner.

Changing a project does not change an already exported package. Exporting a new package does not update an installed script until that package is imported with the update workflow.

The package filename and display name are convenient labels. The script identity determines whether the runner treats a package as a revision of an existing script.

## Data

| Term | Meaning |
| --- | --- |
| **Variable** | A named runtime value referenced with `{{name}}`. Variables can come from project declarations, trigger payloads, node outputs, loops, manifests, or runner-provided system data. |
| **Runtime variable** | A writable value that exists only during one run. |
| **Persistent variable** | A writable value stored by the runner between runs of the script. |
| **Global variable** | A runner-level value made available according to runner policy rather than stored inside one package. |
| **Secret declaration** | A name and type included in a package to state that a value is required. It does not contain the production secret value. |
| **Secret value** | Sensitive data configured on the runner and resolved only during execution. Editor simulation values are temporary and are not exported. |

A variable is ordinary workflow data and may appear in logs or package configuration. A secret is intentionally supplied outside the package. Do not use a normal variable when disclosure would be harmful.

See [Variables and Data](editor/variables.md) and [Secrets](runner/secrets.md) for syntax and lifecycle details.

## Compatibility and security

| Term | Meaning |
| --- | --- |
| **Target runtime** | The intended operating-system and session family, such as Windows desktop or Linux headless. It controls which nodes may be added and which runner can execute the package. |
| **Permission** | A human-readable description of sensitive behavior requested by a node or package. Permissions carry a risk level. |
| **Capability** | A machine-checkable identifier for a category of runtime access, such as file, process, network, serial, or desktop interaction. |
| **Risk** | A review classification such as low, medium, or high that helps an operator focus on behavior with greater impact. Risk is not proof that a package is safe or unsafe. |
| **Approval** | The runner operator's acceptance of one exact package hash and its declared access. Approval does not automatically transfer to an updated revision. |
| **Policy** | Runner-side restrictions that can deny behavior even when the package declares it and the revision is approved. |

### Verification, simulation, validation, and approval

| Stage | Where it happens | What it answers |
| --- | --- | --- |
| **Verification** | Editor | Is the graph internally consistent, configured, and compatible enough to simulate or export? |
| **Simulation** | Editor | Does the workflow follow the expected paths with test input and controlled side effects? |
| **Validation** | Runner | Is the imported package structurally valid, intact, supported, and consistent with runner contracts? |
| **Approval** | Runner operator | Do I accept this exact revision and its requested behavior on this machine? |

Passing an earlier stage does not bypass a later one.

### Enabled and approved are different

An **approved** script has operator approval for its current package hash. An **enabled** script may register long-running triggers when the service loads it. A script generally needs to be valid, approved, and enabled before listener-based triggers become active.

Disabling a script is useful when you want to keep it installed but stop unattended trigger registration. Revoking approval records that the current revision is no longer trusted.

## Runner operation

| Term | Meaning |
| --- | --- |
| **Runner** | The native BaudBound application that validates packages, stores installed scripts and state, enforces approval and policy, and executes supported actions. |
| **Runner home** | The directory containing runner-managed state, installed packages, the SQLite database, logs, and related local data. It can be overridden with `BAUDBOUND_HOME`. |
| **Configuration** | The runner's `config.toml`, containing listener, target-runtime, serial-device, and related machine-specific settings. |
| **Listener** | A long-running component that waits for external input and creates trigger events, such as a webhook socket, file watcher, schedule timer, or serial reader. |
| **Service** | A long-running `baudbound serve` process that owns listeners and dispatches their events. |
| **Desktop background runner** | A service process started and supervised by the BaudBound desktop application for the signed-in desktop user. |
| **Operating-system service** | A headless `baudbound serve` process supervised by systemd, OpenRC, runit, or another service manager configured by the operator. |

The desktop background runner and an operating-system service solve the same long-running need in different environments. Do not run both against the same runner home, listener ports, and serial devices.

## Network terms

| Term | Meaning |
| --- | --- |
| **Bind address** | The local network interface on which a listener accepts connections. `127.0.0.1` is reachable only from the same machine; `0.0.0.0` exposes all IPv4 interfaces. |
| **Port** | The numeric TCP endpoint used by a webhook or WebSocket listener. Only one process can normally bind the same address and port combination. |
| **Webhook hook name** | The node setting used to create a route such as `/events/deploy`. It is not the listener address or port. |
| **Reverse proxy** | A separate server that accepts client traffic and forwards selected requests to the runner, commonly adding TLS, authentication, and request controls. |

For example, a Webhook node with hook name `deploy` may receive requests at `http://127.0.0.1:43891/events/deploy`. The `deploy` segment comes from the node; `127.0.0.1:43891` comes from runner configuration.

Read [Background Service and Triggers](runner/service-triggers.md) before enabling listeners and [Security Model](security/index.md) before exposing them beyond the local machine.
