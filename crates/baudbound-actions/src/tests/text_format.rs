use serde_json::{Value, json};

use super::execute;

#[test]
fn executes_every_editor_text_transform_operation() {
    let cases = [
        (
            json!({"operation": "template", "template": "Hello Ada"}),
            json!("Hello Ada"),
            json!([]),
        ),
        (
            json!({"operation": "trim", "input": "  text \n"}),
            json!("text"),
            json!([]),
        ),
        (
            json!({"operation": "uppercase", "input": "BaudBound"}),
            json!("BAUDBOUND"),
            json!([]),
        ),
        (
            json!({"operation": "lowercase", "input": "BaudBound"}),
            json!("baudbound"),
            json!([]),
        ),
        (
            json!({"operation": "sentence_case", "input": "hELLO WORLD"}),
            json!("Hello world"),
            json!([]),
        ),
        (
            json!({"operation": "capitalize_words", "input": "hELLO   wORLD"}),
            json!("Hello   World"),
            json!([]),
        ),
        (
            json!({"operation": "replace", "input": "one one", "search": "one", "replacement": "two"}),
            json!("two two"),
            json!([]),
        ),
        (
            json!({"operation": "regex_replace", "input": "a1 b22", "search": "\\d+", "replacement": "#"}),
            json!("a# b#"),
            json!([]),
        ),
        (
            json!({"operation": "split", "input": "one,two,three", "delimiter": ","}),
            json!(""),
            json!(["one", "two", "three"]),
        ),
        (
            json!({"operation": "join", "items": ["one", 2, true, {"ok": true}], "delimiter": "|"}),
            json!("one|2|true|{\"ok\":true}"),
            json!(["one", 2, true, {"ok": true}]),
        ),
        (
            json!({"operation": "substring", "input": "BaudBound", "start": 4, "length": 5}),
            json!("Bound"),
            json!([]),
        ),
        (
            json!({"operation": "pad_start", "input": "7", "targetLength": 3, "pad": "0"}),
            json!("007"),
            json!([]),
        ),
        (
            json!({"operation": "pad_end", "input": "7", "targetLength": 3, "pad": "0"}),
            json!("700"),
            json!([]),
        ),
        (
            json!({"operation": "url_encode", "input": "a b&!'()*~"}),
            json!("a%20b%26!'()*~"),
            json!([]),
        ),
        (
            json!({"operation": "url_decode", "input": "a%20b%26!'()*~"}),
            json!("a b&!'()*~"),
            json!([]),
        ),
        (
            json!({"operation": "base64_encode", "input": "BaudBound ✓"}),
            json!("QmF1ZEJvdW5kIOKckw=="),
            json!([]),
        ),
        (
            json!({"operation": "base64_decode", "input": "QmF1ZEJvdW5kIOKckw=="}),
            json!("BaudBound ✓"),
            json!([]),
        ),
        (
            json!({"operation": "json_escape", "input": "line\n\"quoted\""}),
            json!("\"line\\n\\\"quoted\\\"\""),
            json!([]),
        ),
        (
            json!({"operation": "json_unescape", "input": "null"}),
            json!("null"),
            json!([]),
        ),
    ];

    for (config, expected_text, expected_items) in cases {
        let operation = config["operation"]
            .as_str()
            .expect("operation should be text")
            .to_owned();
        let result = execute("action.text.format", config)
            .unwrap_or_else(|error| panic!("{operation} should succeed: {error}"));
        assert_eq!(
            result.output_data.get("text"),
            Some(&expected_text),
            "unexpected text output for {operation}"
        );
        assert_eq!(
            result.output_data.get("items"),
            Some(&expected_items),
            "unexpected item output for {operation}"
        );
    }
}

#[test]
fn parses_join_items_from_an_exported_json_string() {
    let result = execute(
        "action.text.format",
        json!({
            "operation": "join",
            "items": "[\"one\",2]",
            "delimiter": ":"
        }),
    )
    .expect("exported JSON list should be accepted");

    assert_eq!(result.output_data.get("text"), Some(&json!("one:2")));
    assert_eq!(result.output_data.get("items"), Some(&json!(["one", 2])));
}

#[test]
fn rejects_malformed_text_transform_inputs() {
    let cases = [
        json!({"operation": "regex_replace", "input": "text", "search": "[", "replacement": ""}),
        json!({"operation": "join", "items": "{}", "delimiter": ","}),
        json!({"operation": "base64_decode", "input": "%%%"}),
        json!({"operation": "base64_decode", "input": "YQ"}),
        json!({"operation": "base64_decode", "input": "Y Q=="}),
        json!({"operation": "base64_decode", "input": "/w=="}),
        json!({"operation": "url_decode", "input": "%ZZ"}),
        json!({"operation": "json_unescape", "input": "not-json"}),
        json!({"operation": "unsupported", "input": "text"}),
    ];

    for config in cases {
        let operation = config["operation"]
            .as_str()
            .expect("operation should be text")
            .to_owned();
        let error =
            execute("action.text.format", config).expect_err("malformed transform input must fail");
        assert!(
            !error.to_string().trim().is_empty(),
            "{operation} should return an actionable error"
        );
    }
}

#[test]
fn json_unescape_serializes_non_string_values_like_the_editor() {
    let cases = [
        ("true", json!("true")),
        ("42", json!("42")),
        ("[1,2]", json!("[1,2]")),
        (r#"{"ok":true}"#, json!(r#"{"ok":true}"#)),
        (r#""plain""#, json!("plain")),
    ];

    for (input, expected) in cases {
        let result = execute(
            "action.text.format",
            json!({"operation": "json_unescape", "input": input}),
        )
        .expect("valid JSON should unescape");
        assert_eq!(result.output_data.get("text"), Some(&expected));
        assert_eq!(
            result.output_data.get("items"),
            Some(&Value::Array(Vec::new()))
        );
    }
}

#[test]
fn substring_and_padding_count_unicode_code_points() {
    let substring = execute(
        "action.text.format",
        json!({"operation": "substring", "input": "A😀BC", "start": 1, "length": 2}),
    )
    .expect("Unicode substring should succeed");
    assert_eq!(substring.output_data.get("text"), Some(&json!("😀B")));

    let padded = execute(
        "action.text.format",
        json!({"operation": "pad_start", "input": "😀", "targetLength": 3, "pad": "ab"}),
    )
    .expect("Unicode padding should succeed");
    assert_eq!(padded.output_data.get("text"), Some(&json!("ab😀")));
}
