use std::collections::BTreeMap;

use crate::runtime::{
    config_string, duration_from_amount, evaluate_calculation_expression, number_from_value,
    number_value, render_template, resolve_config_map, resolve_http_request_config,
    value_to_string,
};
use crate::{ValueType, validate_value};

use super::{
    RunVariableScope, RuntimeActionError, RuntimeActionFailure, RuntimeActionRequest, RuntimeError,
    RuntimeExecutor, RuntimeNode, action_diagnostics::action_completion_message,
    cast_validation::validate_config_casts,
};

impl RuntimeExecutor<'_> {
    pub(super) fn execute_node(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        // Every node, not just the ones that call an external action. Log,
        // variable operations, delay and calculate render templates too, and a
        // cast that fails during rendering resolves to the literal template
        // text rather than stopping, so it has to be proven here.
        validate_config_casts(&node.id, &node.config, &self.context.variables)?;
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
        self.push_runtime_log(&level, message, Some(node.id.clone()));
        Ok(())
    }

    fn execute_external_action(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        self.ensure_not_cancelled()?;
        if node.action_type == "action.webhook_response" {
            self.validate_webhook_response_state(node)?;
        }
        let config = if node.action_type == "action.http" {
            resolve_http_request_config(&node.id, &node.config, &self.context.variables).map_err(
                |message| structured_http_runtime_error(node, "INVALID_REQUEST", message, false),
            )?
        } else {
            resolve_config_map(&node.config, &self.context.variables)
        };
        // No action type declares non-numeric typed config fields yet, so
        // this always validates against an empty map today. The call site
        // stays in place so the boundary takes effect the moment such a
        // declaration exists, without another pass over the dispatch code.
        validate_typed_config(&node.id, &config, &BTreeMap::new())?;
        baudbound_script::validate_resolved_numeric_config(&node.action_type, &config).map_err(
            |message| {
                if node.action_type == "action.http" {
                    structured_http_runtime_error(node, "INVALID_TIMEOUT", message, false)
                } else {
                    RuntimeError::Action {
                        node_id: node.id.clone(),
                        message,
                    }
                }
            },
        )?;
        let request = RuntimeActionRequest {
            action: node.action.clone(),
            action_type: node.action_type.clone(),
            config,
            node_id: node.id.clone(),
        };

        if node.action_type == "action.http" {
            self.log_http_request(node, &request.config);
        }

        let result = match self.action_handler.execute_action(&request, &self.context) {
            Ok(result) => result,
            Err(RuntimeActionError::Cancelled) => return Err(RuntimeError::Cancelled),
            Err(RuntimeActionError::Unsupported(action_type)) => {
                return Err(RuntimeError::UnsupportedStep {
                    action_type,
                    node_id: node.id.clone(),
                });
            }
            Err(RuntimeActionError::Failed { message, .. }) => {
                return Err(RuntimeError::Action {
                    node_id: node.id.clone(),
                    message,
                });
            }
            Err(RuntimeActionError::StructuredFailure { failure, .. }) => {
                return Err(RuntimeError::StructuredAction {
                    node_id: node.id.clone(),
                    failure,
                });
            }
        };
        self.ensure_not_cancelled()?;

        for reference in &result.sensitive_output_keys {
            let value = resolve_sensitive_output_reference(&result.output_data, reference)
                .ok_or_else(|| RuntimeError::Action {
                    node_id: node.id.clone(),
                    message: format!(
                        "action marked missing output reference {reference:?} as sensitive"
                    ),
                })?;
            if !self.secret_values.iter().any(|existing| existing == value) {
                self.secret_values.push(value.clone());
            }
            if !self
                .transient_sensitive_values
                .iter()
                .any(|existing| existing == value)
            {
                self.transient_sensitive_values.push(value.clone());
            }
        }

        if node.action_type == "action.http" {
            self.log_http_response(node, &request.config, &result.output_data);
        }

        let completion_message =
            action_completion_message(&node.action_type, &request.config, &result.output_data);
        for (key, value) in result.output_data {
            let name = format!("{}.{}", node.id, key);
            let explicitly_sensitive = result
                .sensitive_output_keys
                .iter()
                .any(|reference| sensitive_output_root(reference) == key);
            if explicitly_sensitive || self.value_contains_sensitive(&value) {
                let transient =
                    explicitly_sensitive || self.value_contains_transient_sensitive(&value);
                self.set_sensitive_variable(name, value, RunVariableScope::NodeOutput, transient)?;
            } else {
                self.set_variable(name, value, RunVariableScope::NodeOutput)?;
            }
        }
        if node.action_type == "action.webhook_response" {
            self.webhook_response_sent = true;
        }

        if let Some(message) = completion_message {
            self.push_runtime_log("info", message, Some(node.id.clone()));
        }
        Ok(())
    }

    fn validate_webhook_response_state(&self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        let waiting = self
            .context
            .trigger_payload
            .get("response")
            .and_then(|response| response.get("waiting"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !waiting {
            return Err(RuntimeError::Action {
                node_id: node.id.clone(),
                message: "Webhook Response reached without a waiting webhook request.".to_owned(),
            });
        }
        if self.webhook_response_sent {
            return Err(RuntimeError::Action {
                node_id: node.id.clone(),
                message: "Webhook response was already sent for this request.".to_owned(),
            });
        }
        Ok(())
    }

    fn execute_delay(&mut self, node: &RuntimeNode) -> Result<(), RuntimeError> {
        validate_config_casts(&node.id, &node.config, &self.context.variables)?;
        let config = resolve_config_map(&node.config, &self.context.variables);
        // Delay's fields are all numeric and stay under `NumericKind`; see
        // the comment on the call above in `execute_external_action`.
        validate_typed_config(&node.id, &config, &BTreeMap::new())?;
        baudbound_script::validate_resolved_numeric_config(&node.action_type, &config).map_err(
            |message| RuntimeError::Action {
                node_id: node.id.clone(),
                message,
            },
        )?;
        let amount = number_from_value(config.get("amount"))
            .or_else(|| number_from_value(config.get("every")))
            .unwrap_or(0.0);
        let unit = config_string(&config, "unit").unwrap_or_else(|| "seconds".to_owned());
        let duration =
            duration_from_amount(amount, &unit).map_err(|message| RuntimeError::Action {
                node_id: node.id.clone(),
                message,
            })?;
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
        self.set_variable(
            format!("{}.result", node.id),
            value.clone(),
            RunVariableScope::NodeOutput,
        )?;
        self.push_runtime_log(
            "info",
            format!(
                "Evaluated calculation expression {}. Result: {}.",
                serde_json::to_string(&rendered).unwrap_or_else(|_| rendered.clone()),
                serde_json::to_string(&value).unwrap_or_else(|_| value_to_string(&value))
            ),
            Some(node.id.clone()),
        );
        Ok(())
    }
}

/// Validates a resolved config map against the types its fields require.
///
/// Runs after template resolution and before the action executes, so a type
/// error stops the run before any side effect occurs. Numeric fields are
/// deliberately excluded from `field_types`: they are validated by
/// `NumericKind` in `baudbound_script::validate_resolved_numeric_config`,
/// which accepts a whole number through a float field (for example a literal
/// `440` in `beep.frequencyHz`), a leniency `ValueType::Float` does not share.
pub(crate) fn validate_typed_config(
    node_id: &str,
    config: &serde_json::Map<String, serde_json::Value>,
    field_types: &BTreeMap<String, ValueType>,
) -> Result<(), RuntimeError> {
    for (field, value_type) in field_types {
        let Some(value) = config.get(field) else {
            continue;
        };
        if let Err(reason) = validate_value(value, *value_type) {
            return Err(RuntimeError::Type {
                node_id: node_id.to_owned(),
                message: format!("field \"{field}\" {reason}"),
            });
        }
    }
    Ok(())
}

fn structured_http_runtime_error(
    node: &RuntimeNode,
    code: &'static str,
    message: String,
    retryable: bool,
) -> RuntimeError {
    RuntimeError::StructuredAction {
        node_id: node.id.clone(),
        failure: RuntimeActionFailure::new(code, "http", message, retryable),
    }
}

fn resolve_sensitive_output_reference<'a>(
    outputs: &'a serde_json::Map<String, serde_json::Value>,
    reference: &str,
) -> Option<&'a serde_json::Value> {
    let mut segments = reference.split('.');
    let root = segments.next()?;
    let mut value = outputs.get(root)?;
    for segment in segments {
        if segment.is_empty() {
            return None;
        }
        value = value.as_object()?.get(segment)?;
    }
    Some(value)
}

fn sensitive_output_root(reference: &str) -> &str {
    reference
        .split_once('.')
        .map_or(reference, |(root, _)| root)
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, json};

    use super::{resolve_sensitive_output_reference, sensitive_output_root};

    #[test]
    fn resolves_nested_sensitive_output_references_without_widening_the_secret() {
        let outputs = Map::from_iter([(
            "values".to_owned(),
            json!({"password":"secret-value","username":"Ada"}),
        )]);

        assert_eq!(
            resolve_sensitive_output_reference(&outputs, "values.password"),
            Some(&json!("secret-value"))
        );
        assert_eq!(sensitive_output_root("values.password"), "values");
        assert_eq!(
            resolve_sensitive_output_reference(&outputs, "values.username"),
            Some(&json!("Ada"))
        );
    }

    #[test]
    fn rejects_missing_or_malformed_sensitive_output_references() {
        let outputs = Map::from_iter([("values".to_owned(), json!({"password":"secret-value"}))]);

        assert_eq!(
            resolve_sensitive_output_reference(&outputs, "values.missing"),
            None
        );
        assert_eq!(
            resolve_sensitive_output_reference(&outputs, "values..password"),
            None
        );
        assert_eq!(
            resolve_sensitive_output_reference(&outputs, ".password"),
            None
        );
    }
}
