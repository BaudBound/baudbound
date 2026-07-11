---
title: Packages and Assets
description: Configure project metadata, include assets, and export or import BaudBound packages.
tags: [editor, packages]
---
# Packages and Assets

The editor imports and exports `.bbs` package files. A package is both the runner artifact and a portable editor project.

## Before export

Set a clear project name, author, description, version requirement, website or repository when applicable, and the intended target runtime. Resolve all verification errors and review capability warnings.

## Assets

Files used by a workflow can be embedded in the package asset area. Runtime nodes refer to packaged assets through the package contract rather than assuming the original editor machine's path. Keep secrets and machine credentials out of assets.

## Editor metadata

Canvas positions, comments, visual edge style, and other editing-only state are stored in `editor.json`. The runner ignores comments and presentation settings. Executable behavior is stored in `program.json` and package identity in `manifest.json`.

## Integrity

Export generates package integrity metadata. The runner validates hashes when importing and before trusting installed content. Repacking or editing files inside `.bbs` makes the package unverified. Make changes in the editor and export a new package instead.

See [Package format](../package-format/index.md) for the archive contract and [Scripts and approvals](../runner/scripts-approvals.md) for updates.
