use std::sync::OnceLock;

use jsonschema::{Registry, Validator};
use serde_json::Value;

use super::PackageLoadError;

include!(concat!(env!("OUT_DIR"), "/embedded_schemas.rs"));

const KNOWN_VALUE_TYPES: &[&str] = &[
    "string",
    "integer",
    "float",
    "boolean",
    "object",
    "list",
    "color",
    "keyboard_key",
    "datetime",
    "duration",
];

pub(super) fn validate_manifest_schema(manifest: &Value) -> Result<(), PackageLoadError> {
    let validator = manifest_validator().map_err(PackageLoadError::SchemaContract)?;
    validate_schema(validator, manifest).map_err(PackageLoadError::ManifestSchema)
}

pub(super) fn validate_program_schema(program: &Value) -> Result<(), PackageLoadError> {
    let validator = program_validator().map_err(PackageLoadError::SchemaContract)?;
    validate_schema(validator, program).map_err(PackageLoadError::ProgramSchema)?;
    validate_runtime_output_types(program)
}

fn validate_runtime_output_types(program: &Value) -> Result<(), PackageLoadError> {
    for node in executable_nodes_with_outputs(program) {
        for output in node {
            let Some(name) = output.get("type").and_then(Value::as_str) else {
                continue;
            };
            if !KNOWN_VALUE_TYPES.contains(&name) {
                return Err(PackageLoadError::UnknownValueType(name.to_owned()));
            }
        }
    }
    Ok(())
}

/// Yields the `runtime_outputs` array of every executable node in the
/// program: the primary trigger, each secondary trigger, and each step.
/// This mirrors the trigger/triggers/steps traversal in `graph.rs`.
fn executable_nodes_with_outputs(program: &Value) -> Vec<&[Value]> {
    let mut nodes = Vec::new();
    let Some(entry) = program.get("entry") else {
        return nodes;
    };

    if let Some(outputs) = entry
        .get("trigger")
        .and_then(|trigger| trigger.get("runtime_outputs"))
        .and_then(Value::as_array)
    {
        nodes.push(outputs.as_slice());
    }

    for trigger in entry
        .get("triggers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(outputs) = trigger.get("runtime_outputs").and_then(Value::as_array) {
            nodes.push(outputs.as_slice());
        }
    }

    for step in entry
        .get("program")
        .and_then(|program| program.get("steps"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(outputs) = step.get("runtime_outputs").and_then(Value::as_array) {
            nodes.push(outputs.as_slice());
        }
    }

    nodes
}

fn validate_schema(validator: &Validator, value: &Value) -> Result<(), String> {
    let errors = validator
        .iter_errors(value)
        .take(20)
        .map(|error| {
            let path = error.instance_path().to_string();
            if path.is_empty() {
                error.to_string()
            } else {
                format!("{path}: {error}")
            }
        })
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn manifest_validator() -> Result<&'static Validator, String> {
    static VALIDATOR: OnceLock<Result<Validator, String>> = OnceLock::new();
    match VALIDATOR.get_or_init(|| build_standalone_validator(MANIFEST_SCHEMA_JSON)) {
        Ok(validator) => Ok(validator),
        Err(message) => Err(message.clone()),
    }
}

fn program_validator() -> Result<&'static Validator, String> {
    static VALIDATOR: OnceLock<Result<Validator, String>> = OnceLock::new();
    match VALIDATOR.get_or_init(build_program_validator) {
        Ok(validator) => Ok(validator),
        Err(message) => Err(message.clone()),
    }
}

fn build_program_validator() -> Result<Validator, String> {
    let root =
        serde_json::from_str::<Value>(PROGRAM_SCHEMA_JSON).map_err(|error| error.to_string())?;
    let resources = NODE_SCHEMA_JSONS
        .iter()
        .map(|source| serde_json::from_str::<Value>(source).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, String>>()?;
    let root_id = root
        .get("$id")
        .and_then(Value::as_str)
        .ok_or_else(|| "embedded program schema is missing $id".to_owned())?;
    let mut registry = Registry::new()
        .add(root_id, &root)
        .map_err(|error| error.to_string())?;
    for schema in &resources {
        let id = schema
            .get("$id")
            .and_then(Value::as_str)
            .ok_or_else(|| "embedded node schema is missing $id".to_owned())?;
        registry = registry
            .add(id, schema)
            .map_err(|error| error.to_string())?;
    }
    let registry = registry.prepare().map_err(|error| error.to_string())?;

    jsonschema::draft202012::options()
        .with_registry(&registry)
        .build(&root)
        .map_err(|error| error.to_string())
}

fn build_standalone_validator(source: &str) -> Result<Validator, String> {
    let schema = serde_json::from_str::<Value>(source).map_err(|error| error.to_string())?;
    jsonschema::draft202012::options()
        .build(&schema)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::json;

    use super::*;

    #[test]
    fn accepts_a_schema_complete_minimal_program() {
        validate_program_schema(&minimal_program()).expect("minimal program should match schema");
    }

    #[test]
    fn program_variable_metadata_exactly_unions_manifest_and_runtime_data_types() {
        let program_schema: Value = serde_json::from_str(PROGRAM_SCHEMA_JSON)
            .expect("embedded program schema should parse");
        let manifest_schema: Value = serde_json::from_str(MANIFEST_SCHEMA_JSON)
            .expect("embedded manifest schema should parse");
        let runtime_types = program_schema["$defs"]["runtimeDataType"]["enum"]
            .as_array()
            .expect("runtimeDataType should be an enum");
        let manifest_variable_types =
            manifest_schema["properties"]["variables"]["items"]["properties"]["type"]["enum"]
                .as_array()
                .expect("manifest variable type should be an enum");
        let variable_types = program_schema["$defs"]["variableType"]["enum"]
            .as_array()
            .expect("variableType should be an enum");
        let expected = runtime_types
            .iter()
            .chain(manifest_variable_types)
            .map(|value| value.as_str().expect("type names should be strings"))
            .collect::<BTreeSet<_>>();
        let actual = variable_types
            .iter()
            .map(|value| value.as_str().expect("type names should be strings"))
            .collect::<BTreeSet<_>>();

        assert_eq!(actual, expected);
    }

    #[test]
    fn accepts_color_runtime_variable_exported_by_the_editor() {
        let mut program = minimal_program();
        program["entry"]["program"]["runtime_context"]["variables"] = json!([{
            "name": "n-pixel.hex",
            "token": "{{n-pixel.hex}}",
            "type": "color",
            "scope": "node_output",
            "source": "node_output",
            "read_only": true,
            "description": "Pixel color as a hex string."
        }]);

        validate_program_schema(&program)
            .expect("editor color runtime variable should match the runner schema");
    }

    #[test]
    fn validates_manifest_schema_before_deserialization() {
        let manifest = minimal_manifest();
        validate_manifest_schema(&manifest).expect("minimal manifest should match schema");

        let mut unknown_field = manifest.clone();
        unknown_field["unexpected"] = json!(true);
        assert!(matches!(
            validate_manifest_schema(&unknown_field),
            Err(PackageLoadError::ManifestSchema(_))
        ));

        let mut oversized_name = manifest;
        oversized_name["name"] = json!("x".repeat(129));
        assert!(matches!(
            validate_manifest_schema(&oversized_name),
            Err(PackageLoadError::ManifestSchema(_))
        ));
    }

    #[test]
    fn accepts_typed_default_variables_exported_by_the_editor() {
        let cases = [
            ("string", json!("value")),
            ("integer", json!(42)),
            ("float", json!(42.5)),
            ("boolean", json!(false)),
            ("object", json!({ "key": "value" })),
            ("list", json!(["value"])),
            (
                "datetime",
                json!({"type": "datetime", "value": "2026-08-03T12:00:00Z"}),
            ),
            (
                "duration",
                json!({"type": "duration", "unit": "seconds", "value": 3}),
            ),
            ("color", json!("#ff8800")),
            ("keyboard_key", json!("Ctrl+S")),
        ];

        for (variable_type, value) in cases {
            let mut manifest = minimal_manifest();
            let mut variable = json!({
                "name": "example",
                "scope": "persistent",
                "type": variable_type,
                "description": "",
                "value": value
            });
            if variable_type == "list" {
                variable["item_type"] = json!("string");
            }
            manifest["variables"] = json!([variable]);

            validate_manifest_schema(&manifest).unwrap_or_else(|error| {
                panic!("{variable_type} default variable should match schema: {error}")
            });
        }
    }

    #[test]
    fn rejects_unknown_node_config_and_action_types() {
        let mut unknown_config = minimal_program();
        unknown_config["entry"]["trigger"]["config"]["unexpected"] = json!(true);
        assert!(matches!(
            validate_program_schema(&unknown_config),
            Err(PackageLoadError::ProgramSchema(_))
        ));

        let mut unknown_action = minimal_program();
        unknown_action["entry"]["trigger"]["action_type"] = json!("trigger.unknown");
        assert!(matches!(
            validate_program_schema(&unknown_action),
            Err(PackageLoadError::ProgramSchema(_))
        ));
    }

    #[test]
    fn accepts_text_transform_operations_and_runtime_outputs_exported_by_the_editor() {
        for operation in ["uppercase", "sentence_case", "capitalize_words"] {
            let mut program = minimal_program();
            program["entry"]["program"]["steps"] = json!([{
                "id": "n-transform",
                "action_type": "action.text.format",
                "type": "action",
                "action": "format_text",
                "config": {
                    "input": "{{test}}",
                    "operations": [
                        {
                            "id": format!("operation-{operation}"),
                            "operation": operation
                        }
                    ]
                },
                "runtime_outputs": [
                    {
                        "name": "text",
                        "type": "string",
                        "description": "Transformed text result.",
                        "example": "n-transform.text"
                    },
                    {
                        "name": "items",
                        "type": "list",
                        "description": "List result for split and join operations.",
                        "example": "n-transform.items"
                    }
                ]
            }]);

            validate_program_schema(&program)
                .unwrap_or_else(|error| panic!("{operation} export should match schema: {error}"));
        }
    }

    #[test]
    fn accepts_parse_url_contract_exported_by_the_editor() {
        let mut program = minimal_program();
        program["entry"]["program"]["steps"] = json!([{
            "id": "n-parse-url",
            "action_type": "action.url.parse",
            "type": "action",
                "action": "parse_url",
            "config": {
                "url": "{{request_url}}"
            },
            "runtime_outputs": [
                {
                    "name": "protocol",
                    "type": "string",
                    "description": "URL protocol.",
                    "example": "n-parse-url.protocol"
                },
                {
                    "name": "query_parameters",
                    "type": "list",
                    "description": "Ordered query parameter entries.",
                    "example": "n-parse-url.query_parameters"
                }
            ]
        }]);

        validate_program_schema(&program).expect("Parse URL export should match schema");
    }

    #[test]
    fn accepts_for_each_with_only_node_scoped_runtime_outputs() {
        let mut program = minimal_program();
        program["entry"]["program"]["steps"] = json!([{
            "id": "n-each",
            "action_type": "control.for_each",
            "type": "for_each",
            "config": {
                "items": "{{n-source.items}}"
            },
            "runtime_outputs": [
                {
                    "name": "item",
                    "type": "object",
                    "description": "Current list item for the active iteration.",
                    "example": "n-each.item"
                }
            ]
        }]);

        validate_program_schema(&program).expect("For Each node outputs should match schema");

        program["entry"]["program"]["steps"][0]["config"]["itemVariable"] = json!("item");
        assert!(matches!(
            validate_program_schema(&program),
            Err(PackageLoadError::ProgramSchema(_))
        ));
    }

    #[test]
    fn accepts_color_match_control_contract_exported_by_the_editor() {
        let mut program = minimal_program();
        program["entry"]["program"]["steps"] = json!([{
            "id": "n-color-match",
            "action_type": "control.color_match",
            "type": "color_match",
            "config": {
                "actualColor": "{{n-pixel.rgb}}",
                "expectedColor": "#336699",
                "comparisonMode": "total_distance",
                "tolerancePercent": "12.5"
            },
            "runtime_outputs": [
                {
                    "name": "matches",
                    "type": "boolean",
                    "description": "Whether the colors matched within tolerance.",
                    "example": "true"
                }
            ]
        }]);

        validate_program_schema(&program).expect("Color Match export should match schema");
    }

    #[test]
    fn accepts_millisecond_delay_and_schedule_units() {
        let mut scheduled = minimal_program();
        scheduled["entry"]["trigger"] = json!({
            "id": "n-schedule",
            "action_type": "trigger.schedule",
            "type": "schedule",
            "config": { "every": "25", "unit": "milliseconds" },
            "runtime_outputs": []
        });
        validate_program_schema(&scheduled).expect("millisecond schedule should match schema");

        let mut delayed = minimal_program();
        delayed["entry"]["program"]["steps"] = json!([{
            "id": "n-delay",
            "action_type": "action.delay",
            "type": "action",
            "action": "delay",
            "config": { "amount": "25", "unit": "milliseconds" },
            "runtime_outputs": []
        }]);
        validate_program_schema(&delayed).expect("millisecond delay should match schema");
    }

    #[test]
    fn rejects_unknown_delay_and_schedule_units() {
        let mut scheduled = minimal_program();
        scheduled["entry"]["trigger"] = json!({
            "id": "n-schedule",
            "action_type": "trigger.schedule",
            "type": "schedule",
            "config": { "every": "1", "unit": "fortnights" },
            "runtime_outputs": []
        });
        assert!(matches!(
            validate_program_schema(&scheduled),
            Err(PackageLoadError::ProgramSchema(_))
        ));

        let mut delayed = minimal_program();
        delayed["entry"]["program"]["steps"] = json!([{
            "id": "n-delay",
            "action_type": "action.delay",
            "type": "action",
            "action": "delay",
            "config": { "amount": "1", "unit": "fortnights" },
            "runtime_outputs": []
        }]);
        assert!(matches!(
            validate_program_schema(&delayed),
            Err(PackageLoadError::ProgramSchema(_))
        ));
    }

    fn minimal_program() -> Value {
        json!({
            "entry": {
                "trigger": {
                    "id": "n-trigger",
                    "action_type": "trigger.manual",
                    "type": "manual",
                    "config": {},
                    "runtime_outputs": []
                },
                "triggers": [],
                "program": {
                    "type": "block",
                    "execution_model": "directed_graph",
                    "runtime_context": {
                        "expression_reference": "{{node-id.data_name}}",
                        "template_reference": "{{node-id.data_name}}",
                        "variables": [],
                        "built_in_variables": {
                            "syntax": "{{variable_name}}",
                            "variables": []
                        },
                        "node_outputs": []
                    },
                    "steps": [],
                    "edges": []
                }
            }
        })
    }

    fn minimal_manifest() -> Value {
        json!({
            "format_version": 1,
            "script_language_version": 1,
            "id": "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7",
            "name": "schema-test",
            "version": "1.0.0",
            "created_with": "BaudBound Test",
            "created_at": "2026-01-01T00:00:00.000Z",
            "minimum_runner_version": "2.0.0"
        })
    }
}
