---
title: Editor Workspace
description: Navigate and edit graphs, nodes, edges, comments, and project settings in the visual editor.
tags: [editor, workspace]
---
# Editor Workspace

## Add and connect nodes

Click or drag a palette entry onto the canvas. Drag from an output handle to a compatible input handle to create an edge. Branching nodes expose named outputs; connect only the branches the workflow needs.

Selecting a node opens its configuration. Node IDs remain stable and identify node outputs, validation messages, and run logs. A custom name replaces the ID in the node header while the underlying ID remains unchanged.

## Selection and editing

Use normal pointer selection for individual nodes. Hold the platform selection modifier while dragging on empty canvas to select multiple nodes. Copy, paste, duplicate, and delete operate on the current selection. The node context menu exposes the same core operations.

## Comments

Comment nodes support the same selection, movement, copy, paste, duplication, and deletion controls as executable nodes. They are not executed by the runner. A comment supports editable text, color, size, and font size; drag it from its top bar.

## Edges

Project settings choose one edge style for the graph: straight, step, smooth step, or Bezier. The selection is editor metadata stored in `editor.json`; it does not alter runtime execution.

## Project settings

Project settings define package identity, author and links, runner version requirement, target runtime, edge presentation, and other editor behavior. Use a stable project name because the runner uses package identity and script aliases during updates.

## Graph rules

Triggers start execution and do not accept incoming execution edges. Actions and control-flow nodes must be reachable from a trigger to run. Loops use their body and completion outputs; the body no longer needs an explicit edge back to the loop node. Verification rejects malformed branches, invalid node configuration, unresolved variables, and unsupported target-runtime combinations.
