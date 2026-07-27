use serde_json::json;

use super::execute;

#[test]
fn converts_supported_value_types() {
    let cases = [
        (json!("42.5"), "number", json!(42.5)),
        (json!("42"), "integer", json!(42)),
        (json!("TRUE"), "boolean", json!(true)),
        (json!("[1,2]"), "list", json!([1, 2])),
        (json!("{\"ok\":true}"), "object", json!({"ok": true})),
        (json!({"ok": true}), "text", json!("{\"ok\":true}")),
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
        json!({"value": "0x10", "targetType": "number"}),
        json!({"value": "0b10", "targetType": "integer"}),
    ];

    for config in cases {
        execute("action.value.convert", config).expect_err("invalid conversion must fail");
    }
}
