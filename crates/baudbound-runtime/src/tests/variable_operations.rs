use serde_json::{Value, json};

use crate::execute_manual_program;

#[test]
fn variable_operations_accept_and_resolve_shared_identifiers() {
    let report = execute(
        vec![
            variable_node(
                "n-source",
                "Release-Channel_2",
                "set",
                "string",
                json!("stable"),
            ),
            variable_node(
                "n-target",
                "0-result",
                "set",
                "string",
                json!("{{Release-Channel_2}}"),
            ),
        ],
        linear_edges(&["n-source", "n-target"]),
    )
    .expect("portable variable identifiers should execute and resolve");

    assert_eq!(report.variables.get("0-result"), Some(&json!("stable")));
}

#[test]
fn set_coerces_exported_json_container_strings() {
    let report = execute(
        vec![
            variable_node("n-list", "items", "set", "list", json!(r#"["one","two"]"#)),
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

    assert_eq!(report.variables.get("items"), Some(&json!(["one", "two"])));
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
fn resolves_references_recursively_inside_json_values() {
    let append = variable_node(
        "n-list",
        "items",
        "append_list",
        "list",
        json!(r#"{"value":"{{source}}"}"#),
    );
    let report = execute(
        vec![
            variable_node(
                "n-source",
                "source",
                "set",
                "string",
                json!("quoted \"value\"\r\n"),
            ),
            variable_node(
                "n-object",
                "payload",
                "set",
                "object",
                json!(r#"{"nested":{"value":"{{source}}"}}"#),
            ),
            append,
        ],
        linear_edges(&["n-source", "n-object", "n-list"]),
    )
    .expect("nested variable references should resolve without corrupting JSON");

    assert_eq!(
        report.variables.get("payload"),
        Some(&json!({"nested": {"value": "quoted \"value\"\r\n"}}))
    );
    assert_eq!(
        report.variables.get("items"),
        Some(&json!([{"value": "quoted \"value\"\r\n"}]))
    );
}

#[test]
fn logs_variable_names_scopes_inputs_and_resulting_values() {
    let report = execute(
        vec![
            variable_node("n-set", "count", "set", "number", json!(2)),
            variable_node("n-increment", "count", "increment", "number", json!(3)),
            variable_node("n-append", "items", "append_list", "list", json!("first")),
            variable_node("n-clear", "count", "clear", "number", Value::Null),
        ],
        linear_edges(&["n-set", "n-increment", "n-append", "n-clear"]),
    )
    .expect("variable operations should execute");

    let messages = report
        .logs
        .iter()
        .map(|log| log.message.as_str())
        .collect::<Vec<_>>();
    assert!(messages.contains(&r#"Set runtime variable "count" to 2.0."#));
    assert!(messages.contains(&r#"Incremented runtime variable "count" by 3. New value: 5.0."#));
    assert!(
        messages.contains(
            &r#"Appended "first" to runtime list variable "items". New value: ["first"]."#
        )
    );
    assert!(messages.contains(&r#"Cleared runtime variable "count". New value: 0."#));
}

#[test]
fn append_list_preserves_json_compatible_items() {
    let append = variable_node(
        "n-append",
        "items",
        "append_list",
        "list",
        json!(r#"{"id":7}"#),
    );
    let report = execute(
        vec![
            variable_node("n-list", "items", "set", "list", json!("[]")),
            append,
        ],
        linear_edges(&["n-list", "n-append"]),
    )
    .expect("JSON object should append as an object rather than a string");

    assert_eq!(report.variables.get("items"), Some(&json!([{"id": 7}])));
}

#[test]
fn supports_toggle_remove_merge_and_delete_operations() {
    let mut remove_first = variable_node(
        "n-remove-first",
        "items",
        "remove_list_items",
        "list",
        json!(2),
    );
    remove_first["config"]["removeMode"] = json!("first");

    let mut remove_all = variable_node(
        "n-remove-all",
        "items",
        "remove_list_items",
        "list",
        json!(2),
    );
    remove_all["config"]["removeMode"] = json!("all");

    let mut list = variable_node("n-list", "items", "set", "list", json!("[1,2,2,3]"));
    list["config"]["itemType"] = json!("number");

    let mut remove_field = variable_node(
        "n-remove-field",
        "payload",
        "remove_object_field",
        "object",
        Value::Null,
    );
    remove_field["config"]["fieldPath"] = json!("user.private.token");

    let merge = variable_node(
        "n-merge",
        "payload",
        "merge_object",
        "object",
        json!(r#"{"status":"ready","count":2}"#),
    );

    let report = execute(
        vec![
            variable_node(
                "n-toggle",
                "enabled",
                "toggle_boolean",
                "boolean",
                Value::Null,
            ),
            list,
            remove_first,
            remove_all,
            variable_node(
                "n-object",
                "payload",
                "set",
                "object",
                json!(r#"{"user":{"name":"Ada","private":{"token":"secret"}},"count":1}"#),
            ),
            remove_field,
            merge,
            variable_node("n-delete", "enabled", "delete", "boolean", Value::Null),
        ],
        linear_edges(&[
            "n-toggle",
            "n-list",
            "n-remove-first",
            "n-remove-all",
            "n-object",
            "n-remove-field",
            "n-merge",
            "n-delete",
        ]),
    )
    .expect("new variable operations should execute");

    assert!(
        !report.variables.contains_key("enabled"),
        "delete did not remove enabled; logs: {:#?}",
        report.logs
    );
    assert!(!report.variables.contains_key("enabled.$type"));
    assert_eq!(report.variables.get("items"), Some(&json!([1, 3])));
    assert_eq!(
        report.variables.get("payload"),
        Some(&json!({
            "user": {"name": "Ada", "private": {}},
            "count": 2,
            "status": "ready"
        }))
    );
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
    field_node["config"]["fieldValueType"] = json!("object");

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
        ("string", json!("value"), json!("")),
        ("number", json!(42), json!(0)),
        ("boolean", json!(true), json!(false)),
        ("list", json!(["value"]), json!([])),
        ("object", json!({"value": true}), json!({})),
        (
            "duration",
            json!({"type": "duration", "unit": "minutes", "value": 5}),
            json!({"type": "duration", "unit": "seconds", "value": 0}),
        ),
        (
            "datetime",
            json!({"type": "datetime", "value": "2026-07-29T00:00:00.000Z"}),
            json!({"type": "datetime", "value": "1970-01-01T00:00:00.000Z"}),
        ),
        ("file_path", json!("input.txt"), json!("")),
    ];
    let steps = types
        .iter()
        .enumerate()
        .flat_map(|(index, (value_type, initial, _))| {
            [
                variable_node(
                    &format!("n-set-{index}"),
                    &format!("value_{index}"),
                    "set",
                    value_type,
                    initial.clone(),
                ),
                variable_node(
                    &format!("n-clear-{index}"),
                    &format!("value_{index}"),
                    "clear",
                    value_type,
                    Value::Null,
                ),
            ]
        })
        .collect::<Vec<_>>();
    let ids = (0..types.len())
        .flat_map(|index| [format!("n-set-{index}"), format!("n-clear-{index}")])
        .collect::<Vec<_>>();
    let id_refs = ids.iter().map(String::as_str).collect::<Vec<_>>();

    let report = execute(steps, linear_edges(&id_refs)).expect("clear operations should execute");

    for (index, (_, _, expected)) in types.iter().enumerate() {
        assert_eq!(
            report.variables.get(&format!("value_{index}")),
            Some(expected),
            "unexpected clear value for {}",
            types[index].0
        );
    }
}

#[test]
fn clear_requires_an_existing_variable_and_append_infers_item_type() {
    let clear_report = execute(
        vec![variable_node(
            "n-clear",
            "missing",
            "clear",
            "string",
            Value::Null,
        )],
        linear_edges(&["n-clear"]),
    )
    .expect("missing clear target should follow the failed path");
    assert!(
        clear_report
            .logs
            .iter()
            .any(|log| log.level == "error" && log.message.contains("requires existing variable"))
    );

    let report = execute(
        vec![
            variable_node("n-first", "items", "append_list", "list", json!(1)),
            variable_node("n-second", "items", "append_list", "list", json!(2)),
        ],
        linear_edges(&["n-first", "n-second"]),
    )
    .expect("append should infer a homogeneous number list");
    assert_eq!(report.variables.get("items"), Some(&json!([1, 2])));

    let mismatch = execute(
        vec![
            variable_node("n-first", "items", "append_list", "list", json!(1)),
            variable_node("n-second", "items", "append_list", "list", json!("two")),
        ],
        linear_edges(&["n-first", "n-second"]),
    )
    .expect("item type mismatch should follow the failed path");
    assert!(
        mismatch
            .logs
            .iter()
            .any(|log| { log.level == "error" && log.message.contains("requires number items") })
    );
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
    let increment_report = execute(
        vec![variable_node(
            "n-increment",
            "count",
            "increment",
            "number",
            json!("not-a-number"),
        )],
        linear_edges(&["n-increment"]),
    )
    .expect("invalid increment should use the failed outcome");
    assert!(
        increment_report
            .logs
            .iter()
            .any(|log| { log.level == "error" && log.message.contains("finite number") })
    );
    assert!(!increment_report.variables.contains_key("count"));

    for path in ["users[01].name", "users.", "users[name]", "users..name"] {
        let mut field_node = variable_node(
            "n-field",
            "payload",
            "set_object_field",
            "object",
            json!("value"),
        );
        field_node["config"]["fieldPath"] = json!(path);
        let report = execute(vec![field_node], linear_edges(&["n-field"]))
            .expect("invalid object path should use the failed outcome");
        assert!(
            report.logs.iter().any(|log| {
                log.level == "error" && log.message.contains("invalid object field path")
            }),
            "unexpected logs for {path:?}: {:#?}",
            report.logs
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

#[test]
fn script_settings_namespace_is_read_only() {
    for name in ["settings", "settings.endpoint"] {
        let error = execute(
            vec![variable_node(
                "n-write",
                name,
                "set",
                "string",
                json!("changed"),
            )],
            linear_edges(&["n-write"]),
        )
        .expect_err("Script Settings must not be writable");
        let message = error.to_string();
        assert!(
            message.contains("read-only Script Settings")
                || message.contains("Script Settings are read-only")
                || message.contains("invalid variable name"),
            "unexpected error for {name:?}: {message}"
        );
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
    let mut node = json!({
        "id": id,
        "action_type": "runtime.set_variable",
        "type": "set_variable",
        "config": {
            "name": name,
            "operation": operation,
            "scope": "runtime",
            "value": value
        },
        "runtime_outputs": []
    });
    if operation == "set" {
        node["config"]["valueType"] = json!(value_type);
    }
    if operation == "set" && value_type == "list" {
        node["config"]["itemType"] = json!("string");
    }
    if operation == "set_object_field" {
        node["config"]["fieldValueType"] = json!("string");
    }
    if operation == "remove_list_items" {
        node["config"]["removeMode"] = json!("all");
    }
    node
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
