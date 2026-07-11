---
title: Runner Installation
description: Install BaudBound on supported Windows and Linux systems.
tags: [runner, installation]
---
# Runner Installation

## Supported systems

BaudBound supports 64-bit Windows and major Linux desktop or server distributions. Desktop builds require the platform webview and graphical dependencies used by Tauri. Linux releases are distributed in formats suitable for Debian-family, Fedora-family, and portable AppImage use; Arch users can use the AppImage or package from source.

## Windows

Download the signed Windows installer from the project's GitHub release. The installer places the application and `baudbound` command in a managed installation location, registers uninstall metadata, and provides the files required by the updater. WebView2 is required and is normally present on supported Windows systems.

## Linux

Choose the release artifact appropriate for the machine:

- `.deb` for Debian and Ubuntu families.
- `.rpm` for Fedora, RHEL, and compatible families.
- `.AppImage` for a portable distribution-independent desktop launch.

Install native packages with the distribution package manager so dependencies and uninstall behavior remain managed. Make an AppImage executable before launching it.

## First launch

The runner creates its default configuration automatically. No initialization command is required. Launching the desktop app opens the Dashboard; the CLI can verify installation with:

```text
baudbound --version
baudbound doctor
baudbound config path
```

The first release supports Windows and Linux only. macOS packages and runtime targets are not produced.

Continue with the [quick start](quick-start.md) and [updates](updates.md).
