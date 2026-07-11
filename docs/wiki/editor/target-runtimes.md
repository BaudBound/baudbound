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

## Runtime families {.tabset}

### Generic

**Generic Headless** is for automation that can run as a non-interactive service on either supported operating system. **Generic Desktop** permits only interactive actions implemented natively on both Windows and Linux.

Choose a generic target when the same package must move between platforms without editing. The editor rejects nodes whose native implementation is limited to one operating system.

### Windows

**Windows Headless** allows Windows-specific non-interactive behavior in a service or terminal. **Windows Desktop** adds actions that require an interactive Windows session.

Active-window lookup, window focus, pixel-color access, and other explicitly Windows-native nodes require Windows Desktop. A headless Windows service cannot safely provide desktop input or window-session behavior.

### Linux

**Linux Headless** is intended for daemons, terminals, and server automation. **Linux Desktop** adds the cross-platform interactive actions that have native Linux implementations.

Linux desktop support depends on an interactive graphical session and the capabilities declared by each node. Nodes marked Windows-only remain unavailable even when the project selects Linux Desktop.

## Choosing a target

Desktop targets may use notifications, message boxes, clipboard, keyboard, mouse, and other supported interactive actions. Headless targets reject nodes that require an interactive session. Platform-specific node definitions can narrow support further.

Choose the narrowest runtime that matches deployment. A package targeting Windows Desktop must not silently run in a Linux or headless process.
