---
title: Node Reference
description: Catalog of triggers, control-flow nodes, and actions available in the BaudBound editor.
tags: [editor, nodes, reference]
---
# Node Reference

The editor currently provides the following executable nodes. Exact configuration is validated by the generated [node schemas](../package-format/contracts.md).

## Triggers

| Node | Purpose |
| --- | --- |
| Manual | Starts a run on demand from the runner or simulation panel. |
| Schedule | Starts on a configured schedule while the service is running. |
| File Watch | Starts when a watched filesystem path changes. |
| Webhook | Accepts an HTTP request and optionally waits for a Webhook Response node. |
| WebSocket | Starts from an inbound WebSocket message. |
| Hotkey | Starts from a registered key combination in a desktop session. |
| Serial Input | Reads from the runner serial device identified by `deviceId`. |
| Startup | Starts when the runner service loads the script. |
| Process Started | Starts when a matching process appears. |

## Control flow

| Node | Purpose |
| --- | --- |
| If/Else | Routes execution by one or more conditions, with optional inversion. |
| Switch | Routes a value to matching cases or a default branch. |
| Loop | Repeats a body for a configured iteration count. |
| While | Repeats a body while its condition remains true. |
| For Each | Iterates over a list and exposes the current item and index. |

## Data, network, and timing

| Node | Purpose |
| --- | --- |
| Variable Operation | Creates, updates, appends, or removes variable data. |
| Calculate | Evaluates supported numeric calculations. |
| Format Text | Produces formatted text from values and templates. |
| Log | Writes a structured runtime message. |
| Delay | Pauses the current execution path. |
| HTTP Request | Sends an outbound HTTP request and returns response data. |
| Webhook Response | Completes a waiting inbound webhook request. |
| WebSocket Write | Sends data to the connection associated with a WebSocket-triggered run. |

## Files and processes

| Node | Purpose |
| --- | --- |
| Read File / Write File | Reads or writes filesystem content. |
| Download File | Downloads remote content to a file. |
| Delete / Copy / Move File | Performs the named filesystem operation. |
| Run Process | Starts a process using native process APIs. |
| Process Status | Checks whether a process is running. |
| Kill Process | Terminates a matching process. |
| Open Application | Opens an application through supported native platform APIs. |
| Run Sub-script | Executes an installed BaudBound script as a child run. |

## Desktop and devices

| Node | Purpose |
| --- | --- |
| Notification / Message Box | Displays native desktop feedback. |
| Get Pixel Color | Reads a screen pixel on supported Windows desktops. |
| Get Active Window / Focus Window | Queries or focuses supported Windows windows. |
| Play Sound / Beep | Plays supported audio or a system beep. |
| Serial Write | Writes bytes or text to a configured runner serial device. |
| Keyboard / Type Text | Sends supported native keyboard input. |
| Mouse Click / Mouse Move | Sends supported native pointer input. |
| Clipboard | Reads or writes the native clipboard. |
| Shell | Executes a configured shell command and carries elevated risk. |

Nodes may request capabilities and platform support narrower than their category suggests. Always verify against the selected [target runtime](target-runtimes.md) and review the package in the runner before approval.
