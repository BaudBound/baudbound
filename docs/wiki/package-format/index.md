---
title: BBS Package Format
description: Structure, validation, integrity, and compatibility rules for BaudBound script packages.
tags: [packages, format]
---
# BBS Package Format

A `.bbs` file is a ZIP archive with a strict allowlist of root documents and declared assets. It is designed to be portable between the web editor and Windows or Linux runners.

Core documents include `manifest.json`, `program.json`, `permissions.json`, `capabilities.json`, `integrity.json`, and `editor.json`. The manifest identifies the package, target runtime, version requirements, assets, and secret declarations. The program contains executable nodes and edges. Permissions, capabilities, and risk are generated from the graph and independently recalculated by the runner. Editor metadata is non-executable.

Asset paths must be normalized package-relative paths. Absolute paths, traversal, duplicate archive entries, unexpected files, undeclared assets, and missing declared assets are rejected. Extraction is bounded to defend against malformed archives and decompression abuse.

Package format version and minimum runner version are separate concepts. The runner rejects unsupported format versions and packages requiring a newer runner.

See [Package contracts](contracts.md), [Package integrity](../security/package-integrity.md), and [Editor export](../editor/packages-assets.md).
