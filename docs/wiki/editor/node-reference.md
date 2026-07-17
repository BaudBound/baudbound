---
title: Node Reference
description: Configuration, flow, outputs, access, compatibility, failures, and simulation behavior for every BaudBound node.
tags: [editor, nodes, reference]
---
# Node Reference

This reference covers every executable node currently registered by the editor. `action_type` is the stable package identifier. Configuration marked variable-aware accepts `{{variable}}` references. Exact serialized fields are enforced by the linked [per-node schemas](../package-format/index.md).

## Common node behavior

- Every node may have an optional **Custom name** for editor readability.
- Triggers begin runs and expose one `out` execution handle.
- Ordinary actions continue through `out`.
- Fallible actions expose `success` and `failed`. The failed route includes `error.message`, `error.code`, `error.type`, `error.retryable`, and `error.details`.
- Runtime output references use the real node ID, for example `{{n-example.status_code}}`.
- When one output connects to multiple destinations, the numbered connections execute sequentially in their explicit execution order. Canvas position does not select that order.
- Simulation descriptions are controlled tests. Native runner paths, permissions, devices, processes, and desktop state still require target-machine testing.
- Platform support defaults to all targets, then narrows through desktop-only, explicit target, and configuration-specific rules.

Risk and permission meanings are defined in [Approvals, Capabilities, and Risk](../security/approvals-capabilities.md).

## Triggers

### Manual

- **Action type:** `trigger.manual`; capability `trigger.manual`; low risk.
- **Configuration:** none. Only one Manual trigger is allowed per project.
- **Output:** no trigger-specific data; continues through `out`.
- **Use:** start a script on demand from Scripts or `baudbound script run SCRIPT`.
- **Simulation:** trigger card starts immediately with optional test payload.

### Schedule

- **Action type:** `trigger.schedule`; capability `trigger.schedule`; low risk.
- **Configuration:** **Every** positive number, default `5`; **Unit** `milliseconds`, `seconds`, `minutes`, `hours`, or `days`, default `minutes`. The resulting interval must be at least one millisecond.
- **Output:** runner payload includes interval and due-time information; graph continues through `out`.
- **Use:** recurring work while a background service is active.
- **Runtime:** unchanged registrations preserve due timing across reload; missed intervals are counted without dispatching every missed occurrence. Millisecond schedules use operating-system timers and are not hard real-time guarantees.
- **Simulation:** fires automatically while simulation remains active.

### File Watch

- **Action type:** `trigger.file_watch`; capability `trigger.file_watch`; low risk.
- **Configuration:** static **Path**; optional **Include subdirectories** for directory targets. Runtime templates are rejected.
- **Outputs:** `path` and normalized `event` (`created`, `modified`, `deleted`, or `renamed`).
- **Use:** react to one file or a directory tree.
- **Runtime:** target must exist and be accessible when listener registration starts. OS save behavior may emit multiple events.
- **Simulation:** supplied path/event become outputs; no filesystem watcher is opened.

### Webhook

- **Action type:** `trigger.webhook`; capability `trigger.webhook`; permission `webhook_public_bind`; high risk.
- **Configuration:** HTTP **Method**; required **Hook name**; optional wait switch, positive response timeout, fallback status `100-599`, content type, and body.
- **Outputs:** `method`, `path`, `headers`, `query`, raw `body`, parsed `json`, and `response` state.
- **Use:** receive HTTP at `/events/HOOK_NAME` while webhooks are enabled.
- **Flow rule:** waiting mode requires a reachable Webhook Response node. Timeout uses the configured fallback.
- **Simulation:** creates a request and response state from supplied payload; it does not open a network port.

### WebSocket

- **Action type:** `trigger.websocket`; capability `trigger.websocket`; permission `websocket_public_bind`; high risk.
- **Configuration:** required **Path** beginning with `/`, default `/events/messages`.
- **Outputs:** message text, path, `connection_id`, headers, query, and remote address.
- **Use:** begin one run per inbound text message on a matched connection.
- **Runtime:** requires enabled WebSocket listener and connection capacity. Use WebSocket Write with this run's connection ID.
- **Simulation:** supplies a synthetic connection when one is not entered.

### Hotkey

- **Action type:** `trigger.hotkey`; capability `trigger.hotkey`; medium risk; Windows Desktop only.
- **Configuration:** captured **Key** expression, default `Ctrl+Alt+B`. A single key such as `G`, `F1`, or `MediaPlayPause` is valid. Any distinct supported keys can form a chord, including `K+L`, `F1+T`, and `Ctrl+Shift+B`. See [Supported Windows node keys](#supported-windows-node-keys) for the exact names.
- **Output:** canonical `key` expression and timestamp.
- **Use:** start a script when the complete physical key chord is held while the Windows desktop background runner is active.
- **Matching:** the held keys must match the configured chord exactly. The run starts when the final required key is pressed, regardless of the order in which the keys were pressed. Holding the chord does not repeatedly start runs.
- **Platform:** Windows Desktop only. Firmware-only `Fn` keys and Windows secure-attention input such as `Ctrl+Alt+Delete` are not available.
- **Simulation:** uses the supplied or configured expression; it does not register a global OS hook.

### Serial Input

- **Action type:** `trigger.serial_input`; capability `trigger.serial_input`; permission `serial_input`; high risk.
- **Configuration:** logical **Device id**, default `serial-device`; lowercase letters, numbers, `_`, and `-` only.
- **Outputs:** `device_id`, received `data`, byte count, and runner `timestamp`.
- **Use:** start when a runner-mapped serial device emits data.
- **Graph rule:** the same device ID cannot be used by two Serial Input triggers in one project.
- **Simulation:** supplied text produces output and UTF-8 byte length without opening a port.

### Startup

- **Action type:** `trigger.startup`; capability `trigger.startup`; permission `startup_trigger`; high risk.
- **Configuration:** none.
- **Output:** startup reason and service context where available.
- **Use:** run once when an eligible script is loaded by a newly started service session.
- **Simulation:** reports a synthetic `runner_startup` reason.

### App / Process Started

- **Action type:** `trigger.process_started`; capability `trigger.process_started`; medium risk.
- **Configuration:** **Match by** process name, executable path, or window title; required **Target**.
- **Outputs:** process name, process ID, executable path, and window title where available.
- **Platform:** window-title matching requires Windows Desktop; other modes support compatible Windows/Linux targets.
- **Runtime:** polling dispatches when a process first appears, not continuously while the same process remains present.
- **Simulation:** uses supplied process facts or stable sample values.

## Control Flow

### If / Else

- **Action type:** `control.if`; capability `runtime.if`; low risk.
- **Configuration:** one or more condition rows with value, operator, optional inversion, target, and AND/OR combinator.
- **Flow:** named `true` and `false` outputs.
- **Operators:** equality, ordering, contains, prefix/suffix, regex, empty, and null checks.
- **Simulation/runtime:** values are resolved with their types before comparison. Inversion applies to one row before combinators.
- **Example:** `{{status_code}} >= 400` routes errors to `true`.

### Color Match

- **Action type:** `control.color_match`. It uses capability `runtime.color_match`, has low risk, and does not request a permission.
- **Configuration:** **Actual color**, **Expected color**, **Comparison mode**, and a variable-aware **Tolerance** from `0` through `100` percent.
- **Accepted colors:** canonical hex such as `#2F80ED`, RGB text such as `rgb(47, 128, 237)`, or a typed RGB object with exactly `r`, `g`, and `b` integer channels from `0` through `255`.
- **Flow:** `match` runs when the measured difference is less than or equal to the tolerance. `no match` runs for any valid pair outside the tolerance. Exactly one branch runs.
- **Per channel mode:** compares the largest red, green, or blue channel difference against the percentage tolerance. This is useful when no individual channel may drift too far.
- **Total RGB distance mode:** compares the normalized three-dimensional distance between the colors. This allows channel differences to contribute to one overall similarity value.
- **Outputs:** `matches`, `difference_percent`, `red_difference`, `green_difference`, and `blue_difference` remain available to later nodes through the Color Match node ID.
- **Validation:** `0` percent requires exact equality. `100` percent accepts every pair of valid RGB colors. Invalid or dynamically resolved malformed colors stop the node with an execution error instead of following `no match`.
- **Get Pixel Color example:** set **Actual color** to `{{n-pixel.rgb}}`, set **Expected color** to `#2F80ED`, choose a comparison mode, and enter the acceptable tolerance. The RGB object is passed directly without converting it to text.
- **Other examples:** compare two literals such as `#101820` and `rgb(16, 24, 32)`, or compare RGB object variables from any source. Color Match is available on every target runtime because it only compares data and does not read the screen.

### Switch

- **Action type:** `control.switch`; capability `runtime.switch`; low risk.
- **Configuration:** variable-aware **Value** and ordered case rows with stable IDs, labels, and expected values.
- **Flow:** one named output per case plus default behavior when no case matches.
- **Runtime:** first typed-equal case wins.
- **Example:** route `{{event_type}}` to `created`, `updated`, or default.

### Loop

- **Action type:** `control.loop`; capability `runtime.loop`; low risk.
- **Configuration:** variable-aware non-negative **Repeat count**.
- **Flow:** `loop` executes the body; `done` continues after all iterations.
- **Outputs/data:** loop index variables are available through runtime loop context.
- **Graph rule:** do not connect the body back to Loop; the runtime repeats it.
- **Simulation:** visibly repeats the body at selected speed.

### While

- **Action type:** `control.while`; capability `runtime.while`; low risk.
- **Configuration:** the same condition rows and inversion behavior as If/Else.
- **Flow:** `loop` executes while conditions pass; `done` follows the first false result.
- **Graph rule:** no explicit edge returns to While.
- **Safety:** ensure body state can make the condition false; runtime limits and cancellation remain important.

### For Each

- **Action type:** `control.for_each`; capability `runtime.for_each`; low risk.
- **Configuration:** variable-aware **Items** resolving to a list; **Item variable** and **Index variable** names.
- **Flow:** `loop` runs once per item; `done` follows completion.
- **Data:** configured variables hold the current item and zero-based index.
- **Failure:** non-list input fails control-flow validation/execution.
- **Example:** iterate `{{response.json.items}}` as `item` and `index`.

## Data and Output

### Variable Operation

- **Action type:** `runtime.set_variable`; capabilities `runtime.variables` and, for stored scopes, `runtime.persistent_storage`.
- **Configuration:** operation `set`, `increment`, `append_list`, `set_object_field`, or `clear`; name; scope `runtime`, `persistent`, or `global`; value type; operation-specific value and field path.
- **Access:** runtime scope is low, persistent is medium, global is high.
- **Data:** writes `{{name}}` and refreshes `$length`, `$count`, `$type`, and `$is_empty`.
- **Validation:** names use letters, numbers, and underscores, cannot start with a number, and cannot use reserved prefixes.
- **Simulation:** updates current simulation state; runner persistence must be tested separately.

### Calculate

- **Action type:** `action.calculate`; capability `action.calculate`; permission `calculate`; low risk; fallible.
- **Configuration:** variable-aware numeric **Expression**.
- **Output:** `result` number on success; structured error on failure.
- **Use:** arithmetic supported by the runtime expression evaluator, not arbitrary code.
- **Simulation:** evaluates with current values using the same supported expression rules.

### Text Transform

- **Action type:** `action.text.format`; capability `action.text`; permission `text_transform`; low risk.
- **Configuration:** operation-specific fields for template, input, search, replacement, delimiter, items, start, length, padding, or target length.
- **Operations:** template, trim, uppercase, lowercase, sentence case, capitalize words, literal/regex replace, split, join, substring, padding, URL/Base64 encode/decode, and JSON escape/unescape.
- **Output:** transformed result data available from the node output.
- **Failure:** invalid regex, encoding, indexes, or input shape produces validation/runtime error as applicable.

### Log

- **Action type:** `action.log`; capability `action.log`; permission `log`; low risk.
- **Configuration:** level `info`, `warn`, `error`, or `debug`; variable-aware **Message**.
- **Output:** no action-specific data; continues through `out`.
- **Runtime:** stores the resolved message with node and run identity.
- **Security:** do not intentionally log secrets or untrusted payloads without review.

### Delay

- **Action type:** `action.delay`; capability `action.delay`; permission `delay`; low risk.
- **Configuration:** variable-aware positive **Amount** and unit milliseconds, seconds, minutes, hours, or days. The resolved duration must be at least one millisecond.
- **Output:** none; continues after the cancellable wait.
- **Simulation:** validates the resolved duration and records the simulated delay without blocking the UI thread.

### Beep

- **Action type:** `action.beep`; capability `action.sound`; permission `beep`; low risk; Desktop only; fallible.
- **Configuration:** variable-aware positive frequency Hz, default `800`; duration ms, default `200`.
- **Flow/data:** success or failed with structured error.
- **Runtime:** plays a generated tone through the default desktop audio output.
- **Simulation:** Web Audio sine tone clamped to safe editor bounds; browsers may block audio before interaction.

### Show Notification

- **Action type:** `action.notification`; capability `action.notification`; permission `show_notification`; medium risk; Desktop only; fallible.
- **Configuration:** variable-aware **Title** and **Message**.
- **Output:** success/failure state.
- **Simulation:** editor toast rather than native notification-center behavior.

### MessageBox

- **Action type:** `action.message_box`; capability `action.message_box`; permission `show_message_box`; medium risk; Windows Desktop only; fallible.
- **Configuration:** type info/warning/error; title; message; buttons OK, OK/Cancel, Yes/No, or Yes/No/Cancel.
- **Output:** selected button plus success/failure data.
- **Simulation:** modal inside the editor; Stop aborts a waiting selection.

### Play Sound

- **Action type:** `action.sound.play`; capability `action.sound`; permission `play_sound`; medium risk; Desktop only; fallible.
- **Configuration:** source package asset or file path and corresponding selected path.
- **Output:** played source/path information and failure data.
- **Simulation:** plays package audio in the browser; a runner filesystem path cannot be tested there.

## Network and Serial

### HTTP Request

- **Action type:** `action.http`; capability `action.http`; permission `http_request`; medium risk; fallible.
- **Configuration:** method; variable-aware URL, headers, body, timeout `1-300` seconds, and user agent.
- **Outputs:** status code/text, headers, body, optional parsed `json`, duration, or structured network error.
- **Runtime:** native HTTP client behavior may differ from browser redirects, CORS, forbidden headers, cookies, and TLS stores.
- **Simulation:** sends a real browser `fetch`; use a safe test endpoint.

### Webhook Response

- **Action type:** `action.webhook_response`; capability `action.webhook_response`; permission `webhook_response`; low risk; fallible.
- **Configuration:** variable-aware status `100-599`, content type, headers, and body.
- **Outputs:** `sent`, status, content type, headers, body, owning `trigger_id`, or error.
- **Graph rule:** must be reachable from a Webhook trigger with waiting enabled.
- **Runtime:** exactly one response owns one pending request.

### WebSocket Write

- **Action type:** `action.websocket.write`; capability `action.websocket`; permission `websocket_write`; medium risk; fallible.
- **Configuration:** variable-aware **Connection id** and **Message**.
- **Outputs:** send result/byte information or connection error.
- **Use:** reply to the connection ID emitted by the WebSocket trigger for the current run.
- **Failure:** unknown, stale, or disconnected IDs are rejected.

### Serial Write

- **Action type:** `action.serial.write`; capability `action.serial`; permission `serial_write`; medium risk; fallible.
- **Configuration:** logical device ID selected from Serial Input triggers, variable-aware data, and line ending none/LF/CRLF.
- **Output:** write result or structured serial error.
- **Graph rule:** requires a Serial Input trigger using the same logical ID.
- **Simulation:** reports intended bytes and line ending without opening hardware.

## Files

### Read File

- **Action type:** `action.file.read`; capability `action.file`; permission `file_read`; medium risk; fallible.
- **Configuration:** variable-aware path and UTF-8 encoding.
- **Outputs:** content, byte count, resolved path, or error.
- **Access:** absolute, sensitive, or runtime-selected paths require the dangerous `read_sensitive_file` permission instead of `file_read`.
- **Simulation:** sample output only; runner account permissions and file existence remain untested.

### Write File

- **Action type:** `action.file.write`; capability `action.file`; permission `file_write_limited`; high risk; fallible.
- **Configuration:** variable-aware path/content and mode overwrite or append.
- **Outputs:** resolved path, bytes written, mode, or error.
- **Access:** absolute, sensitive, or runtime-selected paths require the dangerous `write_any_file` permission instead of `file_write_limited`.
- **Review:** paths can be influenced by variables; confirm they cannot escape intended storage.

### Download File

- **Action type:** `action.file.download`; capabilities `action.file` and network behavior; permission `download_file`; medium risk; fallible.
- **Configuration:** variable-aware URL and destination path; overwrite switch.
- **Outputs:** destination, transferred size/status information, or error.
- **Access:** an absolute, sensitive, or runtime-selected destination also requires `write_any_file`.
- **Review:** validate both remote source and local overwrite consequences.

### Delete File

- **Action type:** `action.file.delete`; capability `action.file`; permission `delete_file`; dangerous; fallible.
- **Configuration:** variable-aware path.
- **Output:** deleted path or error.
- **Warning:** deletion is not a recycle-bin operation. Restrict input and test on disposable data.

### Copy File

- **Action type:** `action.file.copy`; capability `action.file`; permission `file_copy`; medium risk; fallible.
- **Configuration:** variable-aware source and destination; overwrite switch.
- **Outputs:** resolved paths, copy result, or error.
- **Access:** a broad source requires `read_sensitive_file`; a broad destination requires `write_any_file`.

### Move File

- **Action type:** `action.file.move`; capability `action.file`; permission `file_move`; medium risk; fallible.
- **Configuration:** variable-aware source and destination; overwrite switch.
- **Outputs:** resolved paths, move result, or error.
- **Access:** a broad source requires `read_sensitive_file`; a broad destination requires `write_any_file`.
- **Review:** a successful move removes the original path.

## Processes and Windows

### Run Process

- **Action type:** `action.process.run`; capability `action.process`; permission `run_process`; high risk; fallible.
- **Configuration:** variable-aware executable, arguments, optional working directory, and optional timeout from `1` to `86400` seconds, default `300`.
- **Outputs:** process ID, exit/status information where applicable, or error.
- **Runtime:** uses native process APIs, not shell parsing. Arguments must match the target executable's contract.

### Process Status

- **Action type:** `action.process.status`; capability `action.process`; permission `process_query`; medium risk; fallible.
- **Configuration:** match by process name, executable path, or window title; variable-aware target.
- **Outputs:** running flag, matching process information, or error.
- **Platform:** window-title mode requires Windows Desktop.

### Kill Process

- **Action type:** `action.process.kill`; capability `action.process`; permission `process_kill`; high risk; fallible.
- **Configuration:** match by process name, executable path, window title, or PID; variable-aware target.
- **Outputs:** matched/terminated process information or error.
- **Platform:** window-title mode requires Windows Desktop.

### Open Application

- **Action type:** `action.application.open`; capability `action.window`; permission `open_application`; medium risk; Desktop only; fallible.
- **Configuration:** variable-aware application name/ID/shortcut/desktop entry and arguments.
- **Outputs:** resolved application ID and process ID when exposed.
- **Simulation:** returns sample IDs without opening an application.

### Get Active Window

- **Action type:** `action.window.active`; capability `action.window`; permission `window_query`; medium risk; Windows Desktop only; fallible.
- **Configuration:** none.
- **Outputs:** window title, process ID/name, executable path, and native handle where available.
- **Simulation:** sample window data; no native lookup.

### Window Focus

- **Action type:** `action.window.focus`; capability `action.window`; permission `window_focus`; high risk; Windows Desktop only; fallible.
- **Configuration:** match by process name, executable path, or window title; variable-aware target.
- **Outputs:** focused window/process details or error.
- **Review:** focus changes can redirect subsequent keyboard or mouse actions.

### Get Pixel Color

- **Action type:** `action.pixel.get`; capability `action.pixel`; permission `screen_pixel_read`; medium risk; Windows Desktop only; fallible.
- **Configuration:** variable-aware signed integer screen X and Y coordinates from `-2147483648` through `2147483647`.
- **Outputs:** coordinates, RGB channels, hex color, and error on failure.
- **Simulation:** deterministic sample color derived for testing, not a real screenshot read.
- **Runtime:** coordinates use the Windows virtual desktop. Negative values address monitors to the left of or above the primary display. Points in gaps between monitors are rejected.
- **Tools:** use the desktop runner coordinate picker to select a point and copy its X coordinate, Y coordinate, pair, or sampled color.

## Input Control

### Set Clipboard

- **Action type:** `action.clipboard.set`; capability `action.clipboard`; permission `write_clipboard`; medium risk; Desktop only; fallible.
- **Configuration:** variable-aware value to write.
- **Output:** written text, its UTF-8 byte length, or an error.
- **Review:** replaces the user's current clipboard and may expose data to other applications.

### Get Clipboard

- **Action type:** `action.clipboard.get`; capability `action.clipboard`; permission `read_clipboard`; medium risk; Desktop only; fallible.
- **Configuration:** none.
- **Output:** clipboard text or an error when text is unavailable.
- **Review:** clipboard text can contain passwords or other sensitive data. Avoid sending it to logs or external services unless that is intentional.
- **Platform:** requires a signed-in Windows or Linux desktop session with a working native clipboard provider.

### Keyboard

- **Action type:** `action.keyboard`; capability `action.keyboard`; permission `keyboard_control`; high risk; Windows Desktop only; fallible.
- **Configuration:** one or more distinct supported keys captured by the editor or added with the key-reference buttons. Separate chord members with `+`, for example `G`, `F1`, `K+L`, or `Ctrl+Shift+S`.
- **Supported keys:** uses the same [Supported Windows node keys](#supported-windows-node-keys) as the Hotkey trigger. Unsupported names are rejected instead of being guessed.
- **Platform:** Windows Desktop only.
- **Output:** sent key/status or error.
- **Review:** ensure the intended application has focus. Use Type Text for words and arbitrary text.

#### Supported Windows node keys

Hotkey and Keyboard nodes use one shared Windows key contract. The editor, exported package checks, global hotkey service, and native Keyboard action all validate the same names. The editor captures held keys and also provides a button for every canonical key name.

| Group | Supported canonical names |
| --- | --- |
| Modifiers | `Ctrl`, `Alt`, `Shift`, `Windows` |
| Letters and digits | `A` through `Z`, `0` through `9` |
| Function | `F1` through `F24` |
| Navigation and editing | `Escape`, `Enter`, `Space`, `Tab`, `Backspace`, `Delete`, `Insert`, `Home`, `End`, `PageUp`, `PageDown`, `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight` |
| System and lock | `CapsLock`, `NumLock`, `ScrollLock`, `PrintScreen`, `Pause`, `ContextMenu` |
| Punctuation | `Semicolon`, `Equal`, `Comma`, `Minus`, `Period`, `Slash`, `Backquote`, `BracketLeft`, `Backslash`, `BracketRight`, `Quote`, `IntlBackslash` |
| Numpad | `Numpad0` through `Numpad9`, `NumpadMultiply`, `NumpadAdd`, `NumpadSeparator`, `NumpadSubtract`, `NumpadDecimal`, `NumpadDivide` |
| Browser | `BrowserBack`, `BrowserForward`, `BrowserRefresh`, `BrowserStop`, `BrowserSearch`, `BrowserFavorites`, `BrowserHome` |
| Media and volume | `VolumeMute`, `VolumeDown`, `VolumeUp`, `MediaNext`, `MediaPrevious`, `MediaStop`, `MediaPlayPause` |
| Application launch | `LaunchMail`, `LaunchMedia`, `LaunchApp1`, `LaunchApp2` |

Firmware-managed keys such as `Fn` and Windows secure-attention input such as `Ctrl+Alt+Delete` cannot be captured or generated. Browser, media, and Windows-key combinations may also be reserved by the browser, Windows, or another application. Build those combinations with the key-reference buttons when the browser cannot capture them. For example, pressing `Ctrl+W` normally asks the browser to close the tab, so use the `Ctrl` and `W` buttons instead.

### Type Text

- **Action type:** `action.keyboard.type_text`; capability `action.keyboard`; permission `keyboard_control`; high risk; Windows Desktop only; fallible.
- **Configuration:** variable-aware text.
- **Platform:** Windows Desktop only.
- **Output:** typed length/status or error.
- **Security:** never type secrets into an unverified foreground target.

### Mouse Click

- **Action type:** `action.mouse`; capability `action.mouse`; permission `mouse_control`; high risk; Windows Desktop only; fallible.
- **Configuration:** left/right/middle/back/forward button and single/double click.
- **Output:** click details or error.
- **Platform:** Windows Desktop only.
- **Runtime:** acts at the current pointer position.

### Move Mouse

- **Action type:** `action.mouse.move`; capability `action.mouse`; permission `mouse_control`; high risk; Windows Desktop only; fallible.
- **Configuration:** variable-aware signed integer X/Y and relative switch.
- **Output:** final coordinates/movement details or error.
- **Platform:** Windows Desktop only.
- **Runtime:** absolute coordinates use the Windows virtual desktop and must belong to a connected monitor. Relative values remain signed offsets from the current pointer position.
- **Review:** use the runner Tools tab to inspect monitor ranges and display scaling or select an exact point with the coordinate picker.

## Scripts and System

### Sub-script

- **Action type:** `action.script.run`; capability `action.sub_script`; permission `sub_script_run`; high risk; fallible.
- **Configuration:** installed child script name or ID.
- **Outputs:** child run ID/status/report summary or error.
- **Runtime:** child must be independently installed, valid, current, approved, and manually runnable. Parent approval does not approve the child.

### Shell Command

- **Action type:** `action.shell`; capability supplied through process execution; permission `run_shell_command`; dangerous; fallible.
- **Configuration:** variable-aware command string interpreted by the target shell and optional timeout from `1` to `86400` seconds, default `300`.
- **Outputs:** exit code, stdout, stderr, or error.
- **Platform:** syntax is platform-specific even under a Generic target.
- **Warning:** prefer a native node or Run Process. Shell interpolation can turn data into arbitrary commands and has independent runner policy gates.

## Related references

- [Variables and Data](variables.md) for output syntax and types.
- [Target Runtimes](target-runtimes.md) for compatibility enforcement.
- [Background Service and Triggers](../runner/service-triggers.md) for listener operation.
- [Webhooks, WebSockets, and Network Access](../runner/network-listeners.md) for network exposure.
- [Configuration and Serial Devices](../runner/configuration.md) for logical hardware mapping.
