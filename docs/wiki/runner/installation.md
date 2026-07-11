---
title: Runner Installation
description: Install BaudBound on supported Windows and Linux systems.
tags: [runner, installation]
---
# Runner Installation

## Supported systems

BaudBound releases currently target 64-bit Windows and 64-bit Linux. Windows is distributed as an installer. Linux is distributed as an AppImage built on Ubuntu 22.04.

## Windows

Download and run the signed NSIS installer from the project's GitHub release. WebView2 is required and is normally present on supported Windows systems.

## Linux

Download the `.AppImage`, make it executable, and launch it:

```text
chmod +x BaudBound_*.AppImage
./BaudBound_*.AppImage
```

BaudBound does not currently publish `.deb` or `.rpm` packages. AppImage compatibility still depends on the host kernel, architecture, graphics session, and required system integration; test it on the intended distribution before deployment.

## First launch

The runner creates its default configuration automatically. No initialization command is required. Launching the desktop app opens the Dashboard; the CLI can verify installation with:

```text
baudbound --version
baudbound doctor
baudbound config path
```

Continue with the [quick start](quick-start.md) and [updates](updates.md).
