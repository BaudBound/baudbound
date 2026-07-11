---
title: Core Concepts
description: The projects, packages, runners, triggers, variables, capabilities, and approvals used by BaudBound.
tags: [getting-started, concepts]
---
# Core Concepts

**Project** is the editable graph and project settings held by the editor.

**Node** is one trigger, control-flow operation, or action. Edges define execution order and branches.

**Trigger** starts a run. Manual triggers start on demand; listeners such as schedules, webhooks, serial input, hotkeys, and file watchers require the runner service.

**Run** is one execution of one trigger. It has an ID, timestamps, result, logs, and variable snapshots.

**Variable** is named runtime data referenced with `{{variable_name}}`. Node outputs and derived metadata are read-only; variable-operation nodes create or update mutable values.

**Secret** is a named sensitive value declared by a package but supplied by the runner operator. Secret values are never exported from the editor.

**Package** is the `.bbs` ZIP container exported by the editor. It carries the executable program, manifest, editor metadata, assets, and integrity hashes.

**Target runtime** describes the required operating environment, such as Windows Desktop or Linux Headless. It is checked by both editor and runner.

**Capability** describes access requested by a package, such as filesystem writes, process execution, network access, desktop input, or serial I/O.

**Approval** records that an operator accepted a specific package revision and its requested capabilities. Package changes invalidate that decision.

**Runner service** is the long-lived process that loads enabled scripts and listens for background triggers. Desktop UI control and headless service operation use the same runner core.
