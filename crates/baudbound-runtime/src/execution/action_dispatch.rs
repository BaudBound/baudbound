use crate::runtime::{
    config_string, duration_from_amount, evaluate_calculation_expression, number_from_value,
    number_value, render_template, resolve_config_map, value_to_string,
};

use super::{
    RuntimeActionError, RuntimeActionRequest, RuntimeError, RuntimeExecutor, RuntimeLogEntry,
    RuntimeNode,
};

impl RuntimeExecutor<'_> {
    pub(super) fn execute_node(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        match node.action_type.as_str() {
            "action.log" => self.execute_log(node),
            "runtime.set_variable" => self.execute_variable_operation(node),
            "action.delay" => self.execute_delay(node),
            "action.calculate" => self.execute_calculate(node),
            action_type if action_type.starts_with("action.") => self.execute_external_action(node),
            action_type => Err(RuntimeError::UnsupportedStep {
                action_type: action_type.to_owned(),
                node_id: node.id.clone(),
            }),
        }
    }

    fn execute_log(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        let level = config_string(&node.config, "level").unwrap_or_else(|| "info".to_owned());
        let message_template = config_string(&node.config, "message").unwrap_or_default();
        let message = render_template(&message_template, &self.context.variables);
        self.logs.push(RuntimeLogEntry {
            level,
            message,
            node_id: Some(node.id.clone()),
        });
        Ok(())
    }

    fn execute_external_action(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        self.ensure_not_cancelled()?;
        let request = RuntimeActionRequest {
            action: node.action.clone(),
            action_type: node.action_type.clone(),
            config: resolve_config_map(&node.config, &self.context.variables),
            node_id: node.id.clone(),
        };

        let result = match self.action_handler.execute_action(&request, &self.context) {
            Ok(result) => result,
            Err(RuntimeActionError::Cancelled) => return Err(RuntimeError::Cancelled),
            Err(source) => {
                return Err(RuntimeError::Action {
                    node_id: node.id.clone(),
                    message: source.to_string(),
                });
            }
        };
        self.ensure_not_cancelled()?;

        for (key, value) in result.output_data {
            self.set_variable(format!("{}.{}", node.id, key), value);
        }

        self.push_runtime_log(
            "info",
            format!("Action {} completed.", node.action_type),
            Some(node.id.clone()),
        );
        Ok(())
    }

    fn execute_delay(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        let amount = number_from_value(node.config.get("amount"))
            .or_else(|| number_from_value(node.config.get("every")))
            .unwrap_or(0.0);
        let unit = config_string(&node.config, "unit").unwrap_or_else(|| "seconds".to_owned());
        let duration = duration_from_amount(amount, &unit);
        self.push_runtime_log(
            "info",
            format!("Delay started for {} ms.", duration.as_millis()),
            Some(node.id.clone()),
        );
        if self.cancellation.wait_for(duration) {
            return Err(RuntimeError::Cancelled);
        }
        self.push_runtime_log(
            "info",
            format!("Delay completed after {} ms.", duration.as_millis()),
            Some(node.id.clone()),
        );
        Ok(())
    }

    fn execute_calculate(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        let expression = config_string(&node.config, "expression").unwrap_or_default();
        let rendered = render_template(&expression, &self.context.variables);
        let result = evaluate_calculation_expression(&rendered).map_err(|message| {
            RuntimeError::Calculation {
                node_id: node.id.clone(),
                message,
            }
        })?;
        let value = number_value(node, result)?;
        self.set_variable(format!("{}.result", node.id), value.clone());
        self.push_runtime_log(
            "info",
            format!(
                "Calculation {} completed with result {}.",
                node.id,
                value_to_string(&value)
            ),
            Some(node.id.clone()),
        );
        Ok(())
    }
}
