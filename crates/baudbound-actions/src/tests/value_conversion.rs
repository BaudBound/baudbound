use serde_json::json;

use super::execute;

#[test]
fn converts_supported_value_types() {
    let cases = [
        (json!("42.5"), "float", json!(42.5)),
        (json!("42"), "integer", json!(42)),
        (json!("TRUE"), "boolean", json!(true)),
        (json!("[1,2]"), "list", json!([1, 2])),
        (json!("{\"ok\":true}"), "object", json!({"ok": true})),
        (json!({"ok": true}), "string", json!("{\"ok\":true}")),
    ];

    for (value, target_type, expected) in cases {
        let result = execute(
            "action.value.convert",
            json!({"value": value, "targetType": target_type}),
        )
        .expect("supported conversion should succeed");
        assert_eq!(result.output_data.get("value"), Some(&expected));
        assert_eq!(
            result.output_data.get("target_type"),
            Some(&json!(target_type))
        );
    }
}

#[test]
fn rejects_lossy_or_invalid_conversions() {
    let cases = [
        json!({"value": "1.5", "targetType": "integer"}),
        json!({"value": "9007199254740992", "targetType": "integer"}),
        json!({"value": "yes", "targetType": "boolean"}),
        json!({"value": "{}", "targetType": "list"}),
        json!({"value": "[]", "targetType": "object"}),
        json!({"value": "0x10", "targetType": "float"}),
        json!({"value": "0b10", "targetType": "integer"}),
    ];

    for config in cases {
        execute("action.value.convert", config).expect_err("invalid conversion must fail");
    }
}

#[test]
fn convert_value_rejects_the_removed_text_target() {
    let error = execute(
        "action.value.convert",
        json!({ "value": "hello", "targetType": "text" }),
    )
    .expect_err("the text target was renamed to string");

    assert!(error.to_string().contains("text"), "{error}");
}

#[test]
fn convert_value_uses_the_shared_conversion() {
    let result = execute(
        "action.value.convert",
        json!({ "value": 42, "targetType": "float" }),
    )
    .expect("integer converts to float");

    assert_eq!(output(&result, "value"), serde_json::json!(42.0));
}

fn output(result: &baudbound_runtime::RuntimeActionResult, key: &str) -> serde_json::Value {
    result
        .output_data
        .get(key)
        .cloned()
        .unwrap_or(serde_json::Value::Null)
}
