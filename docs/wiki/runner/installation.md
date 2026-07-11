---
title: Installation and Updates
description: Install and update BaudBound on supported Windows and Linux systems.
tags: [runner, installation]
---
# Installation and Updates

## Supported systems

BaudBound releases currently target 64-bit Windows and 64-bit Linux. Windows is distributed as an installer. Linux is distributed as an AppImage built on Ubuntu 22.04.

## Platform installation {.tabset}

### Windows

Download and run the signed NSIS installer from the project's GitHub release. WebView2 is required and is normally present on supported Windows systems.

### Linux

Download the `.AppImage`, make it executable, and launch it:

```text
chmod +x BaudBound_*.AppImage
./BaudBound_*.AppImage
```

BaudBound does not currently publish `.deb` or `.rpm` packages. AppImage compatibility still depends on the host kernel, architecture, graphics session, and required system integration; test it on the intended distribution before deployment.

AppImages are portable executables and are not installed through the system package database. For a persistent headless installation with distro-specific prerequisites, follow [Linux Background Service](../self-hosting/linux-background-service.md).
{.is-info}

## First launch

The runner creates its default configuration automatically. No initialization command is required. Launching the desktop app opens the Dashboard; the CLI can verify installation with:

```text
baudbound --version
baudbound doctor
baudbound config path
```

## Updates

Desktop builds check for a signed update at startup. When a newer version is available, the update dialog shows its release information and download progress. After verification, choose Restart to install and launch it.

Headless operators and Linux users whose installation cannot be replaced automatically should download the new artifact from the project's GitHub releases. Never bypass signature verification.

Continue with [Script Management](script-management.md).
