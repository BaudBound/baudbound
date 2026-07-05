# BaudBound Editor Production Hardening Plan

This document is the fix plan from the fresh production audit of the current editor codebase. The goal is to move the editor from a functional alpha into a production-quality tool that can be trusted by real users and by the runner package format.

The order matters. Correctness, safety, and package contract strictness come before more feature work.

## 1. Simulator Stability

Goal: stop memory growth and make long-running simulations reliable.

### Problems To Fix

- The simulator currently retains emitted steps internally even though the UI caps visible logs.
- Long or repeated simulations can continue growing memory.
- Graph execution is recursion-heavy, which is dangerous now that execution safety limits were intentionally removed.
- Simulation lifecycle state is spread across editor state and utility code.

### Implementation Plan

1. Replace retained simulation step history with streaming output.
   - Stop storing normal simulation output in `context.steps`.
   - Stream every step through `onStep`.
   - Return only final status, final variables, failure reason, and last executed node.
   - Keep UI log caps in the UI layer only.

2. Rewrite graph execution as an iterative engine.
   - Replace recursive node execution with an explicit queue.
   - Queue items should include node id, selected output handle, trigger payload, loop state, and runtime context.
   - Check `AbortSignal` before each node and before each async operation.
   - Yield to the browser regularly so the UI stays responsive.

3. Define a single simulator lifecycle model.
   - `idle`
   - `verifying`
   - `running`
   - `waiting_for_trigger`
   - `stopping`
   - `stopped`
   - `failed`
   - `completed`

4. Make side effects execute at the same time as node execution.
   - Log output, beeps, sounds, toasts, dialogs, and variable updates must happen when the node is reached.
   - Do not replay simulation logs after the fact.

### Acceptance Criteria

- A long loop does not grow memory without bound.
- Stop button interrupts running simulation quickly.
- Repeated trigger simulations can run for a long time without stack growth.
- Logs appear at the time the simulated action executes.

## 2. Built-In Variables And Runtime Context

Goal: variables behave consistently across editor UI, simulation, export, verification, and docs.

### Problems To Fix

- Manifest and system built-in variables exist and are documented, but simulation does not resolve all of them consistently.
- Variable information is assembled in multiple places.
- Read-only variable rules must be enforced everywhere.

### Implementation Plan

1. Create one canonical variable model.

```ts
type EditorVariableDefinition = {
	name: string;
	token: string;
	type: VariableType;
	scope: VariableScope;
	readOnly: boolean;
	source: "manifest" | "system" | "runtime" | "node_output" | "user";
	value?: JsonValue;
	description: string;
};
```

2. Build a single variable registry function.
   - Input: project settings, nodes, runtime variables, node outputs.
   - Output: all known variables.
   - Use this for autocomplete, variable tab, help docs, simulation, and verification.

3. Resolve built-ins in simulation.
   - Pass project settings into simulation.
   - Resolve `{{manifest_name}}`, `{{manifest_author}}`, `{{manifest_version}}`, and related manifest values.
   - Provide deterministic simulated values for `{{system_os}}`, `{{system_date}}`, `{{system_time}}`, and other system variables.

4. Enforce read-only variables.
   - `manifest_*` and `system_*` cannot be written.
   - Node output references cannot be written.
   - Verification rejects illegal writes.
   - Inspector shows immediate field errors.
   - Import rejects packages that attempt illegal writes.

### Acceptance Criteria

- Every variable shown in the variable tab resolves in simulation.
- Built-in variables are read-only in UI and verification.
- Autocomplete, docs, and simulation use the same variable source.

## 3. Package Import And Asset Security

Goal: imported `.bbs` packages must be treated as hostile input.

### Problems To Fix

- Asset size/count limits exist for editor-added files, but imported zip assets need stronger preflight checks.
- Imported asset blobs may be read before enough size validation has happened.
- Asset validation should not trust file extensions.

### Implementation Plan

1. Add strict zip preflight validation.
   - Reject too many files.
   - Reject unknown top-level paths.
   - Reject path traversal.
   - Reject absolute paths.
   - Reject duplicate normalized paths.
   - Reject names with control characters.

2. Enforce size limits before reading blobs where possible.
   - Check uncompressed size from JSZip metadata when available.
   - Enforce per-asset max size.
   - Enforce total asset max size.
   - Compare manifest-declared size with actual size.

3. Harden asset content validation.
   - Keep magic-byte checks.
   - Verify extension, MIME, and bytes agree.
   - Reject unsupported asset types.
   - Reject executable or ambiguous content.

4. Add malicious package tests.
   - Oversized asset.
   - Too many assets.
   - Wrong extension with executable bytes.
   - Path traversal.
   - Missing manifest.
   - Invalid editor metadata.
   - Duplicate asset paths.

### Acceptance Criteria

- A malicious `.bbs` cannot force large unbounded blob allocations.
- Asset validation rejects files by content, not only extension.
- Import tests cover malicious package cases.

## 4. Target Runtime Compatibility

Goal: target runtime compatibility must be enforced, not only hinted in the UI.

### Problems To Fix

- Some nodes are disabled in the sidebar for non-desktop targets, but existing incompatible nodes can still be imported, pasted, duplicated, or left after target changes.
- Verification does not fully enforce runtime capability compatibility.

### Implementation Plan

1. Define target runtime capability profiles.
   - Generic Headless
   - Windows Headless
   - Linux Headless
   - macOS Background
   - Generic Desktop
   - Windows Desktop
   - Linux Desktop
   - macOS Desktop

2. Map every node to runtime requirements.
   - Desktop input nodes require desktop runtime.
   - Window nodes require window-management capability.
   - Serial nodes require serial capability.
   - Sound, notification, message box, keyboard, mouse, clipboard, and process nodes need explicit compatibility rules.

3. Enforce compatibility in verification.
   - Existing incompatible nodes fail verification.
   - Target changes reset verification status.
   - Import rejects incompatible packages or presents a blocking error.

4. Improve UI feedback.
   - Disabled sidebar nodes explain why they are unavailable.
   - Incompatible existing nodes are marked on canvas and in the inspector.

### Acceptance Criteria

- A headless target cannot export desktop-only nodes.
- Changing target runtime correctly invalidates verification.
- Import cannot silently load incompatible scripts.

## 5. Strict Node Contracts

Goal: every node fully defines itself in one place.

### Problems To Fix

- Node definitions are improving, but simulation and verification still contain too much node-specific behavior.
- It is still possible to half-add a node.

### Implementation Plan

1. Extend `NodeDefinition` so each node owns:
   - display metadata
   - icon
   - group
   - risk
   - permission
   - capabilities
   - default config
   - config fields
   - config validation
   - runtime outputs
   - ports
   - export serialization
   - simulation behavior
   - help/docs content
   - examples
   - target runtime compatibility

2. Move node-specific verification into node files.
   - Central verification should orchestrate checks.
   - Node files should validate their own config.

3. Move node-specific simulation into node files.
   - Simulation engine handles traversal and context.
   - Node definitions handle node execution behavior.

4. Move node-specific export serialization into node files.
   - Package export asks each definition how to serialize itself.

5. Keep shared helpers only for genuinely shared concepts.
   - No dumping-ground files.
   - No tiny files unless the concept is real and stable.

### Acceptance Criteria

- Adding a new node requires editing one node definition file and registry import only.
- Verification, simulation, docs, runtime outputs, and export behavior come from the node definition.
- A node without required production metadata fails tests.

## 6. Schema And Package Contract

Goal: schemas must match the real exported package format.

### Problems To Fix

- `program.schema.json` allows broad config objects.
- The package contract validates shape, but not exact node-specific config.
- Runner-facing contracts are not strict enough.

### Implementation Plan

1. Generate or compose node config schemas from node definitions.
   - Each node owns a JSON schema fragment.
   - `program.schema.json` references exact schemas by `action_type`.

2. Make config strict.
   - Reject unknown fields.
   - Enforce required fields.
   - Enforce enums.
   - Enforce numeric ranges.
   - Document variable-capable fields.

3. Strengthen package contract validation.
   - Validate exact node config by action type.
   - Validate runtime outputs by action type.
   - Validate permissions and capabilities against used nodes.

4. Validate editor metadata separately.
   - Keep canvas positioning outside runner-critical `program.json`.
   - Validate with `editor.schema.json`.

### Acceptance Criteria

- Invalid node config cannot pass schema validation.
- Exported packages validate against strict schemas.
- Runner can rely on package schemas as a real contract.

## 7. Graph Verification

Goal: invalid graphs cannot export.

### Problems To Fix

- Loop topology is not fully enforced.
- Dynamic ports can become stale after config changes.
- Existing verification does not deeply validate all graph semantics.

### Implementation Plan

1. Enforce trigger rules.
   - Single manual trigger.
   - Valid schedule config.
   - Trigger-specific required config.

2. Enforce port compatibility.
   - Edges must reference existing nodes.
   - Source handles must exist.
   - Target handles must exist.
   - Dynamic switch outputs must match current cases.

3. Enforce loop topology.
   - `loop` output must eventually return to the loop input.
   - Body path must be structurally valid.
   - `done` path must not be mixed into loop body.

4. Enforce `for_each` topology.
   - Same body-return rule as loop.
   - Validate item variable name.
   - Validate list input expression.

5. Enforce branch correctness.
   - If/Else branches must use valid handles.
   - Switch edges to deleted cases must be rejected.

### Acceptance Criteria

- Broken loop graphs fail verification.
- Deleted switch case handles cannot remain valid.
- All edges are validated against current node ports.

## 8. Import And Export Workflow

Goal: users cannot export invalid or stale packages.

### Problems To Fix

- Verification status can become stale if not reset after every meaningful change.
- Import currently accepts warning-level results without enough policy clarity.
- Export flow must always represent the current graph state.

### Implementation Plan

1. Reset verification on every meaningful change.
   - Nodes
   - Edges
   - Project settings
   - Assets
   - Target runtime
   - Variables
   - Serial config

2. Export wizard should verify the current state.
   - No stale verification summary.
   - Download disabled until the current state passes.

3. Import should run full verification before loading.
   - Fatal errors reject.
   - Warnings require explicit user confirmation or a clear policy decision.

4. Package contents preview should match the actual zip.
   - Manifest
   - Program
   - Permissions
   - Capabilities
   - Editor metadata
   - Assets tree
   - README

### Acceptance Criteria

- Export cannot use stale verification state.
- Import policy for warnings is explicit and tested.
- Package preview matches generated package contents.

## 9. Documentation System

Goal: docs should not drift from implementation.

### Problems To Fix

- Help docs are manually maintained and can miss node behavior.
- Users need discoverable docs for node config, runtime output, simulation, permissions, and expressions.

### Implementation Plan

1. Generate node docs from node definitions.
   - Node purpose.
   - Config fields.
   - Runtime outputs.
   - Failure outputs.
   - Permissions.
   - Capabilities.
   - Simulation behavior.

2. Keep manual docs for concepts only.
   - Variables.
   - Runtime context.
   - Package format.
   - Import/export.
   - Simulation.
   - Controls.
   - Security model.

3. Improve expression docs.
   - Separate Calculate expression syntax from If/Else dropdown operators.
   - Document available math functions.
   - Document `^` behavior.
   - Document variable interpolation inside expressions.

4. Add contextual help links.
   - Inspector node header links to node docs.
   - Verification errors link to relevant help sections.

### Acceptance Criteria

- Every node appears in help docs automatically.
- Docs describe runtime outputs and failure behavior for every node.
- Expression docs no longer confuse dropdown comparison operators with typed formulas.

## 10. Test Coverage

Goal: tests should catch broken behavior before users do.

### Test Groups To Add

1. Package contract tests.
   - Valid package fixtures.
   - Invalid package fixtures.
   - Asset package fixtures.
   - Editor metadata fixtures.
   - Schema compatibility.

2. Node definition tests.
   - Every node has required metadata.
   - Every node has config validation.
   - Every node has runtime output docs.
   - Every fallible node has failure behavior.

3. Verification tests.
   - Manual trigger limit.
   - Invalid variable writes.
   - Target runtime incompatibility.
   - Invalid loops.
   - Invalid switch handles.
   - Invalid asset references.

4. Simulation tests.
   - Branching.
   - Loops.
   - For-each.
   - Variable operations.
   - Runtime output references.
   - Built-in variables.
   - Failure overrides.
   - Stop/abort behavior.

5. UI tests with Playwright.
   - Add node.
   - Connect nodes.
   - Delete edge.
   - Copy/paste node.
   - Import invalid package.
   - Export wizard.
   - Asset editor.
   - Project settings dialog.
   - Variable autocomplete.

### Acceptance Criteria

- New production risks are covered by tests.
- Every future node addition has automated coverage requirements.
- Import/export and simulation cannot regress silently.

## 11. Code Organization

Goal: readable structure without fake cleanup that only scatters code.

### Rules

- Do not split files only because they are long.
- Split when one file owns multiple independent responsibilities.
- Keep related UI pieces together.
- Keep node-specific behavior in node definition files.
- Avoid vague folders like `features` when the app is only the editor.

### Recommended Structure

```txt
apps/editor/
  app/
    page.tsx
    editor-page.tsx

  components/
    canvas/
    inspector/
    modals/
    shell/
    simulation/
    ui/

  data/
    nodes/
      definitions/
        actions/
        control/
        triggers/
      node-definition.ts
      registry.ts

    project/
      assets.ts
      built-in-variables.ts
      runtimes.ts
      serial.ts
      variables.ts

    editor/
      key-reference.ts
      panel-layout.ts

  utils/
    package/
      import.ts
      export.ts
      contract.ts
      assets.ts

    simulation/
      engine.ts
      context.ts
      template.ts
      side-effects.ts

    verification/
      verify.ts
      graph.ts
      package.ts
      target-runtime.ts
```

### Acceptance Criteria

- `simulation.ts` no longer owns all node behavior.
- `verification.ts` no longer owns all node behavior.
- `editor-page.tsx` is mostly orchestration, not business logic.
- Folder names describe real app concepts.

## 12. Release Readiness

Before calling the editor production-ready:

1. `pnpm lint` passes.
2. `pnpm typecheck` passes.
3. `pnpm test` passes.
4. `pnpm build` passes.
5. Playwright smoke suite passes.
6. Import/export package fixtures validate against schemas.
7. Memory test proves long simulations do not grow unbounded.
8. Malicious package tests pass.
9. Every node has validation, schema, docs, simulation behavior, runtime outputs, and permission/capability metadata.
10. Docker build is tested.
11. README explains local-only editor behavior and package security model.

## Priority Order

1. Simulator memory and iterative execution.
2. Built-in variable resolution.
3. Asset import hardening.
4. Target runtime verification.
5. Strict node-owned validation and schema fragments.
6. Loop and graph topology verification.
7. Tests for all of the above.
8. Documentation generation from node definitions.
9. Package import/export cleanup.
10. UI polish and remaining organization work.

## Production Standard

The editor should not rely on "good enough" assumptions. The package format must be strict, import must treat all files as hostile, simulation must not leak memory, and every node must be fully defined, validated, documented, simulated, and tested.

