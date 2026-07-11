---
title: Target Runtimes
description: Choose Windows, Linux, desktop, or headless compatibility for a BaudBound project.
tags: [editor, compatibility]
---
# Target Runtimes

Every project selects the environment it is designed to run in. Both the editor and runner enforce that selection.

| Runtime | Intended environment |
| --- | --- |
| Generic Headless | Non-interactive Windows or Linux service behavior |
| Windows Headless | Windows service or terminal without desktop interaction |
| Linux Headless | Linux daemon or terminal without desktop interaction |
| Generic Desktop | Cross-platform interactive Windows or Linux desktop |
| Windows Desktop | Interactive Windows session and Windows-native actions |
| Linux Desktop | Interactive Linux desktop and supported Linux-native actions |

Desktop targets may use notifications, message boxes, clipboard, keyboard, mouse, and other supported interactive actions. Headless targets reject nodes that require an interactive session. Platform-specific definitions further narrow support; for example, active-window, window-focus, and pixel-color operations are currently Windows Desktop features.

Choose the narrowest runtime that matches deployment. A package targeting Windows Desktop must not silently run in a Linux or headless process.
