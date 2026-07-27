use serde_json::{Value, json};

use super::execute;

fn pipeline(input: Value, operations: Value) -> Value {
    json!({ "input": input, "operations": operations })
}

#[test]
fn executes_text_operations_in_order() {
    let result = execute(
        "action.text.format",
        pipeline(
            json!("  hello WORLD  "),
            json!([
                {"id": "trim", "operation": "trim"},
                {"id": "sentence", "operation": "sentence_case"},
                {"id": "replace", "operation": "replace", "search": "world", "replacement": "BaudBound"}
            ]),
        ),
    )
    .expect("pipeline should succeed");

    assert_eq!(
        result.output_data.get("text"),
        Some(&json!("Hello BaudBound"))
    );
    assert_eq!(result.output_data.get("items"), Some(&json!([])));
}

#[test]
fn split_output_can_feed_join() {
    let result = execute(
        "action.text.format",
        pipeline(
            json!("one,two,three"),
            json!([
                {"id": "split", "operation": "split", "delimiter": ","},
                {"id": "join", "operation": "join", "delimiter": " | "}
            ]),
        ),
    )
    .expect("split and join pipeline should succeed");

    assert_eq!(
        result.output_data.get("text"),
        Some(&json!("one | two | three"))
    );
    assert_eq!(result.output_data.get("items"), Some(&json!([])));
}

#[test]
fn final_split_returns_list_output() {
    let result = execute(
        "action.text.format",
        pipeline(
            json!("one,two"),
            json!([{"id": "split", "operation": "split", "delimiter": ","}]),
        ),
    )
    .expect("split should succeed");

    assert_eq!(result.output_data.get("text"), Some(&json!("")));
    assert_eq!(
        result.output_data.get("items"),
        Some(&json!(["one", "two"]))
    );
}

#[test]
fn executes_every_supported_operation() {
    let cases = [
        (json!(" x "), json!({"id":"1","operation":"trim"})),
        (json!("x"), json!({"id":"1","operation":"uppercase"})),
        (json!("X"), json!({"id":"1","operation":"lowercase"})),
        (
            json!("hELLO WORLD"),
            json!({"id":"1","operation":"sentence_case"}),
        ),
        (
            json!("hELLO wORLD"),
            json!({"id":"1","operation":"capitalize_words"}),
        ),
        (
            json!("one one"),
            json!({"id":"1","operation":"replace","search":"one","replacement":"two"}),
        ),
        (
            json!("a1"),
            json!({"id":"1","operation":"regex_replace","search":"\\d","replacement":"#"}),
        ),
        (
            json!("abc"),
            json!({"id":"1","operation":"substring","start":1,"length":1}),
        ),
        (
            json!("7"),
            json!({"id":"1","operation":"pad_start","targetLength":3,"pad":"0"}),
        ),
        (
            json!("7"),
            json!({"id":"1","operation":"pad_end","targetLength":3,"pad":"0"}),
        ),
        (json!("a b"), json!({"id":"1","operation":"url_encode"})),
        (json!("a%20b"), json!({"id":"1","operation":"url_decode"})),
        (
            json!("BaudBound"),
            json!({"id":"1","operation":"base64_encode"}),
        ),
        (
            json!("QmF1ZEJvdW5k"),
            json!({"id":"1","operation":"base64_decode"}),
        ),
        (json!("quoted"), json!({"id":"1","operation":"json_escape"})),
        (
            json!("\"plain\""),
            json!({"id":"1","operation":"json_unescape"}),
        ),
        (
            json!("ignored"),
            json!({"id":"1","operation":"template","template":"template"}),
        ),
    ];

    for (input, operation) in cases {
        execute("action.text.format", pipeline(input, json!([operation])))
            .expect("supported operation should succeed");
    }
}

#[test]
fn rejects_invalid_pipeline_types_and_values() {
    let cases = [
        pipeline(json!("text"), json!([])),
        pipeline(json!(["one"]), json!([{"id":"1","operation":"trim"}])),
        pipeline(
            json!("text"),
            json!([{"id":"1","operation":"join","delimiter":","}]),
        ),
        pipeline(
            json!("text"),
            json!([{"id":"1","operation":"regex_replace","search":"["}]),
        ),
        pipeline(
            json!("%%%"),
            json!([{"id":"1","operation":"base64_decode"}]),
        ),
        pipeline(json!("%ZZ"), json!([{"id":"1","operation":"url_decode"}])),
        pipeline(
            json!("not-json"),
            json!([{"id":"1","operation":"json_unescape"}]),
        ),
        pipeline(json!("text"), json!([{"id":"1","operation":"unsupported"}])),
        pipeline(
            json!("text"),
            json!([{"id":"1","operation":"replace","search":"","replacement":"x"}]),
        ),
        pipeline(
            json!("text"),
            json!([{"id":"1","operation":"split","delimiter":""}]),
        ),
        pipeline(
            json!("text"),
            json!([{"id":"1","operation":"pad_start","targetLength":3,"pad":""}]),
        ),
        pipeline(
            json!("text"),
            json!([{"id":"1","operation":"regex_replace","search":"(?=t)","replacement":"x"}]),
        ),
        pipeline(
            json!("text"),
            json!([{"id":"1","operation":"regex_replace","search":"(t)","replacement":"$0"}]),
        ),
        pipeline(
            json!("text"),
            json!([{"id":"1","operation":"substring","start":"9007199254740992"}]),
        ),
    ];

    for config in cases {
        execute("action.text.format", config).expect_err("invalid pipeline must fail");
    }
}

#[test]
fn trims_safe_integer_fields_and_supports_numbered_regex_captures() {
    let substring = execute(
        "action.text.format",
        pipeline(
            json!("abcd"),
            json!([{"id":"1","operation":"substring","start":" 1 ","length":" 2 "}]),
        ),
    )
    .expect("whitespace around safe integer fields should be accepted");
    assert_eq!(substring.output_data.get("text"), Some(&json!("bc")));

    let replaced = execute(
        "action.text.format",
        pipeline(
            json!("first:last"),
            json!([{"id":"1","operation":"regex_replace","search":"([^:]+):([^:]+)","replacement":"$2, $1"}]),
        ),
    )
    .expect("numbered capture replacements should be portable");
    assert_eq!(
        replaced.output_data.get("text"),
        Some(&json!("last, first"))
    );
}

#[test]
fn json_unescape_serializes_non_string_values() {
    let result = execute(
        "action.text.format",
        pipeline(
            json!("[1,2]"),
            json!([{"id":"1","operation":"json_unescape"}]),
        ),
    )
    .expect("valid JSON should unescape");
    assert_eq!(result.output_data.get("text"), Some(&json!("[1,2]")));
}
