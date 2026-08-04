use super::dialogs::{
    DesktopDialogContent, DesktopDialogError, DesktopDialogProvider, DesktopDialogRequest,
    DesktopDialogResponse, DesktopDialogSize, MessageDialogButtons, MessageDialogVariant,
};
#[cfg(windows)]
use super::input::InputAction;
#[cfg(windows)]
use super::mouse::{normalize_mouse_button, normalize_mouse_click_type};
use super::*;
use super::{audio::beep_config, config::required_i32, screen::pixel_color_map};
use serde_json::{Map, Number, Value};
use std::{sync::Mutex, time::Duration};

struct FixedDialogProvider {
    request: Mutex<Option<DesktopDialogRequest>>,
    response: Mutex<Option<DesktopDialogResponse>>,
    timeout: Mutex<Option<Option<Duration>>>,
}

impl FixedDialogProvider {
    fn new(response: DesktopDialogResponse) -> Self {
        Self {
            request: Mutex::new(None),
            response: Mutex::new(Some(response)),
            timeout: Mutex::new(None),
        }
    }
}

impl DesktopDialogProvider for FixedDialogProvider {
    fn show_dialog(
        &self,
        request: DesktopDialogRequest,
        _cancellation: &baudbound_runtime::RuntimeCancellationToken,
        timeout: Option<Duration>,
    ) -> Result<DesktopDialogResponse, DesktopDialogError> {
        *self
            .request
            .lock()
            .expect("request lock should not be poisoned") = Some(request);
        *self
            .timeout
            .lock()
            .expect("timeout lock should not be poisoned") = Some(timeout);
        self.response
            .lock()
            .expect("response lock should not be poisoned")
            .take()
            .ok_or_else(|| {
                DesktopDialogError::Failed("test response was already consumed".to_owned())
            })
    }
}

fn dialog_context() -> RuntimeContext {
    RuntimeContext {
        cancellation: Default::default(),
        identity: baudbound_runtime::RunIdentity {
            run_id: "run-1".to_owned(),
            script_id: "script-1".to_owned(),
            trigger_node_id: "trigger-1".to_owned(),
        },
        package_bytes: None,
        package_path: None,
        trigger_payload: Value::Null,
        variables: Default::default(),
    }
}

fn message_dialog_request(buttons: MessageDialogButtons) -> DesktopDialogRequest {
    DesktopDialogRequest {
        requesting_script: "script-1".to_owned(),
        timeout_at_unix_ms: None,
        title: "Title".to_owned(),
        content: DesktopDialogContent::MessageDialog {
            buttons,
            dialog_size: DesktopDialogSize::Medium,
            message: "Message".to_owned(),
            variant: MessageDialogVariant::Info,
        },
    }
}

#[test]
fn message_dialog_close_semantics_are_platform_independent() {
    for (buttons, expected) in [
        (MessageDialogButtons::Ok, Some("ok")),
        (MessageDialogButtons::OkCancel, Some("cancel")),
        (MessageDialogButtons::CancelConfirm, Some("cancel")),
        (MessageDialogButtons::YesNo, None),
        (MessageDialogButtons::YesNoCancel, Some("cancel")),
    ] {
        assert_eq!(
            message_dialog_request(buttons)
                .close_response()
                .map(|response| response.button),
            expected.map(str::to_owned)
        );
    }
}

#[test]
fn message_dialog_returns_the_configured_confirm_button() {
    let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse::button(
        "confirm",
    )));
    let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());
    let request = RuntimeActionRequest {
        action: Some("show_message_box".to_owned()),
        action_type: "action.message_box".to_owned(),
        config: Map::from_iter([
            ("type".to_owned(), Value::String("info".to_owned())),
            (
                "buttons".to_owned(),
                Value::String("cancel_confirm".to_owned()),
            ),
            ("dialogSize".to_owned(), Value::String("medium".to_owned())),
            ("title".to_owned(), Value::String("Confirm".to_owned())),
            ("message".to_owned(), Value::String("Continue?".to_owned())),
        ]),
        node_id: "n-message-confirm".to_owned(),
    };

    let result = adapter
        .message_box(&request, &dialog_context())
        .expect("Cancel / Confirm should accept the Confirm response");

    assert_eq!(
        result.output_data.get("button"),
        Some(&Value::String("confirm".to_owned()))
    );
    assert_eq!(
        result.output_data.get("buttons"),
        Some(&Value::String("cancel_confirm".to_owned()))
    );
    let recorded = provider
        .request
        .lock()
        .expect("request lock should not be poisoned");
    assert!(matches!(
        recorded.as_ref().map(|request| &request.content),
        Some(DesktopDialogContent::MessageDialog {
            buttons: MessageDialogButtons::CancelConfirm,
            ..
        })
    ));
}

#[test]
fn message_dialog_accepts_and_forwards_a_timeout() {
    let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse::button(
        "timeout",
    )));
    let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());
    let request = RuntimeActionRequest {
        action: Some("show_message_box".to_owned()),
        action_type: "action.message_box".to_owned(),
        config: Map::from_iter([
            ("type".to_owned(), Value::String("info".to_owned())),
            ("buttons".to_owned(), Value::String("ok".to_owned())),
            ("dialogSize".to_owned(), Value::String("medium".to_owned())),
            (
                "title".to_owned(),
                Value::String("Timed message".to_owned()),
            ),
            (
                "message".to_owned(),
                Value::String("Wait for input.".to_owned()),
            ),
            ("timeoutSeconds".to_owned(), Value::String("2.5".to_owned())),
        ]),
        node_id: "n-message-timeout".to_owned(),
    };

    let result = adapter
        .message_box(&request, &dialog_context())
        .expect("a message dialog timeout should be a valid result");

    assert_eq!(
        result.output_data.get("button"),
        Some(&Value::String("timeout".to_owned()))
    );
    assert_eq!(
        *provider
            .timeout
            .lock()
            .expect("timeout lock should not be poisoned"),
        Some(Some(Duration::from_millis(2_500)))
    );
}

#[test]
fn message_dialog_requires_an_explicit_button_set() {
    let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse::button(
        "ok",
    )));
    let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());
    let request = RuntimeActionRequest {
        action: Some("show_message_box".to_owned()),
        action_type: "action.message_box".to_owned(),
        config: Map::from_iter([
            ("type".to_owned(), Value::String("info".to_owned())),
            ("dialogSize".to_owned(), Value::String("medium".to_owned())),
            ("title".to_owned(), Value::String("Message".to_owned())),
            ("message".to_owned(), Value::String("Body".to_owned())),
        ]),
        node_id: "n-message-missing-buttons".to_owned(),
    };

    let error = adapter
        .message_box(&request, &dialog_context())
        .expect_err("a missing button set must fail before opening a dialog");

    assert!(error.to_string().contains("buttons"));
    assert!(
        provider
            .request
            .lock()
            .expect("request lock should not be poisoned")
            .is_none(),
        "invalid dialog configuration must not reach the desktop provider"
    );
}

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
    assert_eq!(
        normalize_mouse_click_type("single").as_deref(),
        Some("single")
    );
    assert_eq!(
        normalize_mouse_click_type("double").as_deref(),
        Some("double")
    );
    assert_eq!(normalize_mouse_click_type("triple"), None);
    assert_eq!(normalize_mouse_click_type("unknown"), None);
}

#[test]
#[cfg(windows)]
fn parses_shared_keyboard_and_mouse_input_actions() {
    let request = |input_action: Option<&str>| RuntimeActionRequest {
        action: None,
        action_type: "action.keyboard".to_owned(),
        config: input_action.map_or_else(Map::new, |value| {
            Map::from_iter([("inputAction".to_owned(), Value::String(value.to_owned()))])
        }),
        node_id: "n-keyboard".to_owned(),
    };

    assert_eq!(
        InputAction::from_request(&request(None)).unwrap(),
        InputAction::Press
    );
    assert_eq!(
        InputAction::from_request(&request(Some("press"))).unwrap(),
        InputAction::Press
    );
    assert_eq!(
        InputAction::from_request(&request(Some("down"))).unwrap(),
        InputAction::Down
    );
    assert_eq!(
        InputAction::from_request(&request(Some("up"))).unwrap(),
        InputAction::Up
    );
    assert!(InputAction::from_request(&request(Some("toggle"))).is_err());
}

#[test]
fn pre_cancelled_message_dialog_returns_without_opening_a_window() {
    let cancellation = baudbound_runtime::RuntimeCancellationToken::new();
    cancellation.cancel();
    let request = RuntimeActionRequest {
        action: None,
        action_type: "action.message_box".to_owned(),
        config: Map::from_iter([
            ("type".to_owned(), Value::String("info".to_owned())),
            ("buttons".to_owned(), Value::String("ok".to_owned())),
            ("title".to_owned(), Value::String("Test".to_owned())),
            ("message".to_owned(), Value::String("Test".to_owned())),
            ("dialogSize".to_owned(), Value::String("medium".to_owned())),
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
        package_bytes: None,
        package_path: None,
        trigger_payload: Value::Null,
        variables: Default::default(),
    };

    let error = SystemDesktopActionAdapter::default()
        .message_box(&request, &context)
        .expect_err("pre-cancelled message box should not open");

    assert!(matches!(error, RuntimeActionError::Cancelled));
}

#[test]
#[cfg(windows)]
fn pre_cancelled_type_text_returns_without_sending_native_input() {
    let cancellation = baudbound_runtime::RuntimeCancellationToken::new();
    cancellation.cancel();
    let request = RuntimeActionRequest {
        action: None,
        action_type: "action.keyboard.type_text".to_owned(),
        config: Map::from_iter([(
            "text".to_owned(),
            Value::String("must not be typed".to_owned()),
        )]),
        node_id: "n-type-text".to_owned(),
    };
    let context = RuntimeContext {
        cancellation,
        identity: baudbound_runtime::RunIdentity {
            run_id: "run-1".to_owned(),
            script_id: "script-1".to_owned(),
            trigger_node_id: "trigger-1".to_owned(),
        },
        package_bytes: None,
        package_path: None,
        trigger_payload: Value::Null,
        variables: Default::default(),
    };

    let error = SystemDesktopActionAdapter::default()
        .keyboard_type_text(&request, &context)
        .expect_err("pre-cancelled Type Text should not send native input");

    assert!(matches!(error, RuntimeActionError::Cancelled));
}

#[test]
fn message_dialog_is_rejected_without_an_interactive_provider() {
    let request = RuntimeActionRequest {
        action: None,
        action_type: "action.message_box".to_owned(),
        config: Map::from_iter([
            ("type".to_owned(), Value::String("info".to_owned())),
            ("buttons".to_owned(), Value::String("ok".to_owned())),
            ("title".to_owned(), Value::String("Test".to_owned())),
            ("message".to_owned(), Value::String("Test".to_owned())),
            ("dialogSize".to_owned(), Value::String("medium".to_owned())),
        ]),
        node_id: "n-message-box".to_owned(),
    };
    let context = RuntimeContext {
        cancellation: Default::default(),
        identity: baudbound_runtime::RunIdentity {
            run_id: "run-1".to_owned(),
            script_id: "script-1".to_owned(),
            trigger_node_id: "trigger-1".to_owned(),
        },
        package_bytes: None,
        package_path: None,
        trigger_payload: Value::Null,
        variables: Default::default(),
    };

    let error = SystemDesktopActionAdapter::default()
        .message_box(&request, &context)
        .expect_err("message dialog should be unavailable without an interactive provider");

    assert!(error.to_string().contains("DESKTOP_DIALOG_UNAVAILABLE"));
}

#[test]
fn form_dialog_returns_typed_values_and_configured_choice_order() {
    let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse {
        button: "ok".to_owned(),
        values: serde_json::json!({
            "accepted": true,
            "amount": 3.5,
            "color": "#12AB34",
            "date": "2026-08-03",
            "dateTime": "2026-08-03T09:30:00Z",
            "dropdown": "production",
            "environment": "production",
            "files": ["C:\\one.txt", "C:\\two.txt"],
            "features": ["second", "first"],
            "folder": "C:\\work",
            "name": "Ada",
            "slider": 25.0,
            "time": "12:30:00"
        })
        .as_object()
        .expect("test response must be an object")
        .clone(),
    }));
    let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());
    let request = form_dialog_request(serde_json::json!([
        {"type":"information","label":"Deployment","description":"Review the values.","accentColor":"rgb(18, 52, 86)"},
        {"type":"section_heading","label":"Details","description":"","accentColor":"#123456"},
        {"type":"divider","label":"","description":"","accentColor":"#123456"},
        {"type":"text","key":"name","label":"Name","description":"","required":true,"placeholder":"Name","defaultValue":""},
        {"type":"number","key":"amount","label":"Amount","description":"","required":false,"placeholder":"0","defaultValue":""},
        {"type":"checkbox","key":"accepted","label":"Accept","description":"","required":true,"defaultChecked":false},
        {"type":"single_choice","key":"environment","label":"Environment","description":"","required":true,"choices":[{"key":"production","displayValue":"Production"}]},
        {"type":"multi_choice","key":"features","label":"Features","description":"","required":true,"choices":[{"key":"first","displayValue":"First"},{"key":"second","displayValue":"Second"}]},
        {"type":"dropdown","key":"dropdown","label":"Dropdown","description":"","required":true,"choices":[{"key":"production","displayValue":"Production"}]},
        {"type":"date","key":"date","label":"Date","description":"","required":true,"defaultValue":"2026-08-03"},
        {"type":"time","key":"time","label":"Time","description":"","required":true,"defaultValue":"12:30:00"},
        {"type":"datetime","key":"dateTime","label":"Date time","description":"","required":true,"defaultValue":"2026-08-03T12:30:00","timezone":"Europe/Helsinki"},
        {"type":"color","key":"color","label":"Color","description":"","required":true,"defaultValue":"#12AB34"},
        {"type":"file","key":"files","label":"Files","description":"","required":true,"multiple":true},
        {"type":"folder","key":"folder","label":"Folder","description":"","required":true},
        {"type":"slider","key":"slider","label":"Slider","description":"","required":true,"defaultValue":25,"minimum":0,"maximum":100,"step":5}
    ]));

    let result = adapter
        .form_dialog(&request, &dialog_context())
        .expect("form dialog should succeed");

    assert_eq!(result.output_data["submitted"], Value::Bool(true));
    assert_eq!(result.output_data["values"]["name"], "Ada");
    assert_eq!(result.output_data["values"]["amount"], 3.5);
    assert_eq!(
        result.output_data["values"]["dateTime"],
        "2026-08-03T09:30:00Z"
    );
    assert_eq!(result.output_data["values"]["slider"], 25.0);
    assert_eq!(
        result.output_data["values"]["features"],
        serde_json::json!(["first", "second"])
    );
    assert!(result.sensitive_output_keys.is_empty());

    let recorded = provider
        .request
        .lock()
        .expect("request lock should not be poisoned")
        .as_ref()
        .expect("request should be recorded")
        .clone();
    let serialized = serde_json::to_value(recorded).expect("request should serialize");
    assert_eq!(serialized["fields"][0]["type"], "information");
    assert_eq!(serialized["fields"][0]["accentColor"], "#123456");
    assert_eq!(serialized["dialogSize"], "medium");
    assert_eq!(serialized["fields"][6]["choices"][0]["key"], "production");
}

#[test]
fn form_dialog_converts_typed_datetime_defaults_for_each_temporal_component() {
    let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse::button(
        "cancel",
    )));
    let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());
    let typed_datetime = serde_json::json!({
        "type": "datetime",
        "value": "2026-08-03T12:30:15Z"
    });
    let request = form_dialog_request(serde_json::json!([
        {"type":"date","key":"date","label":"Date","description":"","required":false,"defaultValue":typed_datetime},
        {"type":"time","key":"time","label":"Time","description":"","required":false,"defaultValue":typed_datetime},
        {"type":"datetime","key":"dateTime","label":"Date time","description":"","required":false,"defaultValue":typed_datetime,"timezone":"Europe/Helsinki"}
    ]));

    let result = adapter
        .form_dialog(&request, &dialog_context())
        .expect("typed datetime defaults should open the form dialog");
    assert_eq!(result.output_data["submitted"], Value::Bool(false));

    let recorded = provider
        .request
        .lock()
        .expect("request lock should not be poisoned")
        .as_ref()
        .expect("request should be recorded")
        .clone();
    let serialized = serde_json::to_value(recorded).expect("request should serialize");
    let instant = chrono::DateTime::parse_from_rfc3339("2026-08-03T12:30:15Z")
        .expect("test timestamp must be valid");
    assert_eq!(
        serialized["fields"][0]["defaultValue"],
        instant
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d")
            .to_string()
    );
    assert_eq!(
        serialized["fields"][1]["defaultValue"],
        instant
            .with_timezone(&chrono::Local)
            .format("%H:%M:%S")
            .to_string()
    );
    assert_eq!(
        serialized["fields"][2]["defaultValue"],
        "2026-08-03T15:30:15"
    );
}

#[test]
fn form_dialog_rejects_invalid_typed_datetime_defaults_before_opening() {
    let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse::button(
        "cancel",
    )));
    let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());
    let request = form_dialog_request(serde_json::json!([
        {"type":"datetime","key":"dateTime","label":"Date time","description":"","required":false,"defaultValue":{"type":"datetime","value":"not-a-date"},"timezone":"UTC"}
    ]));

    let error = adapter
        .form_dialog(&request, &dialog_context())
        .expect_err("invalid typed datetime defaults must be rejected");

    assert!(error.to_string().contains("invalid RFC 3339 value"));
    assert!(
        provider
            .request
            .lock()
            .expect("request lock should not be poisoned")
            .is_none()
    );
}

#[test]
fn form_dialog_rejects_invalid_information_accent_color_before_opening() {
    let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse {
        button: "ok".to_owned(),
        values: Map::new(),
    }));
    let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());
    let request = form_dialog_request(serde_json::json!([
        {"type":"information","label":"Deployment","description":"","accentColor":"not-a-color"}
    ]));

    let error = adapter
        .form_dialog(&request, &dialog_context())
        .expect_err("invalid information accent color must be rejected");

    assert!(error.to_string().contains("accent color must be"));
    assert!(
        provider
            .request
            .lock()
            .expect("request lock should not be poisoned")
            .is_none()
    );
}

#[test]
fn form_dialog_image_requires_an_installed_package_context() {
    let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse {
        button: "ok".to_owned(),
        values: Map::new(),
    }));
    let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());
    let request = form_dialog_request(serde_json::json!([
        {
            "type":"image",
            "label":"Deployment diagram",
            "description":"",
            "assetPath":"assets/deployment.png",
            "imageFit":"contain",
            "imageHeight":240
        }
    ]));

    let error = adapter
        .form_dialog(&request, &dialog_context())
        .expect_err("an image without an installed package must be rejected");

    assert!(error.to_string().contains(
        "failed to read form image asset \"assets/deployment.png\": an installed package context is required"
    ));
    assert!(
        provider
            .request
            .lock()
            .expect("request lock should not be poisoned")
            .is_none()
    );
}

#[test]
fn form_dialog_preserves_each_supported_window_size() {
    for (configured, expected) in [
        ("small", DesktopDialogSize::Small),
        ("medium", DesktopDialogSize::Medium),
        ("large", DesktopDialogSize::Large),
    ] {
        let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse {
            button: "ok".to_owned(),
            values: Map::new(),
        }));
        let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());
        let mut request = form_dialog_request(serde_json::json!([
            {"type":"information","label":"Information","description":"","accentColor":"#5B8AF5"}
        ]));
        request.config.insert(
            "dialogSize".to_owned(),
            Value::String(configured.to_owned()),
        );

        adapter
            .form_dialog(&request, &dialog_context())
            .expect("supported window size should open the dialog");

        let recorded = provider
            .request
            .lock()
            .expect("request lock should not be poisoned");
        let Some(DesktopDialogRequest {
            content: DesktopDialogContent::FormDialog { dialog_size, .. },
            ..
        }) = recorded.as_ref()
        else {
            panic!("form dialog request should be recorded");
        };
        assert_eq!(*dialog_size, expected);
    }
}

#[test]
fn form_dialog_rejects_an_empty_component_list_before_opening() {
    let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse::button(
        "cancel",
    )));
    let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());

    let error = adapter
        .form_dialog(
            &form_dialog_request(serde_json::json!([])),
            &dialog_context(),
        )
        .expect_err("an empty form must be rejected");

    assert!(
        error
            .to_string()
            .contains("requires at least one component")
    );
    assert!(
        provider
            .request
            .lock()
            .expect("request lock should not be poisoned")
            .is_none()
    );
}

#[test]
fn message_dialog_preserves_each_supported_window_size() {
    for (configured, expected) in [
        ("small", DesktopDialogSize::Small),
        ("medium", DesktopDialogSize::Medium),
        ("large", DesktopDialogSize::Large),
    ] {
        let provider = std::sync::Arc::new(FixedDialogProvider::new(
            DesktopDialogResponse::button("ok"),
        ));
        let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());
        let request = RuntimeActionRequest {
            action: Some("show_message_box".to_owned()),
            action_type: "action.message_box".to_owned(),
            config: Map::from_iter([
                ("type".to_owned(), Value::String("info".to_owned())),
                ("buttons".to_owned(), Value::String("ok".to_owned())),
                ("title".to_owned(), Value::String("Message".to_owned())),
                ("message".to_owned(), Value::String("Body".to_owned())),
                (
                    "dialogSize".to_owned(),
                    Value::String(configured.to_owned()),
                ),
            ]),
            node_id: "n-message".to_owned(),
        };

        adapter
            .message_box(&request, &dialog_context())
            .expect("supported window size should open the message dialog");

        let recorded = provider
            .request
            .lock()
            .expect("request lock should not be poisoned");
        let Some(DesktopDialogRequest {
            content: DesktopDialogContent::MessageDialog { dialog_size, .. },
            ..
        }) = recorded.as_ref()
        else {
            panic!("message dialog request should be recorded");
        };
        assert_eq!(*dialog_size, expected);
    }
}

#[test]
fn form_dialog_rejects_unsupported_window_size_before_opening() {
    let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse {
        button: "ok".to_owned(),
        values: Map::new(),
    }));
    let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());
    let mut request = form_dialog_request(serde_json::json!([]));
    request.config.insert(
        "dialogSize".to_owned(),
        Value::String("fullscreen".to_owned()),
    );

    let error = adapter
        .form_dialog(&request, &dialog_context())
        .expect_err("unsupported window size must be rejected");

    assert!(
        error
            .to_string()
            .contains("unsupported form dialog window size")
    );
    assert!(
        provider
            .request
            .lock()
            .expect("request lock should not be poisoned")
            .is_none()
    );
}

#[test]
fn password_components_mark_only_their_nested_outputs_as_sensitive() {
    let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse {
        button: "ok".to_owned(),
        values: serde_json::json!({"password":"correct horse battery staple","username":"Ada"})
            .as_object()
            .expect("test response must be an object")
            .clone(),
    }));
    let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider);
    let request = form_dialog_request(serde_json::json!([
        {"type":"text","key":"username","label":"Username","description":"","required":true,"placeholder":"","defaultValue":""},
        {"type":"password","key":"password","label":"Password","description":"","required":true,"placeholder":""}
    ]));

    let result = adapter
        .form_dialog(&request, &dialog_context())
        .expect("password component should succeed");

    assert_eq!(result.output_data["values"]["username"], "Ada");
    assert_eq!(
        result.output_data["values"]["password"],
        "correct horse battery staple"
    );
    assert_eq!(
        result.sensitive_output_keys.iter().collect::<Vec<_>>(),
        vec!["values.password"]
    );
}

#[test]
fn form_dialog_rejects_duplicate_component_and_choice_keys_before_opening_a_window() {
    for (fields, expected) in [
        (
            serde_json::json!([
                {"type":"text","key":"same","label":"First","description":"","required":false,"placeholder":"","defaultValue":""},
                {"type":"checkbox","key":"same","label":"Second","description":"","required":false,"defaultChecked":false}
            ]),
            "form component keys must be unique",
        ),
        (
            serde_json::json!([
                {"type":"single_choice","key":"choice","label":"Choice","description":"","required":false,"choices":[{"key":"same","displayValue":"First"},{"key":"same","displayValue":"Second"}]}
            ]),
            "choice keys must be unique",
        ),
    ] {
        let provider = std::sync::Arc::new(FixedDialogProvider::new(
            DesktopDialogResponse::button("ok"),
        ));
        let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());
        let error = adapter
            .form_dialog(&form_dialog_request(fields), &dialog_context())
            .expect_err("duplicate keys must fail before the window opens");
        assert!(error.to_string().contains(expected));
        assert!(
            provider
                .request
                .lock()
                .expect("request lock should not be poisoned")
                .is_none()
        );
    }
}

#[test]
fn form_dialog_rejects_non_portable_component_and_choice_keys_before_opening_a_window() {
    for (fields, expected) in [
        (
            serde_json::json!([
                {"type":"text","key":"invalid.key","label":"Value","description":"","required":false,"placeholder":"","defaultValue":""}
            ]),
            "form component 1 key may contain only letters A-Z, a-z, numbers 0-9, hyphens, and underscores",
        ),
        (
            serde_json::json!([
                {"type":"single_choice","key":"choice","label":"Choice","description":"","required":false,"choices":[{"key":"invalid key","displayValue":"Invalid"}]}
            ]),
            "form component 1, choice 1 key may contain only letters A-Z, a-z, numbers 0-9, hyphens, and underscores",
        ),
    ] {
        let provider = std::sync::Arc::new(FixedDialogProvider::new(
            DesktopDialogResponse::button("ok"),
        ));
        let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider.clone());
        let error = adapter
            .form_dialog(&form_dialog_request(fields), &dialog_context())
            .expect_err("non-portable keys must fail before the window opens");
        assert!(error.to_string().contains(expected));
        assert!(
            provider
                .request
                .lock()
                .expect("request lock should not be poisoned")
                .is_none()
        );
    }
}

#[test]
fn dialog_response_deserialization_rejects_unknown_fields() {
    let error = serde_json::from_value::<DesktopDialogResponse>(serde_json::json!({
        "button": "ok",
        "values": {},
        "unexpected": true
    }))
    .expect_err("unknown renderer response fields must be rejected");

    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn form_dialog_rejects_unknown_or_invalid_renderer_values() {
    for (values, expected) in [
        (
            serde_json::json!({"choice":"not-configured"}),
            "unknown choice",
        ),
        (serde_json::json!({"choice":42}), "must be text"),
        (
            serde_json::json!({"choice":"configured","invented":true}),
            "unknown field",
        ),
    ] {
        let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse {
            button: "ok".to_owned(),
            values: values
                .as_object()
                .expect("test response must be an object")
                .clone(),
        }));
        let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider);
        let request = form_dialog_request(serde_json::json!([
            {"type":"single_choice","key":"choice","label":"Choice","description":"","required":true,"choices":[{"key":"configured","displayValue":"Configured"}]}
        ]));
        let error = adapter
            .form_dialog(&request, &dialog_context())
            .expect_err("invalid renderer values must be rejected");
        assert!(error.to_string().contains(expected));
    }
}

#[test]
fn cancelled_form_dialog_rejects_renderer_data() {
    let provider = std::sync::Arc::new(FixedDialogProvider::new(DesktopDialogResponse {
        button: "cancel".to_owned(),
        values: serde_json::json!({"password":"must-not-escape"})
            .as_object()
            .expect("test response must be an object")
            .clone(),
    }));
    let adapter = SystemDesktopActionAdapter::with_dialog_provider(provider);
    let request = form_dialog_request(serde_json::json!([
        {"type":"password","key":"password","label":"Password","description":"","required":false,"placeholder":""}
    ]));

    let error = adapter
        .form_dialog(&request, &dialog_context())
        .expect_err("cancelled form dialogs must never return renderer-supplied data");

    assert!(error.to_string().contains("unexpected data"));
}

fn form_dialog_request(fields: Value) -> RuntimeActionRequest {
    RuntimeActionRequest {
        action: Some("form_dialog".to_owned()),
        action_type: "action.form_dialog".to_owned(),
        config: Map::from_iter([
            ("title".to_owned(), Value::String("Form Dialog".to_owned())),
            (
                "description".to_owned(),
                Value::String("Description".to_owned()),
            ),
            ("dialogSize".to_owned(), Value::String("medium".to_owned())),
            ("fields".to_owned(), fields),
        ]),
        node_id: "n-form-dialog".to_owned(),
    }
}

#[test]
fn builds_pixel_color_metadata() {
    let output = pixel_color_map(-12, 34, 171, 32, 48, 255);

    assert_eq!(
        output.get("hex"),
        Some(&Value::String("#ab2030".to_owned()))
    );
    assert_eq!(output.get("red"), Some(&Value::Number(Number::from(171))));
    assert_eq!(output.get("green"), Some(&Value::Number(Number::from(32))));
    assert_eq!(output.get("blue"), Some(&Value::Number(Number::from(48))));
    assert_eq!(output.get("alpha"), Some(&Value::Number(Number::from(255))));
    assert_eq!(
        output.get("integer"),
        Some(&Value::Number(Number::from(0xab_20_30_u32)))
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
    let adapter = SystemDesktopActionAdapter::default();
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
        package_bytes: None,
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
    let adapter = SystemDesktopActionAdapter::default();
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
        package_bytes: None,
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
