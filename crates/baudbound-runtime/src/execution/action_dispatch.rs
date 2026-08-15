use serde_json::{Number, Value};

use crate::runtime::{
    SYSTEM_VARIABLE, config_string, duration_from_amount, evaluate_calculation_expression,
    number_from_value, number_value, refresh_derived_variable_metadata, render_template,
    resolve_config_map, resolve_http_request_config, value_to_string,
};

use super::{
    RunVariableScope, RuntimeActionError, RuntimeActionFailure, RuntimeActionRequest, RuntimeError,
    RuntimeExecutor, RuntimeNode, action_diagnostics::action_completion_message,
    cast_validation::validate_config_casts, initial_state::live_system_fields,
};

impl RuntimeExecutor<'_> {
    /// Rereads the @system fields that are readings rather than facts.
    ///
    /// Called once at the top of every node execution, which is the boundary
    /// that makes two references inside one field agree while still letting a
    /// loop or a delay see the clock move.
    fn refresh_live_system_fields(&mut self) {
        let Some(Value::Object(system)) = self.context.variables.get_mut(SYSTEM_VARIABLE) else {
            return;
        };
        for (field, value) in live_system_fields() {
            system.insert(field.to_owned(), value);
        }
        refresh_derived_variable_metadata(&mut self.context.variables, SYSTEM_VARIABLE);
    }

    pub(super) fn execute_node(
        &mut self,
        node: &RuntimeNode,
    ) -> Result<Option<String>, RuntimeError> {
        // One reading per node execution. Every reference inside this node sees
        // the same clock, and the next node reads again, so a loop or a delay
        // moves while two references in one field cannot disagree.
        self.refresh_live_system_fields();
        // Every node, not just the ones that call an external action. Log,
        // variable operations, delay and calculate render templates too, and a
        // cast that fails during rendering resolves to the literal template
        // text rather than stopping, so it has to be proven here.
        validate_config_casts(&node.id, &node.config, &self.context.variables)?;
        match node.action_type.as_str() {
            "action.log" => self.execute_log(node).map(|()| None),
            "runtime.set_variable" => self.execute_variable_operation(node).map(|()| None),
            "action.delay" => self.execute_delay(node).map(|()| None),
            "action.calculate" => self.execute_calculate(node).map(|()| None),
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

    fn execute_external_action(
        &mut self,
        node: &RuntimeNode,
    ) -> Result<Option<String>, RuntimeError> {
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

        let (result, expected_output) =
            match self.action_handler.execute_action(&request, &self.context) {
                Ok(result) => (result, None),
                Err(RuntimeActionError::Cancelled) => return Err(RuntimeError::Cancelled),
                Err(RuntimeActionError::ExpectedOutcome {
                    output,
                    output_data,
                    ..
                }) => (super::RuntimeActionResult::new(output_data), Some(output)),
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
        let selected_output =
            expected_output.or_else(|| select_action_output(node, &result.output_data));

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
        Ok(selected_output)
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
        // Older packages did not carry a result type, and historically Calculate
        // always returned a float. Keep that behavior for those packages while
        // new editor exports explicitly choose automatic, integer, or float.
        let result_type =
            config_string(&node.config, "resultType").unwrap_or_else(|| "float".to_owned());
        let value = calculation_result_value(node, result, &result_type)?;
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

fn select_action_output(
    node: &RuntimeNode,
    output: &serde_json::Map<String, Value>,
) -> Option<String> {
    let button = output.get("button").and_then(Value::as_str);
    match node.action_type.as_str() {
        "action.form_dialog" => match button {
            Some("ok") => Some("submitted".to_owned()),
            Some("cancel") => Some("cancelled".to_owned()),
            Some("timeout") => Some("timed_out".to_owned()),
            _ => None,
        },
        "action.message_box" => button.map(|button| {
            if button == "timeout" {
                "timed_out".to_owned()
            } else {
                button.to_owned()
            }
        }),
        "action.process.run" | "action.shell" => Some(
            if output.get("success").and_then(Value::as_bool) == Some(true) {
                "exited_zero"
            } else {
                "exited_nonzero"
            }
            .to_owned(),
        ),
        "action.process.status" => Some(
            if output.get("running").and_then(Value::as_bool) == Some(true) {
                "running"
            } else {
                "not_running"
            }
            .to_owned(),
        ),
        "action.http" => output
            .get("status_code")
            .and_then(Value::as_u64)
            .map(|status| {
                if (200..300).contains(&status) {
                    "ok"
                } else if (400..500).contains(&status) {
                    "client_error"
                } else if (500..600).contains(&status) {
                    "server_error"
                } else {
                    "unexpected_status"
                }
                .to_owned()
            }),
        "action.file.read" => Some("read".to_owned()),
        "action.file.delete" => Some("deleted".to_owned()),
        "action.window.focus" => Some("focused".to_owned()),
        "action.process.kill" => Some("killed".to_owned()),
        "action.websocket.write" | "action.serial.write" => Some("sent".to_owned()),
        _ => None,
    }
}

fn calculation_result_value(
    node: &RuntimeNode,
    result: f64,
    result_type: &str,
) -> Result<Value, RuntimeError> {
    match result_type {
        "float" => number_value(node, result),
        "integer" => calculation_integer_value(node, result),
        "automatic" => {
            if is_safe_integer(result) {
                calculation_integer_value(node, result)
            } else {
                number_value(node, result)
            }
        }
        _ => Err(RuntimeError::Calculation {
            node_id: node.id.clone(),
            message: format!("unsupported calculation result type {result_type:?}"),
        }),
    }
}

fn calculation_integer_value(node: &RuntimeNode, result: f64) -> Result<Value, RuntimeError> {
    if !is_safe_integer(result) {
        return Err(RuntimeError::Calculation {
            node_id: node.id.clone(),
            message: "integer result type requires a whole safe integer result".to_owned(),
        });
    }
    Ok(Value::Number(Number::from(result as i64)))
}

fn is_safe_integer(value: f64) -> bool {
    const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
    value.is_finite() && value.fract() == 0.0 && value.abs() <= MAX_SAFE_INTEGER
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

    use crate::RuntimeNode;

    use super::{resolve_sensitive_output_reference, select_action_output, sensitive_output_root};

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

    #[test]
    fn selects_specific_action_outputs_from_runtime_data() {
        for (action_type, outputs, expected) in [
            (
                "action.form_dialog",
                Map::from_iter([("button".to_owned(), json!("ok"))]),
                "submitted",
            ),
            (
                "action.message_box",
                Map::from_iter([("button".to_owned(), json!("yes"))]),
                "yes",
            ),
            (
                "action.process.run",
                Map::from_iter([("success".to_owned(), json!(true))]),
                "exited_zero",
            ),
            (
                "action.shell",
                Map::from_iter([("success".to_owned(), json!(false))]),
                "exited_nonzero",
            ),
            (
                "action.process.status",
                Map::from_iter([("running".to_owned(), json!(false))]),
                "not_running",
            ),
            (
                "action.http",
                Map::from_iter([("status_code".to_owned(), json!(204))]),
                "ok",
            ),
            (
                "action.http",
                Map::from_iter([("status_code".to_owned(), json!(404))]),
                "client_error",
            ),
            (
                "action.http",
                Map::from_iter([("status_code".to_owned(), json!(503))]),
                "server_error",
            ),
            (
                "action.http",
                Map::from_iter([("status_code".to_owned(), json!(302))]),
                "unexpected_status",
            ),
            ("action.file.read", Map::new(), "read"),
            ("action.file.delete", Map::new(), "deleted"),
            ("action.window.focus", Map::new(), "focused"),
            ("action.process.kill", Map::new(), "killed"),
            ("action.websocket.write", Map::new(), "sent"),
            ("action.serial.write", Map::new(), "sent"),
        ] {
            assert_eq!(
                select_action_output(&runtime_node(action_type), &outputs),
                Some(expected.to_owned()),
                "{action_type} should select {expected}"
            );
        }
    }

    fn runtime_node(action_type: &str) -> RuntimeNode {
        RuntimeNode {
            id: "n-test".to_owned(),
            action_type: action_type.to_owned(),
            node_type: "action".to_owned(),
            action: None,
            config: Map::new(),
        }
    }
}
