# baudbound-actions

Runner action implementations for headless and shared runner execution.

Current implementation:

- `HeadlessActionHandler`
- `DesktopActionHandler`
  - wraps a headless handler and routes desktop-only actions through a `DesktopActionAdapter`
  - lets the app provide native clipboard, notification, message box, audio, input, screen, and window adapters without changing runtime/core execution
- `action.beep`
  - emits a terminal bell and waits for configured duration
  - outputs `frequency_hz`, `duration_ms`
- `action.file.read`
  - UTF-8 file reads
  - outputs `path`, `content`, `bytes`
- `action.file.write`
  - overwrite and append modes
  - creates parent directories as needed
  - outputs `path`, `mode`, `bytes`
- `action.file.copy`
  - copies one file to another path
  - respects the `overwrite` option
  - outputs `source_path`, `destination_path`, `bytes`
- `action.file.move`
  - moves or renames a file
  - respects the `overwrite` option
  - outputs `source_path`, `destination_path`
- `action.file.delete`
  - deletes regular files only
  - outputs `path`
- `action.file.download`
  - downloads HTTP(S) content to a destination file
  - respects the `overwrite` option
  - outputs `path`, `url`, `bytes`
- `action.http`
  - sends HTTP requests with method, headers, user-agent, timeout, and optional body
  - returns HTTP error statuses as runtime data instead of transport failures
  - outputs `status_code`, `status_text`, `headers`, `body`, optional `json`, `duration_ms`
- `action.application.open`
  - starts a configured application or executable without waiting for it to finish
  - parses quoted arguments without invoking a shell
  - outputs `application_id`, `process_id`, `arguments`
- `action.process.run`
  - starts an executable and captures output
  - outputs `process_id`, `exit_code`, `success`, `stdout`, `stderr`
- `action.process.status`
  - checks process status by process name or executable path in the headless runner
  - outputs `running`, `state`, `process_id`, `process_name`, `executable_path`
- `action.process.kill`
  - terminates a process by PID, process name, or executable path in the headless runner
  - outputs `running`, `state`, `process_id`, `process_name`, `executable_path`, `killed`
- `action.pixel.get`
  - routed through the desktop action adapter because screen capture is desktop-specific
  - outputs `hex`, `rgb`, `rgba`, `red`, `green`, `blue`, `alpha`, `integer`
- `action.shell`
  - runs a platform shell command and captures output
  - outputs `process_id`, `exit_code`, `success`, `stdout`, `stderr`
- `action.serial.write`
  - writes to a logical serial device id resolved by runner TOML
  - supports none, LF, and CRLF line endings
  - optionally validates USB vendor/product identity from runner TOML
  - outputs `device_id`, `port`, `bytes`
- `action.text.format`
  - template
  - trim
  - uppercase / lowercase
  - replace / regex replace
  - split / join
  - substring
  - pad start / pad end
  - URL encode / decode
  - Base64 encode / decode
  - JSON escape / unescape
- `action.webhook_response`
  - prepares webhook response data for the trigger engine
  - outputs `sent`, `status_code`, `content_type`, `headers`, `body`, `trigger_id`
- `action.websocket.write`
  - sends a text message to an active WebSocket Trigger connection through the runner-provided sink
  - outputs `connection_id`, `message`, `bytes`
- Explicit desktop-only failures in `HeadlessActionHandler` and `UnavailableDesktopActionAdapter`
  - `action.clipboard`
  - `action.message_box`
  - `action.notification`
  - `action.pixel.get`
  - `action.sound.play`

Planned action families:

- Window-title process matching in the desktop runner
- Package asset resolution for desktop action adapters

