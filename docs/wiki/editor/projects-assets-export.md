---
title: Projects, Assets, and Export
description: Manage editor projects, metadata, assets, secrets, package import, and verified BBS export.
tags: [editor, projects, assets, export]
---
# Projects, Assets, and Export

This page covers the complete authoring lifecycle: creating a project, maintaining its metadata and embedded files, importing an existing `.bbs` package, and exporting a verified revision for the runner.

## Project identity and metadata

Open **Project Settings** from the editor top bar. These values are stored in the package manifest and shown during runner review.

| Setting | Purpose | Guidance |
| --- | --- | --- |
| **Name** | Human-readable project and package name | Required. Use a concise name that distinguishes the automation. |
| **Target Runtime** | Intended operating system and desktop/headless environment | Choose before adding platform-specific nodes. See [Target Runtimes](target-runtimes.md). |
| **Description** | Explains what the package does | State its trigger, side effects, and intended operator. |
| **Author** | Human-readable creator or organization | Informational; it is not a cryptographic signature. |
| **Minimum Runner** | Oldest runner version allowed to execute the package | Raise this only when the workflow depends on a newer contract or runtime feature. |
| **Website** | Optional project or product URL | Must be an HTTP or HTTPS URL. |
| **Repository** | Optional source repository URL | Use when reviewers can inspect the workflow's source or release process. |
| **Tags** | Search and classification labels | Press Enter, Space, Tab, or comma to finish a tag. |

The project has a stable manifest identity separate from its display name and exported filename. Importing an updated package with the same identity updates the installed script; a different identity creates another script.

Changing the display name does not intentionally create a new identity. Do not hand-edit package IDs to force an update.

## New projects and browser persistence

The editor keeps its current working project in browser-local storage. This protects against an ordinary page refresh but is not a durable backup. Browser-data clearing, private browsing, a different browser profile, or a storage failure can remove it.

Use **New** only after exporting work that must be retained. Export important revisions to files you back up outside the browser.

## Import an existing package

Choose **Import** and select a `.bbs` file when you need to inspect or continue editing an exported project.

Before replacing the current editor state, import checks package structure, document shape, graph identity, assets, and editor data. A rejected import leaves the current project unchanged.

A successful import restores executable nodes and edges along with editor-owned information such as node positions, comments, and edge style. It does not import production secret values from a runner.

After editing an imported package:

1. review Project Settings and retain the stable identity;
2. verify changed nodes, permissions, capabilities, and target support;
3. update the descriptive metadata when behavior changed;
4. export a new `.bbs` revision; and
5. use the runner's update workflow rather than importing it as an unrelated script.

See [Script Management](../runner/script-management.md) for update and approval consequences.

## Assets

Assets are files embedded inside the `.bbs` package for nodes that support package-owned content. The asset library shows the logical package path and file details used during export.

Use a short, unique filename with an appropriate extension. Keep assets as small as practical because they increase package size and import cost. A node should reference the asset through its asset selector rather than a machine-local path.

Removing an asset that is still referenced causes verification to fail. Replace references before removal.

> Never store passwords, API tokens, private keys, or other credentials as assets. Assets are ordinary package files and can be extracted by anyone who receives the `.bbs` file.
{.is-warning}

Use a [secret declaration](../runner/secrets.md) when the runner must supply sensitive data. The editor stores only the secret name, type, and description. Optional simulation values remain in the current browser session and are not exported.

## Executable and editor data

A package separates runtime behavior from editing information:

| Data | Examples | Used by runner execution |
| --- | --- | --- |
| Executable program | Triggers, actions, control flow, edges, configuration, runtime outputs | Yes |
| Manifest and declarations | Identity, versions, target, permissions, capabilities, risk, secret declarations | Yes |
| Assets | Package-owned files referenced by supported nodes | When referenced |
| Editor data | Canvas positions, comments, comment colors and font sizes, edge style | No |

Comments are documentation for people editing the graph. They are not executable actions and are filtered out of the program graph.

## Export wizard

Choose **Export** to open the three-stage wizard.

### 1. Project

Review the name, target, author, URLs, minimum runner version, generated filename, package-format version, runtime-language version, and archive contents. Return to Project Settings if any value is wrong.

### 2. Access

Review the calculated risk, permissions, and capabilities. These values are derived from current node definitions and configuration. A surprising permission usually means a node or option has more impact than expected; inspect the graph instead of dismissing the warning.

### 3. Verify

The editor checks graph structure, node configuration, variables, assets, target compatibility, permissions, capabilities, and package contracts. **Download .bbs** remains unavailable while a blocking check fails.

Warnings require review but may not block export. Read [Verification and Simulation](simulation.md) for the difference between failures, warnings, and risk notices.

After verification passes, choose **Download .bbs**. The browser saves the generated filename; its download location is controlled by browser settings.

## Integrity and archive handling

The exported file does not contain a package-author signature. On import, runner storage computes a SHA-256 hash of the exact archive bytes. Approval binds to that installed hash, and the runner checks the managed copy for later changes.

Do not unzip, edit, rename internal entries, or repack an exported `.bbs` file. Even a harmless-looking change can invalidate integrity, schemas, paths, or semantic contracts. Return to the editor and export another revision instead.

Integrity proves that package content still matches its declared digest. It does not prove who authored or published the package. Review behavior and provenance before approval.

## Package update checklist

- [ ] Preserve the existing script identity when this is an update.
- [ ] Update the description and version requirements when behavior changed.
- [ ] Confirm the target runtime still matches the deployment machine.
- [ ] Remove unused assets and verify every remaining asset reference.
- [ ] Confirm production credentials are secret declarations, not variables or assets.
- [ ] Simulate changed branches with representative payloads.
- [ ] Review new or changed permissions, capabilities, and risk.
- [ ] Export only after all blocking checks pass.
- [ ] Inspect the new package on the runner before approving the revision.
