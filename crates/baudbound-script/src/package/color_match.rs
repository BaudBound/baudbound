use serde_json::Value;

use super::PackageLoadError;
use crate::parse_rgb_color;

pub(super) fn validate_program_color_match_contract(
    program: &Value,
) -> Result<(), PackageLoadError> {
    validate_value(program).map_err(PackageLoadError::ProgramColor)
}

fn validate_value(value: &Value) -> Result<(), String> {
    match value {
        Value::Array(values) => {
            for value in values {
                validate_value(value)?;
            }
        }
        Value::Object(object) => {
            if object.get("action_type").and_then(Value::as_str) == Some("control.color_match") {
                validate_color_match_node(object)?;
            }
            for value in object.values() {
                validate_value(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_color_match_node(object: &serde_json::Map<String, Value>) -> Result<(), String> {
    let node_id = object
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("control.color_match");
    let config = object
        .get("config")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("node {node_id:?} is missing color match configuration"))?;

    for (key, label) in [
        ("actualColor", "actual color"),
        ("expectedColor", "expected color"),
    ] {
        let value = config
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("node {node_id:?} is missing {label}"))?;
        if contains_template_reference(value) {
            continue;
        }
        parse_rgb_color(&Value::String(value.to_owned()), label)
            .map_err(|message| format!("node {node_id:?} {message}"))?;
    }
    Ok(())
}

fn contains_template_reference(value: &str) -> bool {
    value
        .find("{{")
        .and_then(|start| value[start + 2..].find("}}"))
        .is_some()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn validates_literal_colors_and_defers_runtime_expressions() {
        for actual in [
            json!("#102030"),
            json!("rgb(16, 32, 48)"),
            json!("{{pixel.rgb}}"),
        ] {
            let program = json!({
                "action_type": "control.color_match",
                "id": "n-color",
                "config": {
                    "actualColor": actual,
                    "expectedColor": "#102030",
                }
            });
            validate_program_color_match_contract(&program).expect("valid color match config");
        }
    }

    #[test]
    fn rejects_invalid_static_colors() {
        let program = json!({
            "action_type": "control.color_match",
            "id": "n-color",
            "config": {
                "actualColor": "#xyzxyz",
                "expectedColor": "#102030",
            }
        });
        assert!(validate_program_color_match_contract(&program).is_err());
    }
}
