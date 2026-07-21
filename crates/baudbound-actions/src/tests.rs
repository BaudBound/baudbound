use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};

use baudbound_runtime::{RunIdentity, RuntimeActionHandler, RuntimeActionRequest, RuntimeContext};
use serde_json::{Map, Value, json};

use super::{
    DesktopActionAdapter, DesktopActionHandler, HeadlessActionHandler, SerialDeviceConfig,
    WebSocketMessageSink,
};

#[path = "tests/file_actions.rs"]
mod file_actions;
#[path = "tests/http_action.rs"]
mod http_action;
#[path = "tests/process_actions.rs"]
mod process_actions;
#[path = "tests/text_format.rs"]
mod text_format;

#[derive(Default)]
struct FakeWebSocketSink {
    sent: Mutex<Vec<(String, String)>>,
}

impl WebSocketMessageSink for FakeWebSocketSink {
    fn send_text(&self, connection_id: &str, message: &str) -> Result<usize, String> {
        self.sent
            .lock()
            .expect("fake sink lock should not be poisoned")
            .push((connection_id.to_owned(), message.to_owned()));
        Ok(message.len())
    }
}

#[derive(Default)]
struct FakeDesktopAdapter {
    called: Mutex<Vec<String>>,
    finished_runs: Mutex<Vec<RunIdentity>>,
}

impl DesktopActionAdapter for FakeDesktopAdapter {
    fn run_finished(&self, identity: &RunIdentity) {
        self.finished_runs
            .lock()
            .expect("fake adapter lock should not be poisoned")
            .push(identity.clone());
    }

    fn beep(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), json!("beep"))]),
        })
    }

    fn clipboard_set(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), json!("clipboard_set"))]),
        })
    }

    fn clipboard_get(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([
                ("handled".to_owned(), json!("clipboard_get")),
                ("text".to_owned(), json!("clipboard text")),
            ]),
        })
    }

    fn message_box(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), json!("message_box"))]),
        })
    }

    fn notification(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), json!("notification"))]),
        })
    }

    fn sound_play(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), json!("sound_play"))]),
        })
    }

    fn keyboard(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), json!("keyboard"))]),
        })
    }

    fn keyboard_type_text(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), json!("keyboard_type_text"))]),
        })
    }

    fn mouse_click(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), json!("mouse_click"))]),
        })
    }

    fn mouse_move(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), json!("mouse_move"))]),
        })
    }

    fn pixel_get(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), json!("pixel_get"))]),
        })
    }

    fn active_window(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), json!("active_window"))]),
        })
    }

    fn window_focus(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), json!("window_focus"))]),
        })
    }

    fn process_status_by_window_title(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([(
                "handled".to_owned(),
                json!("process_status_by_window_title"),
            )]),
        })
    }

    fn kill_process_by_window_title(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
        self.record(request);
        Ok(baudbound_runtime::RuntimeActionResult {
            output_data: Map::from_iter([(
                "handled".to_owned(),
                json!("kill_process_by_window_title"),
            )]),
        })
    }
}

impl FakeDesktopAdapter {
    fn record(&self, request: &RuntimeActionRequest) {
        self.called
            .lock()
            .expect("fake adapter lock should not be poisoned")
            .push(request.action_type.clone());
    }
}

#[test]
fn reads_utf8_file() {
    let directory = tempfile::tempdir().expect("tempdir should be created");
    let path = directory.path().join("input.txt");
    fs::write(&path, "hello").expect("file should be written");

    let result = execute(
        "action.file.read",
        json!({ "path": path.display().to_string(), "encoding": "utf-8" }),
    )
    .expect("file read should succeed");

    assert_eq!(result.output_data.get("content"), Some(&json!("hello")));
    assert_eq!(result.output_data.get("bytes"), Some(&json!(5)));
}

#[test]
fn writes_and_appends_file_content() {
    let directory = tempfile::tempdir().expect("tempdir should be created");
    let path = directory.path().join("nested").join("output.txt");

    let overwrite = execute(
        "action.file.write",
        json!({
            "mode": "overwrite",
            "path": path.display().to_string(),
            "content": "hello"
        }),
    )
    .expect("file write should succeed");
    assert_eq!(overwrite.output_data.get("bytes"), Some(&json!(5)));

    execute(
        "action.file.write",
        json!({
            "mode": "append",
            "path": path.display().to_string(),
            "content": " world"
        }),
    )
    .expect("file append should succeed");

    assert_eq!(
        fs::read_to_string(&path).expect("file should read"),
        "hello world"
    );
}

#[test]
fn copies_file_and_respects_overwrite_flag() {
    let directory = tempfile::tempdir().expect("tempdir should be created");
    let source = directory.path().join("input.txt");
    let destination = directory.path().join("out").join("input.txt");
    fs::write(&source, "one").expect("source should write");
    fs::write(directory.path().join("existing.txt"), "old").expect("destination should write");

    let result = execute(
        "action.file.copy",
        json!({
            "sourcePath": source.display().to_string(),
            "destinationPath": destination.display().to_string(),
            "overwrite": "false"
        }),
    )
    .expect("file copy should succeed");
    assert_eq!(result.output_data.get("bytes"), Some(&json!(3)));
    assert_eq!(
        fs::read_to_string(&destination).expect("destination should read"),
        "one"
    );

    let error = execute(
        "action.file.copy",
        json!({
            "sourcePath": source.display().to_string(),
            "destinationPath": destination.display().to_string(),
            "overwrite": "false"
        }),
    )
    .expect_err("copy should reject existing destination");
    assert!(error.to_string().contains("overwrite is disabled"));
}

#[test]
fn moves_file_and_removes_source() {
    let directory = tempfile::tempdir().expect("tempdir should be created");
    let source = directory.path().join("input.txt");
    let destination = directory.path().join("archive").join("input.txt");
    fs::write(&source, "move me").expect("source should write");

    execute(
        "action.file.move",
        json!({
            "sourcePath": source.display().to_string(),
            "destinationPath": destination.display().to_string(),
            "overwrite": false
        }),
    )
    .expect("file move should succeed");

    assert!(!source.exists());
    assert_eq!(
        fs::read_to_string(&destination).expect("destination should read"),
        "move me"
    );
}

#[test]
fn deletes_regular_file_only() {
    let directory = tempfile::tempdir().expect("tempdir should be created");
    let file = directory.path().join("delete-me.txt");
    fs::write(&file, "delete me").expect("file should write");

    execute(
        "action.file.delete",
        json!({ "path": file.display().to_string() }),
    )
    .expect("file delete should succeed");
    assert!(!file.exists());

    let error = execute(
        "action.file.delete",
        json!({ "path": directory.path().display().to_string() }),
    )
    .expect_err("directory delete should fail");
    assert!(error.to_string().contains("not a regular file"));
}

#[test]
fn beep_requires_desktop_audio_adapter() {
    let error = execute(
        "action.beep",
        json!({
            "frequencyHz": "880",
            "durationMs": "200"
        }),
    )
    .expect_err("headless beep should fail");

    assert!(error.to_string().contains("desktop runner action adapter"));
}

#[test]
fn prepares_webhook_response_data() {
    let result = execute_with_trigger_payload(
        "action.webhook_response",
        json!({
            "statusCode": "202",
            "contentType": "application/json",
            "headers": [{ "id": "h-1", "name": "Cache-Control", "value": "no-store" }],
            "body": "{\"queued\":true}"
        }),
        json!({ "trigger_id": "n-webhook" }),
    )
    .expect("webhook response should prepare");

    assert_eq!(result.output_data.get("sent"), Some(&json!(true)));
    assert_eq!(result.output_data.get("status_code"), Some(&json!(202)));
    assert_eq!(
        result.output_data.get("content_type"),
        Some(&json!("application/json"))
    );
    assert_eq!(
        result.output_data.get("trigger_id"),
        Some(&json!("n-webhook"))
    );
    assert_eq!(
        result
            .output_data
            .get("headers")
            .and_then(|headers| headers.get("cache-control")),
        Some(&json!("no-store"))
    );
}

#[test]
fn indexes_serial_devices_from_runner_config() {
    let handler = HeadlessActionHandler::from_serial_devices([serial_device_config()]);

    let device = handler
        .serial_connections
        .config("main-device")
        .expect("serial device should be indexed");

    assert_eq!(device.port, "COM3");
    assert_eq!(device.baud_rate, 9_600);
}

#[test]
fn serial_write_rejects_unknown_device() {
    let error = execute(
        "action.serial.write",
        json!({
            "deviceId": "missing-device",
            "data": "ping",
            "lineEnding": "none"
        }),
    )
    .expect_err("unknown serial device should fail");

    assert!(error.to_string().contains("unknown serial device"));
}

#[test]
fn websocket_write_uses_configured_sink() {
    let sink = Arc::new(FakeWebSocketSink::default());
    let handler_sink: Arc<dyn WebSocketMessageSink> = sink.clone();
    let handler = HeadlessActionHandler::default().with_websocket_sink(handler_sink);
    let result = execute_with_handler(
        &handler,
        "action.websocket.write",
        json!({
            "connectionId": "conn-1",
            "message": "hello"
        }),
        Value::Null,
    )
    .expect("websocket write should use sink");

    assert_eq!(
        result.output_data.get("connection_id"),
        Some(&json!("conn-1"))
    );
    assert_eq!(result.output_data.get("bytes"), Some(&json!(5)));
    assert_eq!(
        sink.sent
            .lock()
            .expect("fake sink lock should not be poisoned")
            .as_slice(),
        &[("conn-1".to_owned(), "hello".to_owned())]
    );
}

fn serial_device_config() -> SerialDeviceConfig {
    SerialDeviceConfig {
        auto_reconnect: true,
        auto_rebind_port: false,
        baud_rate: 9_600,
        data_bits: 8,
        device_id: "main-device".to_owned(),
        dtr_on_open: "deasserted".to_owned(),
        flow_control: "none".to_owned(),
        manufacturer: None,
        max_message_bytes: 1_048_576,
        message_gap_ms: 100,
        open_stabilization_ms: 500,
        parity: "none".to_owned(),
        port: "COM3".to_owned(),
        product_id: None,
        product: None,
        read_mode: "idle_gap".to_owned(),
        serial_number: None,
        stop_bits: "1".to_owned(),
        validate_usb_identity: false,
        vendor_id: None,
    }
}

#[test]
fn desktop_only_actions_fail_explicitly_in_headless_handler() {
    let error = execute(
        "action.notification",
        json!({ "title": "BaudBound", "message": "hello" }),
    )
    .expect_err("notification should be desktop-only");

    assert!(error.to_string().contains("desktop runner action adapter"));
}

#[test]
fn desktop_action_handler_routes_desktop_actions_to_adapter() {
    let adapter = FakeDesktopAdapter::default();
    let handler = DesktopActionHandler::new(HeadlessActionHandler::default(), adapter);

    let result = execute_with_handler(
        &handler,
        "action.notification",
        json!({ "title": "BaudBound", "message": "hello" }),
        Value::Null,
    )
    .expect("desktop action should route to adapter");

    assert_eq!(
        result.output_data.get("handled"),
        Some(&json!("notification"))
    );
    assert_eq!(
        handler
            .adapter
            .called
            .lock()
            .expect("fake adapter lock should not be poisoned")
            .as_slice(),
        &["action.notification".to_owned()]
    );
}

#[test]
fn desktop_action_handler_forwards_run_cleanup_to_adapter() {
    let handler = DesktopActionHandler::new(
        HeadlessActionHandler::default(),
        FakeDesktopAdapter::default(),
    );
    let identity = RunIdentity {
        run_id: "run-cleanup".to_owned(),
        script_id: "script-cleanup".to_owned(),
        trigger_node_id: "trigger-cleanup".to_owned(),
    };

    handler.run_finished(&identity);

    assert_eq!(
        handler
            .adapter
            .finished_runs
            .lock()
            .expect("fake adapter lock should not be poisoned")
            .as_slice(),
        &[identity]
    );
}

#[test]
fn desktop_action_handler_routes_input_and_window_actions_to_adapter() {
    let adapter = FakeDesktopAdapter::default();
    let handler = DesktopActionHandler::new(HeadlessActionHandler::default(), adapter);

    for (action_type, expected) in [
        ("action.beep", "beep"),
        ("action.clipboard.get", "clipboard_get"),
        ("action.clipboard.set", "clipboard_set"),
        ("action.keyboard", "keyboard"),
        ("action.keyboard.type_text", "keyboard_type_text"),
        ("action.mouse", "mouse_click"),
        ("action.mouse.move", "mouse_move"),
        ("action.pixel.get", "pixel_get"),
        ("action.window.active", "active_window"),
        ("action.window.focus", "window_focus"),
    ] {
        let result = execute_with_handler(&handler, action_type, json!({}), Value::Null)
            .expect("desktop action should route to adapter");
        assert_eq!(result.output_data.get("handled"), Some(&json!(expected)));
        if action_type == "action.clipboard.get" {
            assert_eq!(
                result.output_data.get("text"),
                Some(&json!("clipboard text"))
            );
        }
    }

    assert_eq!(
        handler
            .adapter
            .called
            .lock()
            .expect("fake adapter lock should not be poisoned")
            .as_slice(),
        &[
            "action.beep".to_owned(),
            "action.clipboard.get".to_owned(),
            "action.clipboard.set".to_owned(),
            "action.keyboard".to_owned(),
            "action.keyboard.type_text".to_owned(),
            "action.mouse".to_owned(),
            "action.mouse.move".to_owned(),
            "action.pixel.get".to_owned(),
            "action.window.active".to_owned(),
            "action.window.focus".to_owned()
        ]
    );
}

#[test]
fn desktop_action_handler_routes_only_window_title_process_modes_to_adapter() {
    let adapter = FakeDesktopAdapter::default();
    let handler = DesktopActionHandler::new(HeadlessActionHandler::default(), adapter);

    for (action_type, expected) in [
        ("action.process.status", "process_status_by_window_title"),
        ("action.process.kill", "kill_process_by_window_title"),
    ] {
        let result = execute_with_handler(
            &handler,
            action_type,
            json!({"matchMode": "window_title", "target": "BaudBound"}),
            Value::Null,
        )
        .expect("window-title process action should route to the desktop adapter");
        assert_eq!(result.output_data.get("handled"), Some(&json!(expected)));
    }

    let current_pid = std::process::id();
    let result = execute_with_handler(
        &handler,
        "action.process.status",
        json!({"matchMode": "pid", "target": current_pid}),
        Value::Null,
    )
    .expect("PID process status should remain in the shared handler");
    assert_eq!(result.output_data.get("running"), Some(&json!(true)));

    assert_eq!(
        handler
            .adapter
            .called
            .lock()
            .expect("fake adapter lock should not be poisoned")
            .as_slice(),
        &[
            "action.process.status".to_owned(),
            "action.process.kill".to_owned(),
        ]
    );
}

#[test]
fn desktop_action_handler_delegates_headless_actions() {
    let handler = DesktopActionHandler::new(
        HeadlessActionHandler::default(),
        FakeDesktopAdapter::default(),
    );

    let result = execute_with_handler(
        &handler,
        "action.text.format",
        json!({
            "operation": "uppercase",
            "input": "baudbound"
        }),
        Value::Null,
    )
    .expect("headless action should delegate");

    assert_eq!(result.output_data.get("text"), Some(&json!("BAUDBOUND")));
    assert!(
        handler
            .adapter
            .called
            .lock()
            .expect("fake adapter lock should not be poisoned")
            .is_empty()
    );
}

#[test]
fn sends_http_request_and_parses_json_response() {
    let server = TestHttpServer::start(
        "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nX-Test: ok\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
    );

    let result = execute(
        "action.http",
        json!({
            "method": "POST",
            "url": server.url("/submit"),
            "headers": [{ "id": "h-1", "name": "X-Request", "value": "runner" }],
            "userAgent": "BaudBound-Test",
            "timeoutSeconds": "5",
            "body": "{\"name\":\"baudbound\"}"
        }),
    )
    .expect("HTTP request should succeed");

    assert_eq!(result.output_data.get("status_code"), Some(&json!(201)));
    assert_eq!(
        result.output_data.get("status_text"),
        Some(&json!("Created"))
    );
    assert_eq!(result.output_data.get("json"), Some(&json!({ "ok": true })));
    assert_eq!(
        result
            .output_data
            .get("headers")
            .and_then(|headers| headers.get("x-test")),
        Some(&json!("ok"))
    );
    assert!(
        result
            .output_data
            .get("duration_ms")
            .and_then(Value::as_u64)
            .is_some()
    );

    let request = server.join();
    assert!(request.contains("POST /submit HTTP/1.1"));
    assert!(request.contains("x-request: runner"));
    assert!(request.contains("user-agent: BaudBound-Test"));
    assert!(request.contains(r#"{"name":"baudbound"}"#));
}

#[test]
fn downloads_file_to_destination() {
    let server = TestHttpServer::start(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 9\r\nConnection: close\r\n\r\ndownload!",
    );
    let directory = tempfile::tempdir().expect("tempdir should be created");
    let destination = directory.path().join("downloads").join("file.txt");

    let result = execute(
        "action.file.download",
        json!({
            "url": server.url("/file.txt"),
            "destinationPath": destination.display().to_string(),
            "overwrite": "false",
            "timeoutSeconds": 5
        }),
    )
    .expect("download should succeed");

    assert_eq!(
        fs::read_to_string(&destination).expect("download should read"),
        "download!"
    );
    assert_eq!(result.output_data.get("bytes"), Some(&json!(9)));
    assert_eq!(
        result.output_data.get("path"),
        Some(&json!(destination.display().to_string()))
    );

    let request = server.join();
    assert!(request.contains("GET /file.txt HTTP/1.1"));
}

#[test]
fn runs_process_and_captures_output() {
    let (executable, arguments) = platform_echo_process("process-ok");
    let result = execute(
        "action.process.run",
        json!({
            "executable": executable,
            "arguments": arguments,
            "workingDirectory": ""
        }),
    )
    .expect("process should run");

    assert_eq!(result.output_data.get("exit_code"), Some(&json!(0)));
    assert_eq!(result.output_data.get("success"), Some(&json!(true)));
    assert!(
        result
            .output_data
            .get("stdout")
            .and_then(Value::as_str)
            .is_some_and(|stdout| stdout.contains("process-ok"))
    );
}

#[test]
fn opens_application_without_waiting_for_output() {
    let (application, arguments) = platform_echo_process("open-ok");
    let result = execute(
        "action.application.open",
        json!({
            "application": application,
            "arguments": arguments
        }),
    )
    .expect("application should open");

    assert_eq!(
        result.output_data.get("application_id"),
        Some(&json!(application))
    );
    assert!(
        result
            .output_data
            .get("process_id")
            .and_then(Value::as_u64)
            .is_some_and(|process_id| process_id > 0)
    );
    assert!(
        result
            .output_data
            .get("arguments")
            .is_some_and(Value::is_array)
    );
}

#[test]
fn rejects_open_application_with_invalid_arguments() {
    let error = execute(
        "action.application.open",
        json!({
            "application": "example-app",
            "arguments": "\"unterminated"
        }),
    )
    .expect_err("unterminated arguments should fail");

    assert!(
        error.to_string().contains("unterminated quoted string"),
        "{error}"
    );
}

#[test]
fn reads_current_process_status() {
    let current_executable = std::env::current_exe().expect("current exe should resolve");
    let process_name = current_executable
        .file_name()
        .and_then(|name| name.to_str())
        .expect("current exe should have utf-8 file name");

    let result = execute(
        "action.process.status",
        json!({
            "matchMode": "process_name",
            "target": process_name
        }),
    )
    .expect("process status should succeed");

    assert_eq!(result.output_data.get("running"), Some(&json!(true)));
    assert_eq!(result.output_data.get("state"), Some(&json!("running")));
    assert!(
        result
            .output_data
            .get("process_id")
            .and_then(Value::as_u64)
            .is_some_and(|process_id| process_id > 0)
    );
}

#[test]
fn kills_process_by_pid() {
    let mut child = spawn_long_running_process();
    let process_id = child.id();

    let result = execute(
        "action.process.kill",
        json!({
            "matchMode": "pid",
            "target": process_id.to_string()
        }),
    )
    .expect("process kill should succeed");

    assert_eq!(result.output_data.get("killed"), Some(&json!(true)));
    assert_eq!(
        result.output_data.get("process_id"),
        Some(&json!(process_id))
    );
    let _ = child.wait();
}

#[test]
fn runs_shell_command_and_captures_output() {
    let result = execute(
        "action.shell",
        json!({ "command": platform_echo_shell_command("shell-ok") }),
    )
    .expect("shell command should run");

    assert_eq!(result.output_data.get("exit_code"), Some(&json!(0)));
    assert_eq!(result.output_data.get("success"), Some(&json!(true)));
    assert!(
        result
            .output_data
            .get("stdout")
            .and_then(Value::as_str)
            .is_some_and(|stdout| stdout.contains("shell-ok"))
    );
}

#[test]
fn transforms_text_with_regex_replace() {
    let result = execute(
        "action.text.format",
        json!({
            "operation": "regex_replace",
            "input": "server-123",
            "search": "\\d+",
            "replacement": "ok"
        }),
    )
    .expect("text transform should succeed");

    assert_eq!(result.output_data.get("text"), Some(&json!("server-ok")));
}

#[test]
fn joins_json_items() {
    let result = execute(
        "action.text.format",
        json!({
            "operation": "join",
            "items": ["one", "two", 3],
            "delimiter": "|"
        }),
    )
    .expect("join should succeed");

    assert_eq!(result.output_data.get("text"), Some(&json!("one|two|3")));
}

fn execute(
    action_type: &str,
    config: Value,
) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
    execute_with_trigger_payload(action_type, config, Value::Null)
}

fn execute_with_trigger_payload(
    action_type: &str,
    config: Value,
    trigger_payload: Value,
) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
    let handler = HeadlessActionHandler::default();
    execute_with_handler(&handler, action_type, config, trigger_payload)
}

fn execute_with_cancellation(
    action_type: &str,
    config: Value,
    cancellation: baudbound_runtime::RuntimeCancellationToken,
) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
    let handler = HeadlessActionHandler::default();
    let context = RuntimeContext {
        cancellation,
        identity: RunIdentity {
            run_id: "run-1".to_owned(),
            script_id: "script-1".to_owned(),
            trigger_node_id: "trigger-1".to_owned(),
        },
        package_path: None,
        trigger_payload: Value::Null,
        variables: Default::default(),
    };
    let request = RuntimeActionRequest {
        action: None,
        action_type: action_type.to_owned(),
        config: config.as_object().cloned().unwrap_or_default(),
        node_id: "node-1".to_owned(),
    };

    handler.execute_action(&request, &context)
}

fn execute_with_handler(
    handler: &dyn RuntimeActionHandler,
    action_type: &str,
    config: Value,
    trigger_payload: Value,
) -> Result<baudbound_runtime::RuntimeActionResult, baudbound_runtime::RuntimeActionError> {
    let context = RuntimeContext {
        cancellation: Default::default(),
        identity: RunIdentity {
            run_id: "run-1".to_owned(),
            script_id: "script-1".to_owned(),
            trigger_node_id: "trigger-1".to_owned(),
        },
        package_path: None,
        trigger_payload,
        variables: Default::default(),
    };
    let request = RuntimeActionRequest {
        action: None,
        action_type: action_type.to_owned(),
        config: match config {
            Value::Object(config) => config,
            _ => Map::new(),
        },
        node_id: "node-1".to_owned(),
    };

    handler.execute_action(&request, &context)
}

fn platform_echo_process(message: &str) -> (&'static str, String) {
    #[cfg(windows)]
    {
        ("cmd", format!("/C echo {message}"))
    }

    #[cfg(not(windows))]
    {
        ("printf", message.to_owned())
    }
}

fn platform_echo_shell_command(message: &str) -> String {
    #[cfg(windows)]
    {
        format!("echo {message}")
    }

    #[cfg(not(windows))]
    {
        format!("printf {message}")
    }
}

fn spawn_long_running_process() -> std::process::Child {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "ping 127.0.0.1 -n 30 > NUL"])
            .spawn()
            .expect("long running process should start")
    }

    #[cfg(not(windows))]
    {
        std::process::Command::new("sh")
            .args(["-c", "sleep 30"])
            .spawn()
            .expect("long running process should start")
    }
}

struct TestHttpServer {
    join_handle: thread::JoinHandle<String>,
    url: String,
}

impl TestHttpServer {
    fn start(response: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let address = listener
            .local_addr()
            .expect("test server address should resolve");
        let join_handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test server should accept");
            let mut buffer = [0_u8; 4096];
            let bytes_read = stream
                .read(&mut buffer)
                .expect("test server should read request");
            stream
                .write_all(response.as_bytes())
                .expect("test server should write response");
            String::from_utf8_lossy(&buffer[..bytes_read]).to_string()
        });

        Self {
            join_handle,
            url: format!("http://{address}"),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.url, path)
    }

    fn join(self) -> String {
        self.join_handle
            .join()
            .expect("test server thread should finish")
    }
}
