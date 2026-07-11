---
title: Getting Started
description: Create, review, and run your first BaudBound automation.
tags:
  - getting-started
---
# Getting Started

A BaudBound automation has two parts:

1. The editor creates a signed-format `.bbs` package containing the graph and declared permissions.
2. The runner validates, installs, approves, and executes that package on the target machine.

## Basic workflow

1. Open the [BaudBound editor](https://editor.baudbound.app/).
2. Build a graph beginning with a trigger such as Manual Trigger.
3. Verify the graph and export the package for the correct target runtime.
4. [Install the runner](../runner/installation.md).
5. [Import and run the package](../runner/quick-start.md).

Packages exported for a desktop runtime cannot run under a headless target. The editor and runner both enforce target compatibility before execution.
