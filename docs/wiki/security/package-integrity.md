---
title: Package Integrity
description: How BaudBound hashes, validates, installs, and rechecks package content.
tags: [security, packages, integrity]
---
# Package Integrity

The editor writes an integrity document covering protected package content. Import verifies every declared digest and rejects missing, extra, duplicated, or mismatched content according to the package contract.

The runner computes a package-level SHA-256 identity for durable installation and approval binding. Before execution, controlled storage verifies that the installed package still matches its recorded hash. A file modified outside the runner is not silently trusted.

Integrity is not publisher identity. Hashes prove content consistency, while release signatures verify runner artifacts and operator approval accepts a package's behavior. All three controls serve different purposes.

To change a workflow, edit it in the editor, export a new `.bbs`, run `script update`, and review the new revision. Do not patch ZIP entries or copy files into an installed archive.
