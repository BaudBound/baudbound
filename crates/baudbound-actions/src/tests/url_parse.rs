use serde_json::json;

use super::execute;

#[test]
fn parses_standard_and_custom_absolute_urls() {
    let cases = [
        (
            "https://nat.gg:8443/test?param=value1&tag=one&tag=two#result",
            json!({
                "protocol": "https",
                "host": "nat.gg",
                "port": "8443",
                "path": "/test",
                "query": "param=value1&tag=one&tag=two",
                "query_parameters": [
                    {"name": "param", "value": "value1"},
                    {"name": "tag", "value": "one"},
                    {"name": "tag", "value": "two"}
                ],
                "fragment": "result"
            }),
        ),
        (
            "ptr://command/move?param=value1",
            json!({
                "protocol": "ptr",
                "host": "command",
                "port": "",
                "path": "/move",
                "query": "param=value1",
                "query_parameters": [{"name": "param", "value": "value1"}],
                "fragment": ""
            }),
        ),
    ];

    for (url, expected) in cases {
        let result = execute("action.url.parse", json!({"url": url}))
            .unwrap_or_else(|error| panic!("{url} should parse: {error}"));
        assert_eq!(result.output_data, expected.as_object().unwrap().clone());
    }
}

#[test]
fn decodes_query_pairs_without_losing_order_or_duplicates() {
    let result = execute(
        "action.url.parse",
        json!({"url": "custom://host/path?name=Baud%20Bound&tag=one&tag=two&empty"}),
    )
    .expect("custom URL should parse");

    assert_eq!(
        result.output_data.get("query_parameters"),
        Some(&json!([
            {"name": "name", "value": "Baud Bound"},
            {"name": "tag", "value": "one"},
            {"name": "tag", "value": "two"},
            {"name": "empty", "value": ""}
        ]))
    );
}

#[test]
fn rejects_missing_relative_and_malformed_urls() {
    for config in [
        json!({}),
        json!({"url": ""}),
        json!({"url": "/relative/path?param=value"}),
        json!({"url": "https://[invalid"}),
    ] {
        let error = execute("action.url.parse", config).expect_err("invalid URL should fail");
        assert!(!error.to_string().trim().is_empty());
    }
}
