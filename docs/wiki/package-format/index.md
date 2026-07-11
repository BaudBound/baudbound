---
title: BBS Package Format
description: Structure, validation, integrity, schemas, and compatibility rules for BaudBound packages.
tags: [packages, format, schemas]
---
# BBS Package Format

A `.bbs` file is a ZIP archive shared by the editor and runner. It contains a strict set of package documents and optional declared assets.

## Package documents

`manifest.json` identifies the package, target runtime, minimum runner version, assets, and secret declarations. `program.json` contains executable nodes and edges. Permission, capability, and risk documents describe access requested by the graph. `editor.json` contains non-executable canvas and presentation state. Integrity metadata protects package content.

Asset paths must be normalized and package-relative. The runner rejects absolute paths, traversal, duplicate archive entries, unexpected files, undeclared assets, and missing declared assets.

Package format version and minimum runner version are separate. The runner rejects unsupported formats and packages requiring a newer runner.

## Integrity

Export records digests for protected package content. Import verifies every required digest and rejects missing, extra, duplicated, or mismatched content. The runner also records a package-level hash and checks installed content before trusting an approval.

Integrity proves content consistency, not publisher identity. Runner release signatures verify runner artifacts, while operator approval accepts one package revision and its requested access.

To change a workflow, edit it in the editor, export a new `.bbs`, run `script update`, and review the new revision. Do not patch or repack an installed archive.

## Schemas and semantic validation

JSON Schemas under `schemas/` define package documents. `program.schema.json` references generated per-node schemas through stable identifiers hosted at `https://schemas.baudbound.app/`.

Schemas validate shape and basic constraints. The runner additionally validates graph uniqueness and reachability, edge contracts, variable references, package file relationships, capability truthfulness, target compatibility, and runtime version policy. Validation works offline and does not depend on the public schema host.

## Changing the contract

The editor node registry owns fields, outputs, targets, capabilities, risk, and schema generation. A node-contract change must update the editor definition, generated schema, runner models or semantic validation, security derivation, runtime implementation, tests, and documentation together.
