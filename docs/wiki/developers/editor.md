---
title: Editor and Package Development
description: Extend the editor, node registry, schemas, and cross-language package contract.
tags: [developers, editor]
---
# Editor and Package Development

The editor uses Next.js 16, React 19, React Flow, Tailwind CSS 4, Lucide icons, and the repository's shadcn-style primitives. `apps/editor/app` owns routing and page composition, `components` owns UI, `data` owns registry/project definitions, `utils` owns verification/simulation/package behavior, and `tests` owns Node contract tests and Playwright workflows.

The root route is a local project home. Project routes use `/projects/{projectId}`. Durable project records, binary assets, and global editor preferences live in the versioned `baudbound-editor` IndexedDB database behind `data/storage` repositories. Do not add normal `localStorage` or `sessionStorage` persistence. The only localStorage access is the one-time panel-preference migration, which deletes legacy values after its IndexedDB transaction commits.

Project identity is immutable and separate from editable settings. Exports reuse the stored UUID and creation timestamp so the runner can recognize later packages as revisions of the same script. Import conflicts must offer open, replace, or independent-copy behavior explicitly.

Project routes coordinate one writable tab through `BroadcastChannel`. Takeover is denied while the owner is dirty, stale owners expire after missed heartbeats, and IndexedDB revision checks remain the final protection against stale writes. Save transactions must preserve the previous committed revision on failure and keep the current document available for package-export recovery.

Node definitions under `apps/editor/data/nodes/definitions` own display metadata, category, configuration fields, outputs, risk, capabilities, and `supportedTargetRuntimes`. The registry assembles definitions for palette, inspector, verification, help, schema generation, and package export.

Do not create a second platform-compatibility list. Absence of `supportedTargetRuntimes` means all targets; restrictions must be explicit on the definition. Contract tests ensure the runner recognizes the same executable node types and capabilities.

Use existing shadcn-style UI primitives, Lucide icons, and established inspector field components. Keep operational screens compact and responsive, avoid nested cards and horizontal page scrolling, and verify behavior at desktop and narrow window sizes.

The help modal intentionally provides concise registry-derived reference material. The public wiki contains long-form documentation and is linked from the modal navigation.

After definition changes, regenerate node schemas, inspect the diff, run unit/type/lint/build checks, and exercise package export plus runner validation.

## Add or change a node

1. Add or edit one definition under `apps/editor/data/nodes/definitions/{triggers,control,actions}`.
2. Define the stable `actionType`, label, description, group, defaults, config fields, execution ports, runtime outputs, risk, permission/capability derivation, fallibility, and target restrictions.
3. Register the definition in `data/nodes/registry.ts`; do not duplicate its fields in another catalog.
4. Add inspector UI only when the standard config-field model cannot express the interaction cleanly.
5. Implement or update verification and simulation behavior. Simulation must label browser approximations and must not imply unsupported runner behavior.
6. Generate schemas and inspect the exact node schema plus `program.schema.json` references.
7. Update Rust parsing, security derivation, runtime dispatch, and native implementation before presenting the node as supported.
8. Add contract, schema, runtime, rejection, and target-platform tests.
9. Add the action type and full behavior to [Node Reference](../editor/node-reference.md). The wiki coverage gate rejects an undocumented definition.

Run:

```text
pnpm --dir apps/editor schemas:generate
pnpm --dir apps/editor schemas:check
pnpm --dir apps/editor test
```

## Package export flow

The export path takes current project state, sanitizes node config, verifies graph and target rules, derives declared access, separates executable `program.json` from `editor.json`, writes manifest assets and secret declarations, and creates the ZIP archive. The runner later repeats security and semantic checks independently.

Asset IDs and package paths must remain stable and normalized. Secret values never enter package state. Comments, positions, dimensions, and edge style remain editor metadata and cannot affect runner execution. Edge `execution_order` is executable program data. It must remain unique and consecutive for every source node and source handle.

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

## Format and language versions

Change package format version when old runners cannot safely parse the archive/document contract. Change script language version when the same valid graph would execute with incompatible semantics. An additive optional field with a safe default may require neither, but the decision must be explicit and tested against older fixtures.

## UI quality and accessibility

- Reuse existing primitives and keyboard behavior.
- Use labels, descriptions, focus states, and semantic controls; do not encode status only by color.
- Keep operational UI compact and responsive without horizontal page scrolling.
- Preserve canvas dimensions and hit targets across hover/selection states.
- Test wide and narrow viewports and critical workflows with Playwright screenshots.
- Avoid explanatory marketing panels inside the working editor; put detailed guidance in Help and the wiki.

The release gate is `pnpm --dir apps/editor verify:release`.
