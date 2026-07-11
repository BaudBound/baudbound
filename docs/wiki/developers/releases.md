---
title: Release Engineering
description: Version, verify, sign, package, publish, and update BaudBound releases.
tags: [developers, releases]
---
# Release Engineering

BaudBound versions must agree across the Cargo workspace, Tauri configuration, and desktop UI package metadata. Release tags use `vMAJOR.MINOR.PATCH` and are created from a clean, verified `master` commit.

The release workflow builds Windows and Linux artifacts, signs Tauri updater bundles, creates a draft GitHub release, and publishes `latest.json` with platform URLs and signatures. The private updater key and password live in protected GitHub secrets; only the public key is committed in application configuration.

Use the interactive release helper:

```powershell
./tools/runner-release.ps1
```

It can check versions and prerequisites, run verification, create the tag, inspect artifacts, and publish only after explicit confirmation. The internal maintainer procedure remains in [docs/runner-release.md](https://github.com/NATroutter/BaudBound/blob/master/docs/runner-release.md) because it contains repository release operations rather than public product usage.
