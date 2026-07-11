---
title: Editor Development
description: Extend the BaudBound editor and its registry-driven node system.
tags: [developers, editor]
---
# Editor Development

Node definitions under `apps/editor/data/nodes/definitions` own display metadata, category, configuration fields, outputs, risk, capabilities, and `supportedTargetRuntimes`. The registry assembles definitions for palette, inspector, verification, help, schema generation, and package export.

Do not create a second platform-compatibility list. Absence of `supportedTargetRuntimes` means all targets; restrictions must be explicit on the definition. Contract tests ensure the runner recognizes the same executable node types and capabilities.

Use existing shadcn-style UI primitives, Lucide icons, and established inspector field components. Keep operational screens compact and responsive, avoid nested cards and horizontal page scrolling, and verify behavior at desktop and narrow window sizes.

The help modal intentionally provides concise registry-derived reference material. The public wiki contains long-form documentation and is linked from the modal navigation.

After definition changes, regenerate node schemas, inspect the diff, run unit/type/lint/build checks, and exercise package export plus runner validation.
