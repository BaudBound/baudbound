use std::sync::OnceLock;

use jsonschema::{Registry, Validator};
use serde_json::Value;

use super::PackageLoadError;

include!(concat!(env!("OUT_DIR"), "/embedded_schemas.rs"));

pub(super) fn validate_program_schema(program: &Value) -> Result<(), PackageLoadError> {
    let validator = program_validator().map_err(PackageLoadError::SchemaContract)?;
    let errors = validator
        .iter_errors(program)
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
        Err(PackageLoadError::ProgramSchema(errors.join("; ")))
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn accepts_a_schema_complete_minimal_program() {
        validate_program_schema(&minimal_program()).expect("minimal program should match schema");
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
                    "operation": operation,
                    "input": "{{test}}"
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
}
