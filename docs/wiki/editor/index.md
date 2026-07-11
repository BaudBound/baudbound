---
title: Visual Editor
description: Build, document, validate, and export BaudBound automation projects.
tags: [editor]
---
# Visual Editor

The editor is available at [editor.baudbound.app](https://editor.baudbound.app/). It runs in the browser and keeps the active project locally until you import or export a package.

## Workspace

The palette contains trigger, control-flow, and action nodes. Click or drag a palette entry onto the canvas, then drag from an output handle to a compatible input handle. Branching nodes expose named outputs for their possible paths.

Selecting a node opens its configuration in the inspector. Node IDs remain stable and identify outputs, validation messages, and run logs. A custom name changes the displayed header without changing the underlying ID.

The Variables tab lists configured values, node outputs, secrets, built-in values, and derived metadata. The Simulation tab verifies and runs selected triggers. The output console reports verification, simulation, and package operations.

## Selection and editing

Use pointer selection for one node. Hold the platform selection modifier while dragging on empty canvas to select multiple nodes. Copy, paste, duplicate, and delete operate on the current selection and are also available from node context menus.

Project settings choose one visual edge style for the graph: straight, step, smooth step, or Bezier. Edge appearance does not change execution.

## Comments

Comment nodes document the graph without being executed. They support the same selection, movement, copy, paste, duplication, and deletion controls as executable nodes. Edit their text, color, size, and font size, and drag them from the top bar.

## Project settings

Set the project name, author, description, website or repository when applicable, minimum runner version, and [target runtime](target-runtimes.md). Use a stable project identity so runner updates can match new exports to the installed script.

## Graph rules

Triggers start execution and do not accept incoming execution edges. Other executable nodes must be reachable from a trigger. Connect the required named outputs from branching and loop nodes. Loop bodies do not need an explicit edge back to the loop node.

Verification rejects malformed branches, invalid configuration, unresolved variables, incompatible targets, and invalid package metadata.

## Packages and assets

The editor imports and exports `.bbs` packages. Before export, resolve verification errors and review requested capabilities.

Files used by a workflow can be embedded as package assets so the runner does not depend on the original editor machine's path. Never place credentials or secret values in assets.

Canvas positions, comments, and visual settings are editing metadata and do not execute. The package also contains the executable graph, manifest, declarations, assets, and integrity information.

Export creates integrity metadata that the runner checks during import. Editing or repacking files inside `.bbs` invalidates verification; make changes in the editor and export a new package instead.

Use the [node reference](node-reference.md), [variables guide](variables.md), and [simulation guide](simulation.md) while building workflows.
