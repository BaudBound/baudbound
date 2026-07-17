use crate::runtime::{
    RuntimeConditionRow, RuntimeSwitchCaseRow, compare_condition_values, config_string,
    required_config_string, resolve_config_map, resolve_template_value, value_kind,
    values_equal_for_condition,
};
use serde_json::Number;
use serde_json::Value;

use super::{RuntimeError, RuntimeExecutor, RuntimeNode};

impl RuntimeExecutor<'_> {
    pub(super) fn evaluate_color_match(
        &mut self,
        node: &RuntimeNode,
    ) -> Result<bool, RuntimeError> {
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
        );
        self.set_variable(
            format!("{}.difference_percent", node.id),
            color_match_number(node, evaluation.difference_percent)?,
        );
        self.set_variable(
            format!("{}.red_difference", node.id),
            Value::from(evaluation.red_difference),
        );
        self.set_variable(
            format!("{}.green_difference", node.id),
            Value::from(evaluation.green_difference),
        );
        self.set_variable(
            format!("{}.blue_difference", node.id),
            Value::from(evaluation.blue_difference),
        );
        Ok(evaluation.matches)
    }

    pub(super) fn evaluate_conditions(&self, node: &RuntimeNode) -> Result<bool, RuntimeError> {
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
            return Ok(false);
        }

        let mut result = false;
        for (index, row) in rows.iter().enumerate() {
            let compared = compare_condition_values(
                &resolve_template_value(&row.left, &self.context.variables),
                &row.operator,
                &resolve_template_value(&row.right, &self.context.variables),
            )
            .map_err(|message| RuntimeError::ControlFlow {
                node_id: node.id.clone(),
                message,
            })?;
            let row_result = if row.invert { !compared } else { compared };

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
        Ok(result)
    }

    pub(super) fn evaluate_switch(
        &mut self,
        node: &RuntimeNode,
    ) -> Result<Option<String>, RuntimeError> {
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
                        "Switch {} matched case \"{}\".",
                        node.id,
                        if switch_case.name.trim().is_empty() {
                            switch_case.id.as_str()
                        } else {
                            switch_case.name.as_str()
                        }
                    ),
                    Some(node.id.clone()),
                );
                return Ok(Some(format!("case-{}", switch_case.id)));
            }
        }
        Ok(None)
    }

    pub(super) fn loop_count(&self, node: &RuntimeNode) -> Result<u64, RuntimeError> {
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
            message: "loop count must be an exact unsigned 64-bit integer".to_owned(),
        })
    }

    pub(super) fn for_each_items(&self, node: &RuntimeNode) -> Result<Vec<Value>, RuntimeError> {
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
