use crate::runtime::{
    RuntimeConditionRow, RuntimeSwitchCaseRow, compare_condition_values, config_string,
    number_from_value, required_config_string, resolve_template_value, value_kind,
    values_equal_for_condition,
};
use serde_json::Value;

use super::{RuntimeError, RuntimeExecutor, RuntimeNode};

impl RuntimeExecutor<'_> {
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
        let raw_count = node.config.get("count").cloned().unwrap_or(Value::Null);
        let value = match raw_count {
            Value::String(template) => resolve_template_value(&template, &self.context.variables),
            other => other,
        };
        let count = number_from_value(Some(&value)).unwrap_or(0.0);
        Ok(count.max(0.0).trunc() as u64)
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
