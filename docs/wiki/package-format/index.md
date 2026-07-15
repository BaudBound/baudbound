---
title: BBS Package Format
description: Understand BaudBound archive documents, assets, validation, hashing, schemas, and compatibility.
tags: [packages, format, schemas]
---
# BBS Package Format

A `.bbs` file is a ZIP archive used to move an editor project into a runner. The archive has a strict root layout, JSON documents for executable and review data, optional editor metadata, and optional declared assets. The runner must be able to validate it without contacting the public schema server.

Do not manually edit or repack a `.bbs` file. Make changes in the editor and export a new revision. Repacking can change package bytes, break stored-hash approval, create unsupported archive metadata, or make package declarations disagree with the graph.
{.is-warning}

## Archive layout

| Entry | Required | Purpose |
| --- | --- | --- |
| `manifest.json` | Yes | Script identity, package/language versions, author metadata, runner requirement, assets, and secret declarations |
| `program.json` | Yes | Executable nodes, edges, and entry trigger |
| `permissions.json` | Yes | Declared permission list and highest risk level |
| `capabilities.json` | Yes | Required capability list and target runtime |
| `editor.json` | No | Non-executable canvas positions, comments, dimensions, and editor preferences |
| `README.md` | No | Package-local human notes accepted by the parser |
| `assets/` | No | Files declared by `manifest.json` and referenced by asset ID or normalized path |

No other root files are accepted. Directory entries do not count as package documents.

## Document fields

### `manifest.json`

| Field | Meaning |
| --- | --- |
| `format_version` | Version of the archive/document contract |
| `script_language_version` | Version of executable graph semantics |
| `id` | Stable script identity used to match updates |
| `name`, `description`, `author` | Human-facing identity and explanation |
| `website`, `repository`, `tags` | Optional discovery and source metadata |
| `created_with`, `created_at`, `updated_at` | Exporting editor and timestamps |
| `minimum_runner_version` | Oldest runner version allowed to execute the package |
| `assets` | Declared asset records with ID, kind, media type, name, path, and byte size |
| `secrets` | Name, type, description, and required flag; never a secret value |

An update must keep the same `id`. A display name is not a stable update key and may not uniquely identify a script.

### `program.json`

The program contains the executable graph. Each node has a unique ID, action type, and action-specific config. Edges connect named execution ports, and the entry identifies the trigger that starts the graph. The per-node config rules come from the generated node schemas and runner semantic validation.

Every edge contains these execution fields:

| Field | Meaning |
| --- | --- |
| `source` | ID of the node whose output starts the connection |
| `source_handle` | Named output on the source node |
| `target` | ID of the destination node |
| `target_handle` | Named input on the destination node |
| `execution_order` | Zero-based position among edges with the same `source` and `source_handle` |

For each source output, `execution_order` values must be unique and consecutive from `0`. A single connection uses `0`. Three connections use `0`, `1`, and `2`. The editor displays those positions as 1, 2, and 3 for users. The simulator and runner execute the destinations sequentially in that order.

Missing, duplicate, negative, or gapped orders are invalid. A node cannot connect an output to one of its own inputs. The runner enforces these rules independently during package validation and again when constructing the runtime graph.

Canvas coordinates and comment presentation are not executable concerns and belong in `editor.json`.

### Access documents

`permissions.json` contains `declared_permissions` and `risk_level`. `capabilities.json` contains `required_capabilities` and `target_runtime`. The runner independently derives all four concepts from the graph and rejects an understatement, overstatement, duplicate, unsupported target, or false risk level.

Read [Approvals, Capabilities, and Risk](../security/approvals-capabilities.md) for their operator meaning.

## Assets

Every file under `assets/` must have one matching manifest declaration, and every declaration must have one file. Paths must use normalized forward-slash package paths beneath `assets/`. Absolute paths, `..` traversal, backslashes, duplicate case-insensitive paths, undeclared assets, and missing assets are rejected.

The runner accepts the package asset extensions supported by the parser, including common text/data, image, and audio formats. The declared `size` must match the actual uncompressed byte size. Runtime actions read assets by declared ID or package path.

Credentials do not belong in assets. Declare them as secrets and configure values on the runner.

## Archive safety limits

The parser rejects duplicate and case-colliding entries, unsupported root files, invalid asset paths, and directories presented as files. It also enforces package entry and size limits before trusting data. JSON shape validation is only the first layer; semantic checks cover relationships that JSON Schema cannot express safely.

## Hashing and approval

The current package format does not contain a package-author signature or an internal integrity document. On import or update, runner storage computes SHA-256 over the exact `.bbs` bytes and records that package hash. The managed package is hashed again before trust-sensitive operations. Approval binds to that stored hash, so a changed archive requires a normal update and new approval.

This proves consistency of the installed copy, not author identity. See [Security Model](../security/index.md).

## Versions and compatibility

These values answer different questions:

| Value | Question |
| --- | --- |
| Package format version | Can the runner parse this archive contract? |
| Script language version | Does the runner understand this graph behavior? |
| Minimum runner version | Is this runner release new enough for the package? |
| Target runtime | Is this runner environment allowed and natively capable? |

The runner rejects unknown future formats, unsupported language versions, unmet minimum versions, and incompatible targets before execution.

## JSON Schemas

Committed schemas cover `manifest.json`, `program.json`, `permissions.json`, `capabilities.json`, `editor.json`, and every registered node config. Each schema has a stable HTTPS `$id` beneath `https://schemas.baudbound.app/`. `program.schema.json` references per-node schemas by those identifiers.

The public schema host helps editors and external tools. Runner validation remains offline: Rust models and semantic validators are compiled into the runner and do not download schemas during import or execution.

## Contract change checklist

When changing the package or a node contract:

1. Update the editor node definition or package builder source of truth.
2. Regenerate per-node schemas and inspect the diff.
3. Update Rust package models and semantic validation.
4. Update security derivation, runtime dispatch, and native implementation.
5. Decide whether the format or language version must change.
6. Add editor contract, schema, Rust, integration, and platform tests.
7. Update the Node Reference and this package documentation.
8. Confirm old supported packages still validate or document an intentional compatibility break.

Use `pnpm --dir apps/editor schemas:check` and the runner workspace tests before merging. [Editor and Package Development](../developers/editor.md) owns the contributor workflow.
