---
title: Release Engineering
description: Version, verify, sign, package, publish, and update BaudBound releases.
tags: [developers, releases]
---
# Release Engineering

BaudBound versions must agree across the Cargo workspace, Tauri configuration, and desktop UI package metadata. Release tags use `vMAJOR.MINOR.PATCH` and are created from a clean, verified `master` commit.

`apps/baudbound/scripts/verify-release-version.mjs` checks that the tag, root Cargo workspace, `apps/baudbound/tauri.conf.json`, and `apps/baudbound/ui/package.json` agree before packaging.

The release workflow builds a Windows NSIS installer and Linux AppImage, signs updater artifacts, creates a draft GitHub release, and publishes `latest.json` with platform URLs and signatures. The private updater key and password live in protected GitHub secrets; only the public key is committed in application configuration.

Use the interactive release helper:

```powershell
./tools/runner-release.ps1
```

It can check versions and prerequisites, run verification, create the tag, inspect artifacts, and publish only after explicit confirmation. The internal maintainer procedure remains in [docs/runner-release.md](https://github.com/NATroutter/BaudBound/blob/master/docs/runner-release.md) because it contains repository release operations rather than public product usage.

## Release workflow

1. Verify a clean `master` worktree and synchronized version metadata.
2. Run Rust, editor contract/schema, and desktop UI release gates.
3. Create and push `vMAJOR.MINOR.PATCH`.
4. GitHub repeats the quality gate on Ubuntu.
5. Windows builds the NSIS installer; Ubuntu 22.04 builds the AppImage.
6. Tauri signs updater artifacts and uploads platform signatures.
7. The workflow creates a **draft** GitHub release and generates `latest.json` with platform URLs, versions, and signatures.
8. A maintainer checks artifacts on clean supported machines, release notes, updater metadata, and install/update behavior.
9. Publish the draft only after every artifact passes.

The draft prevents a partially uploaded release from becoming the automatic update target.

## Protected settings

| Setting | Purpose |
| --- | --- |
| `GITHUB_TOKEN` | Create the draft and upload artifacts in the repository |
| `TAURI_SIGNING_PRIVATE_KEY` | Sign updater packages; never commit it |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Decrypt the private signing key in CI |

Loss of the private updater key prevents producing updates trusted by existing installations. Exposure allows an attacker with release publication access to sign malicious updates. Back it up offline, restrict the GitHub environment, and rotate only through a deliberately designed migration.

## Review and rollback

Install the Windows artifact on a clean supported Windows machine and launch the Linux AppImage on representative supported systems. Check first launch, config initialization, import/approval/run, tray/background behavior, signed update discovery, download progress, restart, and resulting version.

If the draft is broken, leave it unpublished, fix the source, and create a new patch version. Do not replace published artifact bytes under an existing version because clients and signatures may already reference them. For a published defect, remove it from update discovery when necessary and ship a new signed patch release.
