use serde_json::{Value, json};

use crate::execute_manual_program;

#[test]
fn set_coerces_exported_json_container_strings() {
    let report = execute(
        vec![
            variable_node("n-list", "items", "set", "list", json!(r#"["one",2]"#)),
            variable_node(
                "n-object",
                "payload",
                "set",
                "object",
                json!(r#"{"status":"ok"}"#),
            ),
        ],
        linear_edges(&["n-list", "n-object"]),
    )
    .expect("JSON container values should be parsed");

    assert_eq!(report.variables.get("items"), Some(&json!(["one", 2])));
    assert_eq!(
        report.variables.get("payload"),
        Some(&json!({"status": "ok"}))
    );
}

#[test]
fn set_and_increment_resolve_variable_references() {
    let report = execute(
        vec![
            variable_node("n-source", "source", "set", "number", json!(2)),
            variable_node("n-target", "target", "set", "number", json!("{{source}}")),
            variable_node(
                "n-increment",
                "target",
                "increment",
                "number",
                json!("{{source}}"),
            ),
        ],
        linear_edges(&["n-source", "n-target", "n-increment"]),
    )
    .expect("variable references should resolve before coercion");

    assert_eq!(report.variables.get("target"), Some(&json!(4.0)));
}

#[test]
fn append_list_preserves_json_compatible_items() {
    let report = execute(
        vec![
            variable_node("n-list", "items", "set", "list", json!("[]")),
            variable_node(
                "n-append",
                "items",
                "append_list",
                "list",
                json!(r#"{"id":7}"#),
            ),
        ],
        linear_edges(&["n-list", "n-append"]),
    )
    .expect("JSON object should append as an object rather than a string");

    assert_eq!(report.variables.get("items"), Some(&json!([{"id": 7}])));
}

#[test]
fn set_object_field_supports_dot_fields_and_numeric_indexes() {
    let mut field_node = variable_node(
        "n-field",
        "payload",
        "set_object_field",
        "object",
        json!(r#"{"name":"Ada"}"#),
    );
    field_node["config"]["fieldPath"] = json!("users[0].profile");

    let report = execute(vec![field_node], linear_edges(&["n-field"]))
        .expect("valid nested object path should execute");

    assert_eq!(
        report.variables.get("payload"),
        Some(&json!({"users": [{"profile": {"name": "Ada"}}]}))
    );
}

#[test]
fn clear_uses_the_editor_default_for_every_variable_type() {
    let types = [
        ("string", json!("")),
        ("number", json!(0)),
        ("boolean", json!(false)),
        ("list", json!([])),
        ("object", json!({})),
        (
            "duration",
            json!({"type": "duration", "unit": "seconds", "value": 0}),
        ),
        (
            "datetime",
            json!({"type": "datetime", "value": "1970-01-01T00:00:00.000Z"}),
        ),
        (
            "http_response",
            json!({"type": "http_response", "status": 0, "headers": {}, "body": ""}),
        ),
        ("file_path", json!("")),
    ];
    let steps = types
        .iter()
        .enumerate()
        .map(|(index, (value_type, _))| {
            variable_node(
                &format!("n-clear-{index}"),
                &format!("value_{index}"),
                "clear",
                value_type,
                Value::Null,
            )
        })
        .collect::<Vec<_>>();
    let ids = (0..types.len())
        .map(|index| format!("n-clear-{index}"))
        .collect::<Vec<_>>();
    let id_refs = ids.iter().map(String::as_str).collect::<Vec<_>>();

    let report = execute(steps, linear_edges(&id_refs)).expect("clear operations should execute");

    for (index, (_, expected)) in types.iter().enumerate() {
        assert_eq!(
            report.variables.get(&format!("value_{index}")),
            Some(expected),
            "unexpected clear value for {}",
            types[index].0
        );
    }
}

#[test]
fn exposes_complete_derived_metadata_with_javascript_string_lengths() {
    let report = execute(
        vec![
            variable_node("n-text", "text", "set", "string", json!("A😀")),
            variable_node("n-list", "items", "set", "list", json!("[]")),
            variable_node(
                "n-object",
                "payload",
                "set",
                "object",
                json!(r#"{"one":1,"two":2}"#),
            ),
            variable_node("n-number", "count", "set", "number", json!(4)),
        ],
        linear_edges(&["n-text", "n-list", "n-object", "n-number"]),
    )
    .expect("derived metadata should be generated");

    assert_metadata(&report.variables, "text", 3, "string", false);
    assert_metadata(&report.variables, "items", 0, "list", true);
    assert_metadata(&report.variables, "payload", 2, "object", false);
    assert_metadata(&report.variables, "count", 0, "number", false);
}

#[test]
fn invalid_increment_and_object_paths_fail_closed() {
    let increment_error = execute(
        vec![variable_node(
            "n-increment",
            "count",
            "increment",
            "number",
            json!("not-a-number"),
        )],
        linear_edges(&["n-increment"]),
    )
    .expect_err("invalid increment must not silently become zero or one");
    assert!(increment_error.to_string().contains("finite number"));

    for path in ["users[01].name", "users.", "users[name]", "users..name"] {
        let mut field_node = variable_node(
            "n-field",
            "payload",
            "set_object_field",
            "object",
            json!("value"),
        );
        field_node["config"]["fieldPath"] = json!(path);
        let error = execute(vec![field_node], linear_edges(&["n-field"]))
            .expect_err("invalid object path must fail");
        assert!(
            error.to_string().contains("invalid object field path"),
            "unexpected error for {path:?}: {error}"
        );
    }
}

#[test]
fn all_derived_metadata_names_are_read_only() {
    for suffix in ["$length", "$count", "$type", "$is_empty"] {
        let error = execute(
            vec![variable_node(
                "n-write",
                &format!("value.{suffix}"),
                "set",
                "string",
                json!("bad"),
            )],
            linear_edges(&["n-write"]),
        )
        .expect_err("derived metadata must not be writable");
        assert!(error.to_string().contains("read-only or reserved"));
    }
}

fn execute(steps: Vec<Value>, edges: Vec<Value>) -> Result<crate::RunReport, crate::RuntimeError> {
    execute_manual_program(
        &json!({
            "entry": {
                "trigger": {
                    "id": "n-trigger",
                    "action_type": "trigger.manual",
                    "type": "manual",
                    "config": {},
                    "runtime_outputs": []
                },
                "triggers": [],
                "program": {"steps": steps, "edges": edges}
            }
        }),
        "variable-operations",
    )
}

fn linear_edges(node_ids: &[&str]) -> Vec<Value> {
    let mut edges = Vec::with_capacity(node_ids.len());
    let mut source = "n-trigger";
    for node_id in node_ids {
        edges.push(json!({
            "execution_order": 0,
            "source": source,
            "source_handle": "out",
            "target": node_id,
            "target_handle": "input"
        }));
        source = node_id;
    }
    edges
}

fn variable_node(id: &str, name: &str, operation: &str, value_type: &str, value: Value) -> Value {
    json!({
        "id": id,
        "action_type": "runtime.set_variable",
        "type": "set_variable",
        "config": {
            "name": name,
            "operation": operation,
            "scope": "runtime",
            "valueType": value_type,
            "value": value
        },
        "runtime_outputs": []
    })
}

fn assert_metadata(
    variables: &std::collections::BTreeMap<String, Value>,
    name: &str,
    length: u64,
    value_type: &str,
    is_empty: bool,
) {
    assert_eq!(
        variables.get(&format!("{name}.$length")),
        Some(&json!(length))
    );
    assert_eq!(
        variables.get(&format!("{name}.$count")),
        Some(&json!(length))
    );
    assert_eq!(
        variables.get(&format!("{name}.$type")),
        Some(&json!(value_type))
    );
    assert_eq!(
        variables.get(&format!("{name}.$is_empty")),
        Some(&json!(is_empty))
    );
}
