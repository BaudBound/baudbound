---
title: Package Contracts and Schemas
description: JSON Schema, node schemas, semantic validation, and editor-runner contract enforcement.
tags: [packages, schemas, developers]
---
# Package Contracts and Schemas

Committed JSON Schemas under `schemas/` define the language and package documents. `program.schema.json` dispatches node configuration to generated per-node schemas through URL-based `$ref` identifiers hosted at `https://schemas.baudbound.app/`.

The editor node registry and node definitions are the source of truth for palette metadata, fields, outputs, target runtimes, capabilities, risk, and schema generation. `apps/editor/scripts/generate-node-schemas.mjs` produces the committed node schemas. CI fails when generated output differs from source.

JSON Schema validates shape and basic constraints. The Rust runner also performs semantic validation that cannot be represented cleanly by schema alone: graph uniqueness and reachability, edge and branch contracts, variable references, package file relationships, capability truthfulness, target compatibility, and runtime version policy.

Editor contract tests compare registry behavior, generated schemas, and runner-owned capability support. Adding or changing a node requires synchronized implementation and tests across the editor definition, generated schema, package model when necessary, security mapping, runtime dispatch, and native action or trigger handler.

Schema URLs are stable identifiers for cross-language consumers. The repository-relative source files remain usable by offline validators and the Rust runner does not depend on network access to validate installed packages.
