use serde_json::Value;

use crate::runtime::RuntimeRouterConfig;

use super::{RuntimeError, RuntimeExecutor, RuntimeNode};

pub(super) const ROUTER_INPUT_HANDLE_PREFIX: &str = "in-";
pub(super) const ROUTER_OUTPUT_HANDLE_PREFIX: &str = "out-";

impl RuntimeExecutor<'_> {
    /// Resolves the output handles a router follows for the input it was
    /// entered through, in configured route order.
    pub(super) fn evaluate_router(
        &mut self,
        node: &RuntimeNode,
        input_handle: Option<&str>,
    ) -> Result<Vec<String>, RuntimeError> {
        let config =
            serde_json::from_value::<RuntimeRouterConfig>(Value::Object(node.config.clone()))
                .map_err(|source| RuntimeError::ControlFlow {
                    node_id: node.id.clone(),
                    message: format!("router config is malformed: {source}"),
                })?;
        let Some(input_handle) = input_handle else {
            return Err(RuntimeError::ControlFlow {
                node_id: node.id.clone(),
                message: "router was entered without an input handle".to_owned(),
            });
        };
        let input = input_handle
            .strip_prefix(ROUTER_INPUT_HANDLE_PREFIX)
            .and_then(|input_id| config.inputs.iter().find(|port| port.id == input_id))
            .ok_or_else(|| RuntimeError::ControlFlow {
                node_id: node.id.clone(),
                message: format!("router input handle {input_handle:?} does not exist"),
            })?;

        let mut routes = config
            .routes
            .iter()
            .filter(|route| route.input_id == input.id)
            .collect::<Vec<_>>();
        routes.sort_by_key(|route| route.order);
        if routes.is_empty() {
            return Err(RuntimeError::ControlFlow {
                node_id: node.id.clone(),
                message: format!("router input {:?} has no routes", input.label),
            });
        }

        let mut handles = Vec::with_capacity(routes.len());
        for route in routes {
            if !config.outputs.iter().any(|port| port.id == route.output_id) {
                return Err(RuntimeError::ControlFlow {
                    node_id: node.id.clone(),
                    message: format!(
                        "router route references missing output {:?}",
                        route.output_id
                    ),
                });
            }
            handles.push(format!("{ROUTER_OUTPUT_HANDLE_PREFIX}{}", route.output_id));
        }

        self.push_runtime_log(
            "info",
            format!(
                "Router input {:?} selected {} output{}: {}.",
                input.label,
                handles.len(),
                if handles.len() == 1 { "" } else { "s" },
                handles.join(", ")
            ),
            Some(node.id.clone()),
        );
        Ok(handles)
    }
}
