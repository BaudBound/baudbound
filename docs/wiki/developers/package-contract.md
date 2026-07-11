---
title: Changing the Package Contract
description: Safely add nodes or evolve schemas across the editor and Rust runner.
tags: [developers, packages, schemas]
---
# Changing the Package Contract

Package compatibility is cross-language and must be changed deliberately. Prefer additive changes with explicit defaults. Breaking changes require a format or language version decision and migration strategy before implementation.

For a node change:

1. Update the editor node definition, fields, outputs, capabilities, risk, and target runtimes.
2. Regenerate and review its JSON Schema.
3. Update Rust models or semantic validation where shape or invariants changed.
4. Update security capability derivation.
5. Implement runtime dispatch and the native action or trigger.
6. Add editor contract, schema, Rust unit, integration, and platform tests.
7. Update the public node and behavior documentation.

Never make the runner fetch schemas from the public server during execution. Committed and compiled contracts keep validation deterministic and available offline.
