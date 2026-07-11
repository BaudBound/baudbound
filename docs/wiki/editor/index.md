---
title: Visual Editor
description: Build and export BaudBound automation graphs in the browser.
tags:
  - editor
---
# Visual Editor

The BaudBound editor is available at [editor.baudbound.app](https://editor.baudbound.app/).

The editor is responsible for:

- creating trigger, control-flow, variable, and action nodes;
- validating graph structure and node configuration;
- calculating required permissions and capabilities;
- checking target-runtime compatibility;
- simulating workflows; and
- exporting `.bbs` packages for the runner.

Simulation helps test graph behavior, but it does not replace runner validation. The runner independently recalculates security declarations and rejects incompatible or tampered packages.
