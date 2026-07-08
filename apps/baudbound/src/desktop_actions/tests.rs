use super::*;
use super::{
    config::required_u32,
    dialogs::message_box_result,
    mouse::{normalize_mouse_button, normalize_mouse_click_type},
    screen::pixel_color_map,
};
use rfd::MessageDialogResult;
use serde_json::{Map, Number, Value};

#[test]
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
fn normalizes_mouse_click_types() {
    assert_eq!(normalize_mouse_click_type("double"), "double");
    assert_eq!(normalize_mouse_click_type("triple"), "triple");
    assert_eq!(normalize_mouse_click_type("unknown"), "single");
}

#[test]
fn maps_message_box_results() {
    assert_eq!(message_box_result(MessageDialogResult::Ok), "ok");
    assert_eq!(message_box_result(MessageDialogResult::Cancel), "cancel");
    assert_eq!(message_box_result(MessageDialogResult::Yes), "yes");
    assert_eq!(message_box_result(MessageDialogResult::No), "no");
    assert_eq!(
        message_box_result(MessageDialogResult::Custom("later".to_owned())),
        "later"
    );
}

#[test]
fn builds_pixel_color_metadata() {
    let output = pixel_color_map(12, 34, 16, 32, 48, 255);

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
}

#[test]
fn rejects_negative_pixel_coordinates() {
    let request = RuntimeActionRequest {
        action: None,
        action_type: "action.pixel.get".to_owned(),
        config: Map::from_iter([("x".to_owned(), Value::String("-1".to_owned()))]),
        node_id: "n-pixel".to_owned(),
    };

    let error =
        required_u32(&request, "x").expect_err("negative pixel coordinate should be rejected");

    assert!(error.to_string().contains("non-negative integer"));
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
