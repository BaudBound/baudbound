---
title: Target Runtimes and Platform Support
description: Choose a Windows or Linux desktop/headless target and understand node compatibility enforcement.
tags: [editor, runner, compatibility, platforms]
---
# Target Runtimes and Platform Support

A target runtime states where a package is intended to run. It combines operating-system specificity with whether an interactive desktop session is required. Choose it before building the graph so the editor can hide or reject incompatible nodes.

## Choose a target

1. Will the runner operate without a signed-in graphical desktop session?
   - Choose a **Headless** target.
   - Do not use hotkeys, notifications, dialogs, sound playback, application opening, clipboard, input control, pixel, or window nodes.
2. Does the workflow require a desktop session?
   - Choose a **Desktop** target.
3. Does it depend on Windows-only window or pixel APIs, or window-title process matching?
   - Choose **Windows Desktop**.
4. Is it deliberately portable across Windows and Linux within the same session family?
   - Choose the corresponding **Generic** target.
5. Does it depend on OS-specific paths, executables, serial names, or behavior even though all nodes are generic?
   - Prefer the matching Windows or Linux target so operators are not misled.

## Runtime families {.tabset}

### Generic

| Target | Intended use |
| --- | --- |
| **Generic Headless** | Portable service workflow using non-desktop nodes |
| **Generic Desktop** | Portable interactive workflow using desktop features implemented on both supported systems |

Generic means the package contract is not intentionally bound to one operating system. It does not translate paths, executable names, shell syntax, device protocols, or external dependencies.

### Windows

| Target | Intended use |
| --- | --- |
| **Windows Headless** | Windows service or non-interactive automation without desktop APIs |
| **Windows Desktop** | Signed-in Windows session and Windows-native desktop behavior |

Windows Desktop is required for Get Active Window, Window Focus, Get Pixel Color, and process matching by window title.

### Linux

| Target | Intended use |
| --- | --- |
| **Linux Headless** | Linux service automation without a graphical session |
| **Linux Desktop** | Linux graphical session using currently implemented cross-platform desktop adapters |

Linux Desktop does not imply that every X11 or Wayland window-management feature exists. Active-window lookup, focus, pixel reading, and window-title process matching are currently rejected rather than emulated.

## Desktop-only nodes

These nodes require a Desktop target:

- Hotkey trigger;
- Open Application;
- Clipboard;
- Keyboard and Type Text;
- Show Notification;
- Mouse Click and Move Mouse; and
- Play Sound.

The editor checks `desktopOnly` from node definitions, and the runner independently applies its corresponding compatibility table.

## Windows Desktop-only behavior

These nodes support only Windows Desktop:

- MessageBox;
- Get Active Window;
- Window Focus; and
- Get Pixel Color.

Process Status, Kill Process, and App / Process Started are otherwise portable, but selecting **Window title** match mode makes that configured node Windows Desktop-only.

## Definition rules

A node definition can constrain support in two ways:

- `desktopOnly: true` allows desktop targets and rejects headless targets;
- `supportedTargetRuntimes` lists an explicit narrower set.

When `supportedTargetRuntimes` is omitted, the node is available to all targets unless another rule such as `desktopOnly` or configuration-specific validation narrows it. Omission does not promise that arbitrary machine paths or external dependencies are portable.

## Enforcement stages

1. **Palette:** incompatible nodes are not offered for the selected target.
2. **Editor verification:** existing incompatible nodes produce blocking errors after a target change or package import.
3. **Package declaration:** export records the selected target with calculated capabilities.
4. **Runner package validation:** Rust recalculates node and option compatibility.
5. **Runner host validation:** the package target must be among the current runner's supported targets.
6. **Native adapter:** a required adapter must still be available in the actual desktop session.

A manually edited package cannot bypass these stages.

## Runner target configuration

By default, a runner supports Generic plus its host operating-system targets for the available headless/desktop environment. `runner.target_runtimes` can restrict that set further for a dedicated installation.

For example, a Linux service can explicitly allow only:

```toml
[runner]
target_runtimes = ["Generic Headless", "Linux Headless"]
```

Configuration cannot grant a target or action that the built runner does not implement.

## Examples

| Workflow | Suitable target |
| --- | --- |
| Scheduled HTTP request and log on either OS | Generic Headless |
| Linux file watcher using `/srv/inbox` | Linux Headless |
| Windows process automation without desktop input | Windows Headless |
| Cross-platform notification workflow | Generic Desktop |
| Focus a Windows application and type text | Windows Desktop |
| Serial listener with Linux `/dev/ttyUSB0` mapping | Linux Headless or Linux Desktop, depending on other nodes |

For per-node configuration and support, use [Node Reference](node-reference.md).
