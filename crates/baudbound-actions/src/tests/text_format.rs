use baudbound_runtime::ResourceLimit;
use serde_json::{Value, json};

use super::{execute, execute_with_handler};
use crate::{ActionLimits, HeadlessActionHandler};

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
        (
            json!(90_061),
            json!({"id":"1","operation":"format_duration","durationUnit":"seconds","pattern":"D HH:mm:ss"}),
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
fn generated_text_limit_is_enforced_without_hidden_clamping() {
    let handler = HeadlessActionHandler::default().with_limits(ActionLimits {
        max_generated_text_bytes: ResourceLimit::limited(4),
        ..ActionLimits::default()
    });
    let error = execute_with_handler(
        &handler,
        "action.text.format",
        pipeline(
            json!("x"),
            json!([{"id":"1","operation":"pad_end","targetLength":5,"pad":"y"}]),
        ),
        Value::Null,
    )
    .expect_err("generated text over the configured byte limit must fail");

    assert!(error.to_string().contains("configured 4 byte limit"));

    let unlimited = HeadlessActionHandler::default().with_limits(ActionLimits {
        max_generated_text_bytes: ResourceLimit::Unlimited,
        ..ActionLimits::default()
    });
    let result = execute_with_handler(
        &unlimited,
        "action.text.format",
        pipeline(
            json!("x"),
            json!([{"id":"1","operation":"pad_end","targetLength":5,"pad":"y"}]),
        ),
        Value::Null,
    )
    .expect("unlimited generated text should not be clamped");
    assert_eq!(result.output_data.get("text"), Some(&json!("xyyyy")));
}

#[test]
fn multi_megabyte_text_generation_obeys_the_exact_configured_boundary() {
    const MAXIMUM: usize = 4 * 1024 * 1024;

    let handler = HeadlessActionHandler::default().with_limits(ActionLimits {
        max_generated_text_bytes: ResourceLimit::limited(MAXIMUM as u64),
        ..ActionLimits::default()
    });
    let result = execute_with_handler(
        &handler,
        "action.text.format",
        pipeline(
            json!("x"),
            json!([{"id":"1","operation":"pad_end","targetLength":MAXIMUM,"pad":"y"}]),
        ),
        Value::Null,
    )
    .expect("text exactly at the configured boundary should succeed");
    assert_eq!(result.output_data["text"].as_str().unwrap().len(), MAXIMUM);

    let error = execute_with_handler(
        &handler,
        "action.text.format",
        pipeline(
            json!("x"),
            json!([{"id":"1","operation":"pad_end","targetLength":MAXIMUM + 1,"pad":"y"}]),
        ),
        Value::Null,
    )
    .expect_err("text one byte beyond the configured boundary must fail");
    assert!(error.to_string().contains("4194304 byte limit"));
}

#[test]
fn shared_regex_replacement_fixtures_conform() {
    let fixtures: Value =
        serde_json::from_str(include_str!("../../../../contracts/regex-conformance.json"))
            .expect("shared regex fixtures must be valid JSON");
    let cases = fixtures
        .get("replacement_cases")
        .and_then(Value::as_array)
        .expect("shared regex replacement cases must be an array");

    for fixture in cases {
        let name = fixture["name"].as_str().expect("fixture name must be text");
        let result = execute(
            "action.text.format",
            pipeline(
                fixture["input"].clone(),
                json!([{
                    "id": name,
                    "operation": "regex_replace",
                    "search": fixture["pattern"],
                    "replacement": fixture["replacement"]
                }]),
            ),
        )
        .unwrap_or_else(|error| panic!("{name} failed: {error}"));
        assert_eq!(
            result.output_data.get("text"),
            Some(&fixture["output"]),
            "{name}"
        );
    }
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

#[test]
fn formats_a_datetime_from_the_pipeline_input() {
    let result = execute(
        "action.text.format",
        pipeline(
            json!({ "type": "datetime", "value": "2026-07-03T14:30:45+03:00" }),
            json!([{"id": "1", "operation": "format_datetime", "pattern": "EEEE 'at' HH:mm"}]),
        ),
    )
    .expect("a datetime input should format");

    assert_eq!(
        result.output_data.get("text"),
        Some(&json!("Friday at 14:30"))
    );
}

#[test]
fn rejects_a_format_datetime_operation_that_cannot_run() {
    let cases = [
        // Text reaches the operation as text, not as a datetime.
        pipeline(
            json!("2026-07-03T14:30:45+03:00"),
            json!([{"id": "1", "operation": "format_datetime", "pattern": "yyyy"}]),
        ),
        // A mistyped token is refused rather than written out as itself.
        pipeline(
            json!({ "type": "datetime", "value": "2026-07-03T14:30:45+03:00" }),
            json!([{"id": "1", "operation": "format_datetime", "pattern": "YYYY"}]),
        ),
        pipeline(
            json!({ "type": "datetime", "value": "2026-07-03T14:30:45+03:00" }),
            json!([{"id": "1", "operation": "format_datetime", "pattern": ""}]),
        ),
    ];

    for config in cases {
        execute("action.text.format", config).expect_err("the operation must fail");
    }
}

#[test]
fn formats_duration_from_integer_and_float_pipeline_inputs() {
    let integer = execute(
        "action.text.format",
        pipeline(
            json!(90_061),
            json!([{"id": "1", "operation": "format_duration", "durationUnit": "seconds", "pattern": "D HH:mm:ss"}]),
        ),
    )
    .expect("integer duration should format");
    assert_eq!(integer.output_data.get("text"), Some(&json!("1 01:01:01")));

    let float = execute(
        "action.text.format",
        pipeline(
            json!(1.2345),
            json!([{"id": "1", "operation": "format_duration", "durationUnit": "seconds", "pattern": "ss.SSS"}]),
        ),
    )
    .expect("float duration should format");
    assert_eq!(float.output_data.get("text"), Some(&json!("01.235")));
}

#[test]
fn rejects_a_format_duration_operation_that_cannot_run() {
    let cases = [
        pipeline(
            json!("not a duration"),
            json!([{"id": "1", "operation": "format_duration", "durationUnit": "seconds", "pattern": "HH:mm:ss"}]),
        ),
        pipeline(
            json!(1),
            json!([{"id": "1", "operation": "format_duration", "durationUnit": "weeks", "pattern": "HH:mm:ss"}]),
        ),
        pipeline(
            json!(1),
            json!([{"id": "1", "operation": "format_duration", "durationUnit": "seconds", "pattern": "YYYY"}]),
        ),
    ];

    for config in cases {
        execute("action.text.format", config).expect_err("the operation must fail");
    }
}
