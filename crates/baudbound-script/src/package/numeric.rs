use std::{collections::BTreeMap, sync::OnceLock};

use serde::Deserialize;
use serde_json::Value;

use super::PackageLoadError;

const NUMERIC_CONTRACT_JSON: &str =
    include_str!("../../../../contracts/runner/node-numeric-fields.json");

#[derive(Debug, Deserialize)]
struct NumericContract {
    nodes: BTreeMap<String, BTreeMap<String, NumericFieldContract>>,
    version: u32,
}

#[derive(Debug, Deserialize)]
struct NumericFieldContract {
    allows_variables: bool,
    kind: NumericKind,
    label: String,
    maximum: String,
    maximum_inclusive: bool,
    minimum: String,
    minimum_inclusive: bool,
    required: bool,
    signed: bool,
    when: Option<NumericFieldCondition>,
}

#[derive(Debug, Deserialize)]
struct NumericFieldCondition {
    equals: String,
    key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum NumericKind {
    Float,
    Integer,
}

pub(super) fn validate_program_numeric_contract(program: &Value) -> Result<(), PackageLoadError> {
    let contract = numeric_contract().map_err(PackageLoadError::NumericContract)?;
    validate_value(program, contract, true)
        .map_err(|message| PackageLoadError::ProgramNumeric(message.to_owned()))
}

pub(super) fn validate_resolved_numeric_config(
    action_type: &str,
    config: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let contract = numeric_contract()?;
    validate_node(action_type, None, config, contract, false)
}

fn numeric_contract() -> Result<&'static NumericContract, String> {
    static CONTRACT: OnceLock<Result<NumericContract, String>> = OnceLock::new();
    match CONTRACT.get_or_init(|| {
        let contract = serde_json::from_str::<NumericContract>(NUMERIC_CONTRACT_JSON)
            .map_err(|error| error.to_string())?;
        if contract.version != 2 {
            return Err(format!(
                "unsupported numeric contract version {}",
                contract.version
            ));
        }
        Ok(contract)
    }) {
        Ok(contract) => Ok(contract),
        Err(message) => Err(message.clone()),
    }
}

fn validate_value(
    value: &Value,
    contract: &NumericContract,
    defer_variables: bool,
) -> Result<(), String> {
    match value {
        Value::Array(values) => {
            for value in values {
                validate_value(value, contract, defer_variables)?;
            }
        }
        Value::Object(object) => {
            if let (Some(action_type), Some(Value::Object(config))) = (
                object.get("action_type").and_then(Value::as_str),
                object.get("config"),
            ) {
                validate_node(
                    action_type,
                    object.get("id").and_then(Value::as_str),
                    config,
                    contract,
                    defer_variables,
                )?;
            }
            for value in object.values() {
                validate_value(value, contract, defer_variables)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_node(
    action_type: &str,
    node_id: Option<&str>,
    config: &serde_json::Map<String, Value>,
    contract: &NumericContract,
    defer_variables: bool,
) -> Result<(), String> {
    let Some(fields) = contract.nodes.get(action_type) else {
        return Err(format!(
            "node type {action_type:?} is missing from the numeric contract"
        ));
    };
    for (key, field) in fields {
        if field.when.as_ref().is_some_and(|condition| {
            config.get(&condition.key).and_then(Value::as_str) != Some(condition.equals.as_str())
        }) {
            continue;
        }
        let Some(value) = config.get(key) else {
            if field.required {
                return Err(field_error(node_id, action_type, key, field, "is required"));
            }
            continue;
        };
        if !field.required && value.as_str().is_some_and(|value| value.trim().is_empty()) {
            continue;
        }
        if defer_variables
            && field.allows_variables
            && value
                .as_str()
                .is_some_and(|value| contains_template_reference(value.trim()))
        {
            continue;
        }

        let validation = match field.kind {
            NumericKind::Integer => validate_integer(value, field),
            NumericKind::Float => validate_float(value, field),
        };
        if let Err(message) = validation {
            return Err(field_error(node_id, action_type, key, field, &message));
        }
    }
    Ok(())
}

fn validate_integer(value: &Value, field: &NumericFieldContract) -> Result<(), String> {
    let raw = match value {
        Value::Number(number) => number.to_string(),
        Value::String(value) => value.trim().to_owned(),
        _ => return Err("must be an integer".to_owned()),
    };
    if !is_integer_literal(&raw, field.signed) {
        return Err(if field.signed {
            "must be a whole signed integer".to_owned()
        } else {
            "must be a whole non-negative integer".to_owned()
        });
    }
    let parsed = raw
        .parse::<i128>()
        .map_err(|_| "is outside the supported exact integer range".to_owned())?;
    let minimum = field
        .minimum
        .parse::<i128>()
        .map_err(|_| "has an invalid generated minimum".to_owned())?;
    let maximum = field
        .maximum
        .parse::<i128>()
        .map_err(|_| "has an invalid generated maximum".to_owned())?;
    validate_bounds(parsed, minimum, maximum, field)
}

fn validate_float(value: &Value, field: &NumericFieldContract) -> Result<(), String> {
    let parsed = match value {
        Value::Number(number) => number.as_f64(),
        Value::String(value) if is_float_literal(value.trim(), field.signed) => {
            value.trim().parse::<f64>().ok()
        }
        _ => None,
    }
    .filter(|value| value.is_finite())
    .ok_or_else(|| "must be a finite decimal number".to_owned())?;
    let minimum = field
        .minimum
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| "has an invalid generated minimum".to_owned())?;
    let maximum = field
        .maximum
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| "has an invalid generated maximum".to_owned())?;
    validate_bounds(parsed, minimum, maximum, field)
}

fn validate_bounds<T: Copy + PartialOrd + std::fmt::Display>(
    value: T,
    minimum: T,
    maximum: T,
    field: &NumericFieldContract,
) -> Result<(), String> {
    let above_minimum = if field.minimum_inclusive {
        value >= minimum
    } else {
        value > minimum
    };
    let below_maximum = if field.maximum_inclusive {
        value <= maximum
    } else {
        value < maximum
    };
    if above_minimum && below_maximum {
        Ok(())
    } else {
        Err(format!(
            "must be {} {} and {} {}",
            if field.minimum_inclusive {
                "at least"
            } else {
                "greater than"
            },
            field.minimum,
            if field.maximum_inclusive {
                "at most"
            } else {
                "less than"
            },
            field.maximum
        ))
    }
}

fn is_integer_literal(value: &str, signed: bool) -> bool {
    let unsigned = if let Some(value) = value.strip_prefix('-') {
        if !signed {
            return false;
        }
        value
    } else {
        value
    };
    is_canonical_digits(unsigned)
}

fn is_float_literal(value: &str, signed: bool) -> bool {
    let unsigned = if let Some(value) = value.strip_prefix('-') {
        if !signed {
            return false;
        }
        value
    } else {
        value
    };
    let (mantissa, exponent) = unsigned
        .split_once(['e', 'E'])
        .map_or((unsigned, None), |(mantissa, exponent)| {
            (mantissa, Some(exponent))
        });
    if exponent.is_some_and(|value| {
        let digits = value
            .strip_prefix('+')
            .or_else(|| value.strip_prefix('-'))
            .unwrap_or(value);
        digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        return false;
    }
    let (whole, fraction) = mantissa
        .split_once('.')
        .map_or((mantissa, None), |(whole, fraction)| {
            (whole, Some(fraction))
        });
    is_canonical_digits(whole)
        && fraction.is_none_or(|fraction| {
            !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn is_canonical_digits(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}

fn contains_template_reference(value: &str) -> bool {
    value.contains("{{") && value.contains("}}")
}

fn field_error(
    node_id: Option<&str>,
    action_type: &str,
    key: &str,
    field: &NumericFieldContract,
    message: &str,
) -> String {
    format!(
        "node {} ({action_type}) field {} ({key}) {message}",
        node_id.unwrap_or("<unknown>"),
        field.label
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn rejects_fractional_and_out_of_range_integer_literals_exactly() {
        for value in [
            json!("1.5"),
            json!("2147483648"),
            json!("-2147483649"),
            json!("1e2"),
        ] {
            let program = pixel_program(value, json!(0));
            assert!(validate_program_numeric_contract(&program).is_err());
        }
        validate_program_numeric_contract(&pixel_program(
            json!("-2147483648"),
            json!("2147483647"),
        ))
        .expect("i32 boundaries should be accepted");
    }

    #[test]
    fn accepts_negative_pixel_and_mouse_coordinates() {
        for program in [
            pixel_program(json!("-1920"), json!("-120")),
            mouse_program(json!("-1920"), json!("-120"), false),
            mouse_program(json!("-40"), json!("-20"), true),
        ] {
            validate_program_numeric_contract(&program)
                .expect("signed desktop coordinates should pass package validation");
        }
    }

    #[test]
    fn rejects_invalid_static_float_values_but_allows_runtime_templates() {
        let invalid = json!({
            "action_type": "action.beep",
            "id": "n-beep",
            "config": { "frequencyHz": "20001", "durationMs": "200" }
        });
        assert!(validate_program_numeric_contract(&invalid).is_err());

        let expression = json!({
            "action_type": "action.beep",
            "id": "n-beep",
            "config": { "frequencyHz": "{{frequency}}", "durationMs": "{{duration}}" }
        });
        validate_program_numeric_contract(&expression)
            .expect("runtime templates should defer validation");
        assert!(
            validate_resolved_numeric_config(
                "action.beep",
                expression["config"].as_object().expect("config object")
            )
            .is_err(),
            "unresolved templates must not pass resolved-value validation"
        );
    }

    #[test]
    fn applies_conditional_pid_contract_only_in_pid_mode() {
        let process_name = json!({
            "action_type": "action.process.kill",
            "id": "n-kill",
            "config": { "matchMode": "process_name", "target": "app.exe" }
        });
        validate_program_numeric_contract(&process_name)
            .expect("process names must remain valid outside PID mode");

        let oversized_pid = json!({
            "action_type": "action.process.kill",
            "id": "n-kill",
            "config": { "matchMode": "pid", "target": "4294967296" }
        });
        let error = validate_program_numeric_contract(&oversized_pid)
            .expect_err("PID above u32 must fail package validation");
        assert!(error.to_string().contains("at most 4294967295"));

        let resolved = oversized_pid["config"].as_object().expect("config object");
        assert!(validate_resolved_numeric_config("action.process.kill", resolved).is_err());
    }

    #[test]
    fn validates_every_declared_numeric_field_at_its_boundaries() {
        let contract = numeric_contract().expect("generated numeric contract should load");
        let mut tested_fields = 0;

        for (action_type, fields) in &contract.nodes {
            for (key, field) in fields {
                let case = format!("{action_type}.{key}");
                let mut config = valid_config(fields);

                config.insert(key.clone(), Value::String(valid_minimum(field)));
                validate_node(action_type, Some(&case), &config, contract, false)
                    .unwrap_or_else(|error| panic!("{case} minimum should pass: {error}"));

                config.insert(key.clone(), Value::String(field.maximum.clone()));
                validate_node(action_type, Some(&case), &config, contract, false)
                    .unwrap_or_else(|error| panic!("{case} maximum should pass: {error}"));

                config.insert(key.clone(), Value::String(invalid_below_minimum(field)));
                assert!(
                    validate_node(action_type, Some(&case), &config, contract, false).is_err(),
                    "{case} must reject a value below its minimum"
                );

                config.insert(key.clone(), Value::String(invalid_above_maximum(field)));
                assert!(
                    validate_node(action_type, Some(&case), &config, contract, false).is_err(),
                    "{case} must reject a value above its maximum"
                );

                config.insert(key.clone(), Value::String("not-a-number".to_owned()));
                assert!(
                    validate_node(action_type, Some(&case), &config, contract, false).is_err(),
                    "{case} must reject malformed input"
                );

                if field.allows_variables {
                    config.insert(key.clone(), Value::String("{{matrix_value}}".to_owned()));
                    validate_node(action_type, Some(&case), &config, contract, true)
                        .unwrap_or_else(|error| {
                            panic!("{case} package template should defer: {error}")
                        });
                    assert!(
                        validate_node(action_type, Some(&case), &config, contract, false).is_err(),
                        "{case} unresolved runtime template must fail"
                    );
                }

                config = valid_config(fields);
                config.remove(key);
                let missing = validate_node(action_type, Some(&case), &config, contract, false);
                assert_eq!(
                    missing.is_err(),
                    field.required,
                    "{case} required-field behavior must match the generated contract"
                );

                if let Some(condition) = &field.when {
                    config = valid_config(fields);
                    config.insert(
                        condition.key.clone(),
                        Value::String("not-applicable".to_owned()),
                    );
                    config.insert(key.clone(), Value::String("not-a-number".to_owned()));
                    validate_node(action_type, Some(&case), &config, contract, false)
                        .unwrap_or_else(|error| {
                            panic!("{case} inactive condition should skip validation: {error}")
                        });
                }

                tested_fields += 1;
            }
        }

        assert_eq!(
            tested_fields, 22,
            "matrix must cover every declared numeric field"
        );
    }

    fn valid_config(
        fields: &BTreeMap<String, NumericFieldContract>,
    ) -> serde_json::Map<String, Value> {
        let mut config = serde_json::Map::new();
        for (key, field) in fields {
            if let Some(condition) = &field.when {
                config.insert(
                    condition.key.clone(),
                    Value::String(condition.equals.clone()),
                );
            }
            config.insert(key.clone(), Value::String(valid_minimum(field)));
        }
        config
    }

    fn valid_minimum(field: &NumericFieldContract) -> String {
        if field.minimum_inclusive {
            return field.minimum.clone();
        }
        match field.kind {
            NumericKind::Integer => {
                (field.minimum.parse::<i128>().expect("integer minimum") + 1).to_string()
            }
            NumericKind::Float if field.minimum == "0" => "1".to_owned(),
            NumericKind::Float => {
                let minimum = field.minimum.parse::<f64>().expect("float minimum");
                let maximum = field.maximum.parse::<f64>().expect("float maximum");
                (minimum + ((maximum - minimum) / 2.0)).to_string()
            }
        }
    }

    fn invalid_below_minimum(field: &NumericFieldContract) -> String {
        if !field.minimum_inclusive {
            return field.minimum.clone();
        }
        match field.kind {
            NumericKind::Integer => {
                (field.minimum.parse::<i128>().expect("integer minimum") - 1).to_string()
            }
            NumericKind::Float => {
                (field.minimum.parse::<f64>().expect("float minimum") - 1.0).to_string()
            }
        }
    }

    fn invalid_above_maximum(field: &NumericFieldContract) -> String {
        if !field.maximum_inclusive {
            return field.maximum.clone();
        }
        match field.kind {
            NumericKind::Integer => {
                (field.maximum.parse::<i128>().expect("integer maximum") + 1).to_string()
            }
            NumericKind::Float if field.maximum == "1.7976931348623157e308" => "1e309".to_owned(),
            NumericKind::Float => {
                (field.maximum.parse::<f64>().expect("float maximum") + 1.0).to_string()
            }
        }
    }

    fn pixel_program(x: Value, y: Value) -> Value {
        json!({
            "action_type": "action.pixel.get",
            "id": "n-pixel",
            "config": { "x": x, "y": y }
        })
    }

    fn mouse_program(x: Value, y: Value, relative: bool) -> Value {
        json!({
            "action_type": "action.mouse.move",
            "id": "n-mouse",
            "config": { "relative": relative, "x": x, "y": y }
        })
    }
}
