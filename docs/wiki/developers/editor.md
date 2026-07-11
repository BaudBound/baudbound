---
title: Editor and Package Development
description: Extend the editor, node registry, schemas, and cross-language package contract.
tags: [developers, editor]
---
# Editor and Package Development

Node definitions under `apps/editor/data/nodes/definitions` own display metadata, category, configuration fields, outputs, risk, capabilities, and `supportedTargetRuntimes`. The registry assembles definitions for palette, inspector, verification, help, schema generation, and package export.

Do not create a second platform-compatibility list. Absence of `supportedTargetRuntimes` means all targets; restrictions must be explicit on the definition. Contract tests ensure the runner recognizes the same executable node types and capabilities.

Use existing shadcn-style UI primitives, Lucide icons, and established inspector field components. Keep operational screens compact and responsive, avoid nested cards and horizontal page scrolling, and verify behavior at desktop and narrow window sizes.

The help modal intentionally provides concise registry-derived reference material. The public wiki contains long-form documentation and is linked from the modal navigation.

After definition changes, regenerate node schemas, inspect the diff, run unit/type/lint/build checks, and exercise package export plus runner validation.

## Changing the package contract

Package compatibility is cross-language. Prefer additive changes with explicit defaults. Breaking changes require a format or language version decision before implementation.

For a node-contract change:

1. Update the editor definition, fields, outputs, capabilities, risk, and target runtimes.
2. Regenerate and review its JSON Schema.
3. Update Rust models or semantic validation when shape or invariants changed.
4. Update security capability derivation.
5. Implement runtime dispatch and the native action or trigger.
6. Add editor contract, schema, Rust unit, integration, and platform tests.
7. Update public behavior documentation.

The runner must validate packages offline from committed or compiled contracts. Do not introduce runtime schema downloads.
