use crate::runtime::{
    RuntimeConditionRow, RuntimeSwitchCaseRow, compare_condition_values_with_end, config_string,
    required_config_string, resolve_config_map, resolve_template_value, template_value_is_defined,
    value_kind, values_equal_for_condition,
};
use serde_json::Number;
use serde_json::Value;

use super::{
    RunVariableScope, RuntimeError, RuntimeExecutor, RuntimeNode,
    cast_validation::validate_config_casts,
};

pub(super) struct ConditionEvaluation {
    pub(super) result: bool,
    pub(super) summary: String,
}

impl RuntimeExecutor<'_> {
    pub(super) fn evaluate_color_match(
        &mut self,
        node: &RuntimeNode,
    ) -> Result<bool, RuntimeError> {
        validate_config_casts(&node.id, &node.config, &self.context.variables)?;
        let config = resolve_config_map(&node.config, &self.context.variables);
        baudbound_script::validate_resolved_numeric_config(&node.action_type, &config).map_err(
            |message| RuntimeError::ControlFlow {
                node_id: node.id.clone(),
                message,
            },
        )?;
        let actual = config
            .get("actualColor")
            .ok_or_else(|| RuntimeError::ControlFlow {
                node_id: node.id.clone(),
                message: "actual color is required".to_owned(),
            })?;
        let expected = config
            .get("expectedColor")
            .ok_or_else(|| RuntimeError::ControlFlow {
                node_id: node.id.clone(),
                message: "expected color is required".to_owned(),
            })?;
        let mode = config
            .get("comparisonMode")
            .and_then(Value::as_str)
            .ok_or_else(|| RuntimeError::ControlFlow {
                node_id: node.id.clone(),
                message: "comparison mode is required".to_owned(),
            })
            .and_then(|value| {
                baudbound_script::ColorComparisonMode::parse(value).map_err(|message| {
                    RuntimeError::ControlFlow {
                        node_id: node.id.clone(),
                        message,
                    }
                })
            })?;
        let tolerance = crate::runtime::number_from_value(config.get("tolerancePercent"))
            .ok_or_else(|| RuntimeError::ControlFlow {
                node_id: node.id.clone(),
                message: "tolerance must be a finite percentage from 0 through 100".to_owned(),
            })?;
        let evaluation = baudbound_script::evaluate_color_match(actual, expected, mode, tolerance)
            .map_err(|message| RuntimeError::ControlFlow {
                node_id: node.id.clone(),
                message,
            })?;

        self.set_variable(
            format!("{}.matches", node.id),
            Value::Bool(evaluation.matches),
            RunVariableScope::NodeOutput,
        )?;
        self.set_variable(
            format!("{}.difference_percent", node.id),
            color_match_number(node, evaluation.difference_percent)?,
            RunVariableScope::NodeOutput,
        )?;
        self.set_variable(
            format!("{}.red_difference", node.id),
            Value::from(evaluation.red_difference),
            RunVariableScope::NodeOutput,
        )?;
        self.set_variable(
            format!("{}.green_difference", node.id),
            Value::from(evaluation.green_difference),
            RunVariableScope::NodeOutput,
        )?;
        self.set_variable(
            format!("{}.blue_difference", node.id),
            Value::from(evaluation.blue_difference),
            RunVariableScope::NodeOutput,
        )?;
        let selected = if evaluation.matches {
            "match"
        } else {
            "no_match"
        };
        self.push_runtime_log(
            "info",
            format!(
                "Compared actual color {} with expected color {} using {} mode and {}% tolerance. Difference: {}%. Selected {selected:?} output.",
                diagnostic_value(actual),
                diagnostic_value(expected),
                config_string(&config, "comparisonMode").unwrap_or_else(|| "unknown".to_owned()),
                diagnostic_value(config.get("tolerancePercent").unwrap_or(&Value::Null)),
                diagnostic_value(&color_match_number(node, evaluation.difference_percent)?)
            ),
            Some(node.id.clone()),
        );
        Ok(evaluation.matches)
    }

    pub(super) fn evaluate_conditions(
        &self,
        node: &RuntimeNode,
    ) -> Result<ConditionEvaluation, RuntimeError> {
        // A condition decides which branch runs, so a silently unresolved cast
        // here would send the program down the wrong path rather than stop it.
        validate_config_casts(&node.id, &node.config, &self.context.variables)?;
        let conditions = node
            .config
            .get("conditions")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        let rows =
            serde_json::from_value::<Vec<RuntimeConditionRow>>(conditions).map_err(|source| {
                RuntimeError::ControlFlow {
                    node_id: node.id.clone(),
                    message: format!("condition rows are malformed: {source}"),
                }
            })?;

        if rows.is_empty() {
            return Ok(ConditionEvaluation {
                result: false,
                summary: "no condition rows were configured".to_owned(),
            });
        }

        let mut result = false;
        let mut summaries = Vec::with_capacity(rows.len());
        for (index, row) in rows.iter().enumerate() {
            let left = resolve_template_value(&row.left, &self.context.variables);
            let right = resolve_template_value(&row.right, &self.context.variables);
            let right_end = resolve_template_value(&row.right_end, &self.context.variables);
            let compared = match row.operator.as_str() {
                "is_null_or_missing" => {
                    !template_value_is_defined(&row.left, &self.context.variables) || left.is_null()
                }
                _ => compare_condition_values_with_end(
                    &left,
                    &row.operator,
                    &right,
                    Some(&right_end),
                )
                .map_err(|message| RuntimeError::ControlFlow {
                    node_id: node.id.clone(),
                    message,
                })?,
            };
            let row_result = if row.invert { !compared } else { compared };
            let comparison = if row.operator == "is_between" {
                format!(
                    "{} {} {} and {}",
                    diagnostic_value(&left),
                    condition_operator_label(&row.operator),
                    diagnostic_value(&right),
                    diagnostic_value(&right_end)
                )
            } else if condition_uses_right_value(&row.operator) {
                format!(
                    "{} {} {}",
                    diagnostic_value(&left),
                    condition_operator_label(&row.operator),
                    diagnostic_value(&right)
                )
            } else {
                format!(
                    "{} {}",
                    diagnostic_value(&left),
                    condition_operator_label(&row.operator)
                )
            };
            summaries.push(format!(
                "row {}: {}{} => {}",
                index + 1,
                if row.invert { "NOT " } else { "" },
                comparison,
                row_result
            ));

            if index == 0 {
                result = row_result;
                continue;
            }

            result = match row.combinator.as_deref() {
                Some("or") => result || row_result,
                Some("and") | None => result && row_result,
                Some(other) => {
                    return Err(RuntimeError::ControlFlow {
                        node_id: node.id.clone(),
                        message: format!("unsupported condition combinator {other}"),
                    });
                }
            };
        }
        Ok(ConditionEvaluation {
            result,
            summary: summaries.join(". "),
        })
    }

    pub(super) fn evaluate_switch(&mut self, node: &RuntimeNode) -> Result<String, RuntimeError> {
        validate_config_casts(&node.id, &node.config, &self.context.variables)?;
        let switch_value = resolve_template_value(
            &config_string(&node.config, "value").unwrap_or_default(),
            &self.context.variables,
        );
        let cases = node
            .config
            .get("cases")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        let cases =
            serde_json::from_value::<Vec<RuntimeSwitchCaseRow>>(cases).map_err(|source| {
                RuntimeError::ControlFlow {
                    node_id: node.id.clone(),
                    message: format!("switch cases are malformed: {source}"),
                }
            })?;

        for switch_case in cases {
            let raw_case_value = switch_case
                .value
                .or(switch_case.expected_value)
                .unwrap_or_default();
            let case_value = resolve_template_value(&raw_case_value, &self.context.variables);
            if values_equal_for_condition(&switch_value, &case_value) {
                self.push_runtime_log(
                    "info",
                    format!(
                        "Switch input {} matched case {:?} with value {}.",
                        diagnostic_value(&switch_value),
                        if switch_case.name.trim().is_empty() {
                            switch_case.id.as_str()
                        } else {
                            switch_case.name.as_str()
                        },
                        diagnostic_value(&case_value)
                    ),
                    Some(node.id.clone()),
                );
                return Ok(format!("case-{}", switch_case.id));
            }
        }
        self.push_runtime_log(
            "info",
            format!(
                "Switch input {} matched no case and selected the \"default\" output.",
                diagnostic_value(&switch_value)
            ),
            Some(node.id.clone()),
        );
        Ok("default".to_owned())
    }

    pub(super) fn repeat_count(&self, node: &RuntimeNode) -> Result<u64, RuntimeError> {
        validate_config_casts(&node.id, &node.config, &self.context.variables)?;
        let config = resolve_config_map(&node.config, &self.context.variables);
        baudbound_script::validate_resolved_numeric_config(&node.action_type, &config).map_err(
            |message| RuntimeError::ControlFlow {
                node_id: node.id.clone(),
                message,
            },
        )?;
        match config.get("count") {
            Some(Value::Number(value)) => value.as_u64(),
            Some(Value::String(value)) => value.trim().parse::<u64>().ok(),
            _ => None,
        }
        .ok_or_else(|| RuntimeError::ControlFlow {
            node_id: node.id.clone(),
            message: "repeat count must be an exact unsigned 64-bit integer".to_owned(),
        })
    }

    pub(super) fn for_each_items(&self, node: &RuntimeNode) -> Result<Vec<Value>, RuntimeError> {
        validate_config_casts(&node.id, &node.config, &self.context.variables)?;
        let raw_items = required_config_string(node, "items")?;
        let value = resolve_template_value(&raw_items, &self.context.variables);
        if let Value::Array(items) = value {
            return Ok(items);
        }
        if let Value::String(text) = &value
            && let Ok(Value::Array(items)) = serde_json::from_str::<Value>(text.trim())
        {
            return Ok(items);
        }
        Err(RuntimeError::ControlFlow {
            node_id: node.id.clone(),
            message: format!(
                "for-each items must resolve to a list, found {}",
                value_kind(&value)
            ),
        })
    }

    pub(super) fn default_success_handle(&self, node: &RuntimeNode) -> Option<String> {
        self.graph
            .first_available_output_handle(&node.id, &["success", "out"])
    }
}

fn color_match_number(node: &RuntimeNode, value: f64) -> Result<Value, RuntimeError> {
    Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(|| RuntimeError::ControlFlow {
            node_id: node.id.clone(),
            message: "color difference could not be represented as a finite JSON number".to_owned(),
        })
}

fn diagnostic_value(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

fn condition_operator_label(operator: &str) -> &str {
    match operator {
        "==" => "equals",
        ">" => "is greater than",
        ">=" => "is greater than or equal to",
        "<" => "is less than",
        "<=" => "is less than or equal to",
        "is_between" => "is between",
        "contains" => "contains",
        "equals_ignore_case" => "equals, ignoring case",
        "contains_ignore_case" => "contains, ignoring case",
        "starts_with" => "starts with",
        "ends_with" => "ends with",
        "regex_match" => "matches regular expression",
        "is_empty" => "is empty",
        "is_null_or_missing" => "is null or missing",
        "is_true" => "is true",
        "is_false" => "is false",
        "is_numeric" => "is numeric",
        "is_string" => "is a string",
        "is_boolean" => "is a boolean",
        "is_list" => "is a list",
        "is_object" => "is an object",
        "has_key" => "has key",
        "contains_item" => "contains item",
        other => other,
    }
}

fn condition_uses_right_value(operator: &str) -> bool {
    !matches!(
        operator,
        "is_empty"
            | "is_null_or_missing"
            | "is_true"
            | "is_false"
            | "is_numeric"
            | "is_string"
            | "is_boolean"
            | "is_list"
            | "is_object"
    )
}
