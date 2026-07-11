---
title: Installing the Runner
description: Install BaudBound on supported Windows and Linux systems.
tags:
  - installation
  - runner
---
# Installing the Runner

BaudBound 2.0 supports Windows and Linux. Download release artifacts from the [BaudBound GitHub releases](https://github.com/NATroutter/BaudBound/releases).

## Windows

Download and run the NSIS setup executable. The installer creates normal application shortcuts and an uninstall entry. Early releases may display a Windows unsigned-publisher warning; Tauri updater packages are still cryptographically signed and verified independently.

## Linux desktop

Download the AppImage, make it executable, and launch it:

```bash
chmod +x BaudBound_2.0.0_amd64.AppImage
./BaudBound_2.0.0_amd64.AppImage
```

The AppImage is the primary cross-distribution desktop artifact for Debian, Fedora, Arch, and related distributions.

## First launch

The runner creates its configuration automatically on first use. Desktop mode stores its encryption key in the operating-system credential vault. Headless secret configuration requires an explicit `BAUDBOUND_SECRET_KEY` environment variable.

Continue with the [runner quick start](quick-start.md).
