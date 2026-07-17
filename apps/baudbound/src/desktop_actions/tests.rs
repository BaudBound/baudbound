#[cfg(windows)]
use super::dialogs::message_box_result;
#[cfg(windows)]
use super::mouse::{normalize_mouse_button, normalize_mouse_click_type};
use super::*;
use super::{audio::beep_config, config::required_i32, screen::pixel_color_map};
use serde_json::{Map, Number, Value};
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::{IDCANCEL, IDNO, IDOK, IDYES};

#[test]
fn validates_beep_configuration_without_audio_io() {
    let request = RuntimeActionRequest {
        action: None,
        action_type: "action.beep".to_owned(),
        config: Map::from_iter([
            ("frequencyHz".to_owned(), Value::String("880.5".to_owned())),
            ("durationMs".to_owned(), Value::String("250".to_owned())),
        ]),
        node_id: "n-beep".to_owned(),
    };

    assert_eq!(beep_config(&request).unwrap(), (880.5, 250.0));
    assert_eq!(
        beep_config(&RuntimeActionRequest {
            config: Map::new(),
            ..request.clone()
        })
        .unwrap(),
        (800.0, 200.0)
    );

    for (key, value) in [
        ("frequencyHz", "19"),
        ("frequencyHz", "20001"),
        ("durationMs", "9"),
        ("durationMs", "5001"),
    ] {
        let error = beep_config(&RuntimeActionRequest {
            config: Map::from_iter([(key.to_owned(), Value::String(value.to_owned()))]),
            ..request.clone()
        })
        .expect_err("out-of-range beep configuration should fail");
        assert!(error.to_string().contains("must be between"));
    }
}

#[test]
#[cfg(windows)]
fn normalizes_mouse_buttons() {
    let request = RuntimeActionRequest {
        action: None,
        action_type: "action.mouse".to_owned(),
        config: Map::new(),
        node_id: "n-mouse".to_owned(),
    };

    assert_eq!(
        normalize_mouse_button(&request, "right").unwrap().name,
        "right"
    );
    assert_eq!(
        normalize_mouse_button(&request, "middle").unwrap().name,
        "middle"
    );
    assert_eq!(normalize_mouse_button(&request, "").unwrap().name, "left");
    assert!(normalize_mouse_button(&request, "unknown").is_err());
}

#[test]
#[cfg(windows)]
fn normalizes_mouse_click_types() {
    assert_eq!(normalize_mouse_click_type("double"), "double");
    assert_eq!(normalize_mouse_click_type("triple"), "triple");
    assert_eq!(normalize_mouse_click_type("unknown"), "single");
}

#[test]
#[cfg(windows)]
fn maps_message_box_results() {
    assert_eq!(message_box_result(IDOK).as_deref(), Some("ok"));
    assert_eq!(message_box_result(IDCANCEL).as_deref(), Some("cancel"));
    assert_eq!(message_box_result(IDYES).as_deref(), Some("yes"));
    assert_eq!(message_box_result(IDNO).as_deref(), Some("no"));
    assert_eq!(message_box_result(-1), None);
}

#[test]
#[cfg(windows)]
fn pre_cancelled_message_box_returns_without_opening_native_ui() {
    let cancellation = baudbound_runtime::RuntimeCancellationToken::new();
    cancellation.cancel();
    let request = RuntimeActionRequest {
        action: None,
        action_type: "action.message_box".to_owned(),
        config: Map::from_iter([
            ("title".to_owned(), Value::String("Test".to_owned())),
            ("message".to_owned(), Value::String("Test".to_owned())),
        ]),
        node_id: "n-message-box".to_owned(),
    };
    let context = RuntimeContext {
        cancellation,
        identity: baudbound_runtime::RunIdentity {
            run_id: "run-1".to_owned(),
            script_id: "script-1".to_owned(),
            trigger_node_id: "trigger-1".to_owned(),
        },
        package_path: None,
        trigger_payload: Value::Null,
        variables: Default::default(),
    };

    let error = SystemDesktopActionAdapter
        .message_box(&request, &context)
        .expect_err("pre-cancelled message box should not open");

    assert!(matches!(error, RuntimeActionError::Cancelled));
}

#[test]
#[cfg(not(windows))]
fn message_box_is_rejected_without_a_native_cancellable_backend() {
    let request = RuntimeActionRequest {
        action: None,
        action_type: "action.message_box".to_owned(),
        config: Map::new(),
        node_id: "n-message-box".to_owned(),
    };
    let context = RuntimeContext {
        cancellation: Default::default(),
        identity: baudbound_runtime::RunIdentity {
            run_id: "run-1".to_owned(),
            script_id: "script-1".to_owned(),
            trigger_node_id: "trigger-1".to_owned(),
        },
        package_path: None,
        trigger_payload: Value::Null,
        variables: Default::default(),
    };

    let error = SystemDesktopActionAdapter
        .message_box(&request, &context)
        .expect_err("message box should be unavailable without a native backend");

    assert!(matches!(error, RuntimeActionError::Unsupported(_)));
}

#[test]
fn builds_pixel_color_metadata() {
    let output = pixel_color_map(-12, 34, 16, 32, 48, 255);

    assert_eq!(
        output.get("hex"),
        Some(&Value::String("#102030".to_owned()))
    );
    assert_eq!(output.get("red"), Some(&Value::Number(Number::from(16))));
    assert_eq!(output.get("green"), Some(&Value::Number(Number::from(32))));
    assert_eq!(output.get("blue"), Some(&Value::Number(Number::from(48))));
    assert_eq!(output.get("alpha"), Some(&Value::Number(Number::from(255))));
    assert_eq!(
        output.get("integer"),
        Some(&Value::Number(Number::from(0x10_20_30_u32)))
    );
    assert_eq!(output.get("x"), Some(&Value::Number(Number::from(-12))));
    assert_eq!(output.get("y"), Some(&Value::Number(Number::from(34))));
}

#[test]
fn parses_signed_screen_coordinates() {
    let request = RuntimeActionRequest {
        action: None,
        action_type: "action.pixel.get".to_owned(),
        config: Map::from_iter([("x".to_owned(), Value::String("-2147483648".to_owned()))]),
        node_id: "n-pixel".to_owned(),
    };

    assert_eq!(required_i32(&request, "x").unwrap(), i32::MIN);
}

#[test]
fn asset_sound_requires_package_context_before_audio_io() {
    let adapter = SystemDesktopActionAdapter;
    let request = RuntimeActionRequest {
        action: None,
        action_type: "action.sound.play".to_owned(),
        config: Map::from_iter([
            ("source".to_owned(), Value::String("asset".to_owned())),
            (
                "assetPath".to_owned(),
                Value::String("assets/sounds/beep.wav".to_owned()),
            ),
        ]),
        node_id: "n-sound".to_owned(),
    };
    let context = RuntimeContext {
        cancellation: Default::default(),
        identity: baudbound_runtime::RunIdentity {
            run_id: "run-1".to_owned(),
            script_id: "script-1".to_owned(),
            trigger_node_id: "n-trigger".to_owned(),
        },
        package_path: None,
        trigger_payload: Value::Null,
        variables: Default::default(),
    };

    let error = adapter
        .sound_play(&request, &context)
        .expect_err("asset playback without package should fail");

    assert!(error.to_string().contains("installed package context"));
}

#[cfg(windows)]
#[test]
fn windows_process_title_actions_handle_missing_windows_safely() {
    let adapter = SystemDesktopActionAdapter;
    let request = RuntimeActionRequest {
        action: None,
        action_type: "action.process.status".to_owned(),
        config: Map::from_iter([(
            "target".to_owned(),
            Value::String("BaudBound-Window-That-Does-Not-Exist-7B8C3D9E".to_owned()),
        )]),
        node_id: "n-process-status".to_owned(),
    };
    let context = RuntimeContext {
        cancellation: Default::default(),
        identity: baudbound_runtime::RunIdentity {
            run_id: "run-1".to_owned(),
            script_id: "script-1".to_owned(),
            trigger_node_id: "n-trigger".to_owned(),
        },
        package_path: None,
        trigger_payload: Value::Null,
        variables: Default::default(),
    };

    let status = adapter
        .process_status_by_window_title(&request, &context)
        .expect("missing window status should produce a not-found result");
    assert_eq!(status.output_data.get("running"), Some(&Value::Bool(false)));
    assert_eq!(
        status.output_data.get("state"),
        Some(&Value::String("not_found".to_owned()))
    );

    let kill_request = RuntimeActionRequest {
        action_type: "action.process.kill".to_owned(),
        node_id: "n-process-kill".to_owned(),
        ..request
    };
    let error = adapter
        .kill_process_by_window_title(&kill_request, &context)
        .expect_err("terminating a missing window must fail safely");
    assert!(
        error
            .to_string()
            .contains("no process window title contains")
    );
}
