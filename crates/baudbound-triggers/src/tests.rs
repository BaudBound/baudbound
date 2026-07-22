use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use super::*;
use crate::services::{
    file_watch::{FileWatchSpec, file_watch_event},
    hotkey::{HotkeySpec, parse_hotkey},
    process_started::{ProcessMatchMode, ProcessStartedSpec},
    serial_input::{
        SerialInputSpec, SerialReadMode, select_rebind_port, send_serial_event,
        serial_reader_groups, set_serial_reader_status, sorted_serial_reader_statuses,
    },
    websocket::{WebSocketHandshake, WebSocketRoute, websocket_payload},
};
use baudbound_actions::SerialConnectionRegistry;
use serde_json::json;
use tungstenite::Message;

#[test]
fn bounded_trigger_channel_rejects_overload_without_blocking() {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let first = TriggerEvent {
        action_type: "trigger.manual".to_owned(),
        node_id: "n-first".to_owned(),
        payload: Value::Null,
        script_id: "script-1".to_owned(),
    };
    let second = TriggerEvent {
        action_type: "trigger.manual".to_owned(),
        node_id: "n-second".to_owned(),
        payload: Value::Null,
        script_id: "script-1".to_owned(),
    };

    assert!(try_send_trigger_event(&sender, first, "test"));
    assert!(!try_send_trigger_event(&sender, second, "test"));
    assert_eq!(
        receiver
            .recv()
            .expect("first event should remain queued")
            .node_id,
        "n-first"
    );
}

#[test]
fn creates_due_schedule_events_and_advances_next_due() {
    let start = Instant::now();
    let registration = TriggerRegistration {
        action_type: "trigger.schedule".to_owned(),
        config: json!({"every": "2", "unit": "seconds"}),
        node_id: "n-schedule".to_owned(),
        runner_type: "schedule".to_owned(),
        script_id: "script-1".to_owned(),
        script_name: "Script One".to_owned(),
    };
    let mut service =
        ScheduleService::from_registrations([registration], start).expect("schedule should parse");

    assert!(service.due_events(start, UNIX_EPOCH).is_empty());

    let events = service.due_events(start + Duration::from_secs(2), UNIX_EPOCH);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].script_id, "script-1");
    assert_eq!(events[0].node_id, "n-schedule");
    assert_eq!(events[0].payload["interval_seconds"], 2);
    assert_eq!(events[0].payload["schedule"]["unit"], "seconds");
    assert_eq!(
        service.time_until_next_due(start + Duration::from_secs(2)),
        Some(Duration::from_secs(2))
    );
}

#[test]
fn rejects_invalid_schedule_interval() {
    let registration = TriggerRegistration {
        action_type: "trigger.schedule".to_owned(),
        config: json!({"every": "0", "unit": "minutes"}),
        node_id: "n-schedule".to_owned(),
        runner_type: "schedule".to_owned(),
        script_id: "script-1".to_owned(),
        script_name: "Script One".to_owned(),
    };

    let error = ScheduleService::from_registrations([registration], Instant::now())
        .expect_err("zero interval should fail");

    assert!(error.to_string().contains("positive every"));
}

#[test]
fn ignores_non_schedule_registrations() {
    let registration = TriggerRegistration {
        action_type: "trigger.manual".to_owned(),
        config: json!({}),
        node_id: "n-manual".to_owned(),
        runner_type: "manual".to_owned(),
        script_id: "script-1".to_owned(),
        script_name: "Script One".to_owned(),
    };

    let service = ScheduleService::from_registrations([registration], Instant::now())
        .expect("manual trigger should be ignored");

    assert!(service.is_empty());
}

#[test]
fn parses_hotkey_registration_and_builds_payload() {
    let service = HotkeyService::from_registrations([hotkey_registration(json!({
        "key": "Control + Alt + b"
    }))])
    .expect("hotkey should parse");

    assert_eq!(service.len(), 1);
    assert_eq!(service.registered_hotkeys(), ["Ctrl+Alt+B"]);

    let events = service
        .events_for_key("Ctrl+Alt+B", UNIX_EPOCH + Duration::from_secs(7))
        .expect("hotkey event should build");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].script_id, "script-1");
    assert_eq!(events[0].node_id, "n-hotkey");
    assert_eq!(events[0].payload["key"], "Ctrl+Alt+B");
    assert_eq!(events[0].payload["timestamp"], "7000");
}

#[test]
fn parses_windows_hotkey_catalog_categories_and_aliases() {
    let cases = [
        ("a", "A", 0, 65),
        ("control + shift + b", "Ctrl+Shift+B", 5, 66),
        ("F24", "F24", 0, 135),
        ("arrow up", "ArrowUp", 0, 38),
        ("apostrophe", "Quote", 0, 222),
        ("Numpad7", "Numpad7", 0, 103),
        ("AudioVolumeUp", "VolumeUp", 0, 175),
        ("MediaTrackNext", "MediaNext", 0, 176),
        ("BrowserBack", "BrowserBack", 0, 166),
        ("win + launch mail", "Windows+LaunchMail", 8, 180),
    ];

    for (input, expression, modifiers, virtual_key) in cases {
        let parsed = parse_hotkey(input).expect("declared Windows hotkey should parse");
        assert_eq!(parsed.expression, expression, "input: {input}");
        assert_eq!(parsed.modifiers, modifiers, "input: {input}");
        assert_eq!(parsed.virtual_keys, [virtual_key], "input: {input}");
    }
}

#[test]
fn hotkey_service_dispatches_an_unmodified_key() {
    let service = HotkeyService::from_registrations([hotkey_registration(json!({
        "key": "a"
    }))])
    .expect("plain hotkey should parse");

    assert_eq!(service.registered_hotkeys(), ["A"]);
    let events = service
        .events_for_key("A", UNIX_EPOCH)
        .expect("plain hotkey event should build");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["key"], "A");
}

#[test]
fn hotkey_service_supports_multiple_scripts_on_same_combo() {
    let mut first = hotkey_registration(json!({"key": "Ctrl+Alt+B"}));
    first.script_id = "script-1".to_owned();
    first.node_id = "n-hotkey-1".to_owned();
    let mut second = hotkey_registration(json!({"key": "control-alt-b"}));
    second.script_id = "script-2".to_owned();
    second.node_id = "n-hotkey-2".to_owned();

    let service = HotkeyService::from_registrations([first, second]).expect("hotkeys should parse");
    let events = service
        .events_for_key("Ctrl+Alt+B", UNIX_EPOCH)
        .expect("hotkey events should build");

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].script_id, "script-1");
    assert_eq!(events[1].script_id, "script-2");
}

#[test]
fn reports_trigger_service_diagnostics() {
    let schedule = ScheduleService::from_registrations(
        [TriggerRegistration {
            action_type: "trigger.schedule".to_owned(),
            config: json!({"every": "2", "unit": "seconds"}),
            node_id: "n-schedule".to_owned(),
            runner_type: "schedule".to_owned(),
            script_id: "script-1".to_owned(),
            script_name: "Script One".to_owned(),
        }],
        Instant::now(),
    )
    .expect("schedule should parse");
    let hotkeys = HotkeyService::from_registrations([
        hotkey_registration(json!({"key": "Ctrl+Alt+B"})),
        hotkey_registration(json!({"key": "Ctrl+Alt+C"})),
    ])
    .expect("hotkeys should parse");

    let schedule_diagnostics = schedule.diagnostics();
    let hotkey_diagnostics = hotkeys.diagnostics();

    assert!(schedule_diagnostics.running);
    assert_eq!(schedule_diagnostics.state, "active");
    assert!(schedule_diagnostics.summary.contains("1 schedule"));
    assert!(hotkey_diagnostics.running);
    assert_eq!(hotkey_diagnostics.state, "active");
    assert!(hotkey_diagnostics.summary.contains("2 hotkey binding(s)"));
}

#[test]
fn hotkey_service_ignores_unmatched_keys() {
    let service = HotkeyService::from_registrations([hotkey_registration(json!({
        "key": "Ctrl+Alt+B"
    }))])
    .expect("hotkey should parse");

    assert!(
        service
            .events_for_key("Ctrl+Alt+C", UNIX_EPOCH)
            .expect("unmatched key should be accepted")
            .is_empty()
    );
}

#[test]
fn accepts_hotkeys_with_multiple_regular_keys_or_only_modifiers() {
    assert_eq!(
        parse_hotkey("K+L")
            .expect("regular-key chord should parse")
            .virtual_keys,
        [75, 76]
    );
    assert_eq!(
        parse_hotkey("F1+T")
            .expect("function-key chord should parse")
            .virtual_keys,
        [84, 112]
    );
    assert_eq!(
        parse_hotkey("Ctrl+Alt")
            .expect("modifier-only chord should parse")
            .expression,
        "Ctrl+Alt"
    );
}

#[test]
fn accepts_modifiers_in_any_position_and_rejects_duplicate_keys() {
    assert_eq!(
        parse_hotkey("A+Ctrl")
            .expect("modifier order should not matter")
            .expression,
        "Ctrl+A"
    );
    assert!(parse_hotkey("Ctrl+Control+A").is_err());
    assert!(parse_hotkey("A+A").is_err());
}

#[test]
fn rejects_hotkey_runtime_templates() {
    let registration = hotkey_registration(json!({"key": "{{dynamic_key}}"}));

    let error = HotkeySpec::from_registration(&registration).expect_err("template key should fail");

    assert!(error.to_string().contains("runtime variable templates"));
}

#[test]
fn rejects_hotkey_keys_without_native_windows_mapping() {
    let registration = hotkey_registration(json!({"key": "Ctrl+MediaPlay"}));

    let error = HotkeySpec::from_registration(&registration)
        .expect_err("unsupported native key should fail");

    assert!(error.to_string().contains("not supported"));
}

#[test]
fn validates_file_watch_path_without_runtime_templates() {
    let registration = file_watch_registration(json!({"path": "{{dynamic_path}}"}));

    let error = FileWatchSpec::from_registration(&registration)
        .expect_err("runtime templates should be rejected");

    assert!(error.to_string().contains("runtime variable templates"));
}

#[test]
fn builds_file_watch_trigger_event_payload() {
    let registration = file_watch_registration(json!({"path": "C:/tmp/input.txt"}));
    let event = file_watch_event(
        &registration,
        Path::new("C:/tmp/input.txt"),
        Path::new("C:/tmp/input.txt"),
        "modified",
    );

    assert_eq!(event.script_id, "script-1");
    assert_eq!(event.node_id, "n-file");
    assert_eq!(event.payload["event"], "modified");
    assert_eq!(event.payload["path"], "C:/tmp/input.txt");
    assert_eq!(event.payload["watched_path"], "C:/tmp/input.txt");
}

#[test]
fn creates_startup_events_once_and_drains_them() {
    let registration = startup_registration();
    let startup_time = UNIX_EPOCH + Duration::from_secs(42);
    let mut service = StartupService::from_registrations([registration], startup_time)
        .expect("startup trigger should parse");

    assert_eq!(service.len(), 1);

    let events = service.drain_events();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].script_id, "script-1");
    assert_eq!(events[0].node_id, "n-startup");
    assert_eq!(events[0].payload["reason"], "runner_startup");
    assert_eq!(events[0].payload["timestamp"], "42000");
    assert!(service.is_empty());
}

#[test]
fn parses_process_started_registration() {
    let registration = process_started_registration(json!({
        "matchMode": "process_name",
        "target": "app.exe"
    }));

    let spec = ProcessStartedSpec::from_registration(registration)
        .expect("process started trigger should parse");

    assert_eq!(spec.match_mode, ProcessMatchMode::ProcessName);
    assert_eq!(spec.target, "app.exe");
}

#[cfg(not(windows))]
#[test]
fn rejects_desktop_only_process_started_window_matching() {
    let registration = process_started_registration(json!({
        "matchMode": "window_title",
        "target": "BaudBound"
    }));

    let error = ProcessStartedSpec::from_registration(registration)
        .expect_err("window matching should require desktop runner");

    assert!(error.to_string().contains("Windows Desktop"));
}

#[cfg(windows)]
#[test]
fn accepts_windows_process_started_window_matching() {
    let registration = process_started_registration(json!({
        "matchMode": "window_title",
        "target": "BaudBound"
    }));

    let spec = ProcessStartedSpec::from_registration(registration)
        .expect("Windows runner should accept native window-title matching");

    assert_eq!(spec.match_mode, ProcessMatchMode::WindowTitle);
    assert_eq!(spec.target, "BaudBound");
}

#[test]
fn parses_websocket_trigger_route() {
    let registration = websocket_registration(json!({
        "path": "/events/messages"
    }));

    let route =
        WebSocketRoute::from_registration(registration).expect("websocket route should parse");

    assert_eq!(route.path, "/events/messages");
    assert_eq!(route.registration.node_id, "n-websocket");
}

#[test]
fn builds_websocket_trigger_payload() {
    let route = WebSocketRoute::from_registration(websocket_registration(json!({
        "path": "/events/messages"
    })))
    .expect("websocket route should parse");
    let handshake = WebSocketHandshake {
        headers: BTreeMap::from([("sec-websocket-protocol".to_owned(), "json".to_owned())]),
        path: "/events/messages".to_owned(),
        query: BTreeMap::from([("token".to_owned(), "abc".to_owned())]),
    };

    let payload = websocket_payload(
        &route,
        &handshake,
        "conn-1",
        "127.0.0.1:50000",
        Message::Text(r#"{"event":"ok"}"#.to_owned().into()),
        1024,
    )
    .expect("websocket payload should build");

    assert_eq!(payload["connection_id"], "conn-1");
    assert_eq!(payload["path"], "/events/messages");
    assert_eq!(payload["query"]["token"], "abc");
    assert_eq!(payload["headers"]["sec-websocket-protocol"], "json");
    assert_eq!(payload["message"], r#"{"event":"ok"}"#);
    assert_eq!(payload["json"]["event"], "ok");
    assert_eq!(payload["remote_address"], "127.0.0.1:50000");
}

#[test]
fn parses_serial_input_trigger_config() {
    let registration = serial_input_registration(json!({
        "deviceId": "main-device"
    }));
    let connections = SerialConnectionRegistry::new([serial_device_config()]);

    let spec = SerialInputSpec::from_registration(&registration, &connections)
        .expect("serial config should parse");

    assert_eq!(spec.device_id, "main-device");
    assert_eq!(spec.port, "COM3");
    assert_eq!(spec.read_mode, SerialReadMode::IdleGap);
    assert!(spec.auto_reconnect);
}

#[test]
fn groups_serial_input_triggers_by_logical_device() {
    let connections = SerialConnectionRegistry::new([serial_device_config()]);
    let mut second = serial_input_registration(json!({ "deviceId": "main-device" }));
    second.node_id = "n-serial-2".to_owned();
    second.script_id = "script-2".to_owned();

    let readers = serial_reader_groups(
        [
            serial_input_registration(json!({ "deviceId": "main-device" })),
            second,
        ],
        &connections,
    )
    .expect("serial readers should group");

    assert_eq!(readers.len(), 1);
    assert_eq!(readers["main-device"].1.len(), 2);
}

#[test]
fn automatic_rebind_requires_exactly_one_changed_port() {
    let one = [test_serial_port("COM7")];
    let multiple = [test_serial_port("COM7"), test_serial_port("COM8")];

    assert_eq!(
        select_rebind_port("controller", "COM3", &one, "missing")
            .expect("one match should be selected"),
        Some("COM7".to_owned())
    );
    assert_eq!(
        select_rebind_port("controller", "COM7", &one, "unavailable")
            .expect("the current port should need no change"),
        None
    );
    assert!(select_rebind_port("controller", "COM3", &[], "missing").is_err());
    assert!(select_rebind_port("controller", "COM3", &multiple, "missing").is_err());
}

fn test_serial_port(port_name: &str) -> serialport::SerialPortInfo {
    serialport::SerialPortInfo {
        port_name: port_name.to_owned(),
        port_type: serialport::SerialPortType::Unknown,
    }
}

#[test]
fn rejects_serial_input_when_runner_device_is_missing() {
    let registration = serial_input_registration(json!({
        "deviceId": "missing-device"
    }));
    let connections = SerialConnectionRegistry::default();

    let error = SerialInputSpec::from_registration(&registration, &connections)
        .expect_err("missing runner serial device should fail");

    assert!(
        error
            .to_string()
            .contains("[serial.devices.missing-device]")
    );
}

#[test]
fn service_validation_rejects_missing_serial_device_configuration() {
    let error = SerialInputService::validate(
        [serial_input_registration(
            json!({ "deviceId": "missing-device" }),
        )],
        &SerialConnectionRegistry::default(),
    )
    .expect_err("service validation should reject an unconfigured serial device");

    assert!(
        error
            .to_string()
            .contains("[serial.devices.missing-device]")
    );
}

#[test]
fn builds_serial_input_trigger_event_payload() {
    let registration = serial_input_registration(json!({
        "deviceId": "main-device"
    }));
    let connections = SerialConnectionRegistry::new([serial_device_config()]);
    let spec = SerialInputSpec::from_registration(&registration, &connections)
        .expect("serial config should parse");
    let (sender, receiver) = std::sync::mpsc::sync_channel(32);

    send_serial_event(&registration, &spec, &sender, b"hello", None);

    let event = receiver.recv().expect("serial event should be sent");
    assert_eq!(event.script_id, "script-1");
    assert_eq!(event.node_id, "n-serial");
    assert_eq!(event.payload["device_id"], "main-device");
    assert_eq!(event.payload["data"], "hello");
    assert_eq!(event.payload["bytes"], 5);
    assert!(event.payload["timestamp"].as_str().is_some());
}

#[test]
fn tracks_serial_reader_status() {
    let registration = serial_input_registration(json!({
        "deviceId": "main-device"
    }));
    let connections = SerialConnectionRegistry::new([serial_device_config()]);
    let spec = SerialInputSpec::from_registration(&registration, &connections)
        .expect("serial config should parse");
    let statuses = Arc::new(Mutex::new(BTreeMap::new()));
    let (sender, receiver) = std::sync::mpsc::sync_channel(32);

    set_serial_reader_status(
        &statuses,
        &registration,
        &spec,
        "connecting",
        Some("open failed".to_owned()),
        false,
    );
    send_serial_event(&registration, &spec, &sender, b"hello", Some(&statuses));

    let _ = receiver.recv().expect("serial event should be sent");
    let readers = sorted_serial_reader_statuses(&statuses);
    assert_eq!(readers.len(), 1);
    assert_eq!(readers[0].device_id, "main-device");
    assert_eq!(readers[0].state, "reading");
    assert_eq!(readers[0].last_error.as_deref(), Some("open failed"));
    assert!(readers[0].last_error_unix.is_some());
    assert!(readers[0].last_event_unix.is_some());
}

#[test]
fn matches_webhook_request_and_builds_payload() {
    let service = WebhookService::from_registrations([webhook_registration(json!({
        "method": "POST",
        "hookName": "deploy",
        "responseTimeoutSeconds": "0.25",
        "waitForResponse": false
    }))])
    .expect("webhook should register");
    let dispatch = service
        .dispatch_for_request(&WebhookRequest {
            body: r#"{"status":"ok"}"#.to_owned(),
            headers: BTreeMap::from([("content-type".to_owned(), "application/json".to_owned())]),
            method: "post".to_owned(),
            path_and_query: "/events/deploy?source=test".to_owned(),
        })
        .expect("request should match webhook");

    assert_eq!(dispatch.event.script_id, "script-1");
    assert_eq!(dispatch.event.node_id, "n-webhook");
    assert_eq!(dispatch.event.payload["method"], "POST");
    assert_eq!(dispatch.event.payload["path"], "/events/deploy");
    assert_eq!(dispatch.event.payload["query"]["source"], "test");
    assert_eq!(dispatch.event.payload["json"]["status"], "ok");
    assert_eq!(dispatch.fallback_response.status_code, 200);
    assert_eq!(dispatch.response_timeout, Duration::from_millis(250));
}

#[test]
fn rejects_invalid_webhook_response_timeouts() {
    for timeout in [json!(0), json!(-1), json!("not-a-number")] {
        let error = WebhookService::from_registrations([webhook_registration(json!({
            "method": "POST",
            "hookName": "deploy",
            "responseTimeoutSeconds": timeout,
            "waitForResponse": true
        }))])
        .expect_err("invalid webhook timeout must fail registration");

        assert!(
            error.to_string().contains("positive finite number"),
            "{error}"
        );
    }

    let error = WebhookService::from_registrations([webhook_registration(json!({
        "method": "POST",
        "hookName": "deploy",
        "responseTimeoutSeconds": 1e100,
        "waitForResponse": true
    }))])
    .expect_err("out-of-range webhook timeout must fail registration");
    assert!(error.to_string().contains("out of range"), "{error}");
}

#[test]
fn extracts_waiting_webhook_response_from_run_report() {
    let service = WebhookService::from_registrations([webhook_registration(json!({
        "method": "POST",
        "hookName": "deploy",
        "waitForResponse": true,
        "timeoutResponseStatus": "202",
        "timeoutResponseBody": "fallback"
    }))])
    .expect("webhook should register");
    let dispatch = service
        .dispatch_for_request(&WebhookRequest {
            body: String::new(),
            headers: BTreeMap::new(),
            method: "POST".to_owned(),
            path_and_query: "/events/deploy".to_owned(),
        })
        .expect("request should match webhook");
    let report = RunReport {
        identity: baudbound_runtime::RunIdentity {
            run_id: "run-1".to_owned(),
            script_id: "script-1".to_owned(),
            trigger_node_id: "n-webhook".to_owned(),
        },
        logs: Vec::new(),
        variable_scopes: BTreeMap::new(),
        variables: BTreeMap::from([
            ("n-response.sent".to_owned(), Value::Bool(true)),
            (
                "n-response.status_code".to_owned(),
                Value::Number(serde_json::Number::from(201)),
            ),
            (
                "n-response.content_type".to_owned(),
                Value::String("application/json".to_owned()),
            ),
            (
                "n-response.body".to_owned(),
                Value::String(r#"{"ok":true}"#.to_owned()),
            ),
            (
                "n-response.trigger_id".to_owned(),
                Value::String("n-webhook".to_owned()),
            ),
        ]),
    };

    let response = service.response_for_report(&dispatch, &report);

    assert_eq!(response.status_code, 201);
    assert_eq!(response.content_type, "application/json");
    assert_eq!(response.body, r#"{"ok":true}"#);
}

#[test]
fn waiting_webhook_uses_fallback_when_no_response_node_sent() {
    let service = WebhookService::from_registrations([webhook_registration(json!({
        "method": "POST",
        "hookName": "deploy",
        "waitForResponse": true,
        "timeoutResponseStatus": "202",
        "timeoutResponseBody": "fallback"
    }))])
    .expect("webhook should register");
    let dispatch = service
        .dispatch_for_request(&WebhookRequest {
            body: String::new(),
            headers: BTreeMap::new(),
            method: "POST".to_owned(),
            path_and_query: "/events/deploy".to_owned(),
        })
        .expect("request should match webhook");
    let report = RunReport {
        identity: baudbound_runtime::RunIdentity {
            run_id: "run-1".to_owned(),
            script_id: "script-1".to_owned(),
            trigger_node_id: "n-webhook".to_owned(),
        },
        logs: Vec::new(),
        variable_scopes: BTreeMap::new(),
        variables: BTreeMap::new(),
    };

    let response = service.response_for_report(&dispatch, &report);

    assert_eq!(response.status_code, 202);
    assert_eq!(response.body, "fallback");
}

fn webhook_registration(config: Value) -> TriggerRegistration {
    TriggerRegistration {
        action_type: "trigger.webhook".to_owned(),
        config,
        node_id: "n-webhook".to_owned(),
        runner_type: "webhook".to_owned(),
        script_id: "script-1".to_owned(),
        script_name: "Script One".to_owned(),
    }
}

fn websocket_registration(config: Value) -> TriggerRegistration {
    TriggerRegistration {
        action_type: "trigger.websocket".to_owned(),
        config,
        node_id: "n-websocket".to_owned(),
        runner_type: "websocket".to_owned(),
        script_id: "script-1".to_owned(),
        script_name: "Script One".to_owned(),
    }
}

fn startup_registration() -> TriggerRegistration {
    TriggerRegistration {
        action_type: "trigger.startup".to_owned(),
        config: json!({}),
        node_id: "n-startup".to_owned(),
        runner_type: "startup".to_owned(),
        script_id: "script-1".to_owned(),
        script_name: "Script One".to_owned(),
    }
}

fn process_started_registration(config: Value) -> TriggerRegistration {
    TriggerRegistration {
        action_type: "trigger.process_started".to_owned(),
        config,
        node_id: "n-process-started".to_owned(),
        runner_type: "process_started".to_owned(),
        script_id: "script-1".to_owned(),
        script_name: "Script One".to_owned(),
    }
}

fn hotkey_registration(config: Value) -> TriggerRegistration {
    TriggerRegistration {
        action_type: "trigger.hotkey".to_owned(),
        config,
        node_id: "n-hotkey".to_owned(),
        runner_type: "hotkey".to_owned(),
        script_id: "script-1".to_owned(),
        script_name: "Script One".to_owned(),
    }
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
        product_id: Some("7523".to_owned()),
        product: None,
        read_mode: "idle_gap".to_owned(),
        serial_number: None,
        stop_bits: "1".to_owned(),
        validate_usb_identity: true,
        vendor_id: Some("0x1A86".to_owned()),
    }
}

fn file_watch_registration(config: Value) -> TriggerRegistration {
    TriggerRegistration {
        action_type: "trigger.file_watch".to_owned(),
        config,
        node_id: "n-file".to_owned(),
        runner_type: "file_watch".to_owned(),
        script_id: "script-1".to_owned(),
        script_name: "Script One".to_owned(),
    }
}

fn serial_input_registration(config: Value) -> TriggerRegistration {
    TriggerRegistration {
        action_type: "trigger.serial_input".to_owned(),
        config,
        node_id: "n-serial".to_owned(),
        runner_type: "serial_input".to_owned(),
        script_id: "script-1".to_owned(),
        script_name: "Script One".to_owned(),
    }
}
