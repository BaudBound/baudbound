---
title: Visual Editor
description: Build, validate, simulate, and export BaudBound automation projects.
tags: [editor]
---
# Visual Editor

The editor is available at [editor.baudbound.app](https://editor.baudbound.app/). It runs in the browser and stores active project state locally until you export or import a project package.

## Main areas

- The palette contains available trigger, control-flow, and action nodes.
- The canvas supports connecting, moving, selecting, copying, duplicating, and deleting nodes, plus non-executable comment nodes.
- The inspector edits the selected node and project-level settings.
- The Variables tab lists configured values, node outputs, secrets, built-in values, and derived metadata.
- The Simulation tab verifies and runs selected triggers without exporting.
- The output console reports verification, simulation, and package operations.

## Recommended workflow

1. Choose the [target runtime](target-runtimes.md).
2. Add one or more triggers and connect the graph.
3. Configure node inputs using literals, [variables](variables.md), or declared secrets.
4. Verify and [simulate](simulation.md) every branch.
5. Review project metadata and package assets.
6. Export the `.bbs` package and install it in a runner.

See the [node reference](node-reference.md), [workspace guide](workspace.md), and [package export guide](packages-assets.md).
