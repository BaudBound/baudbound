use crate::runtime::{RuntimeFrame, RuntimeGraph, refresh_derived_variable_metadata};
use crate::{RuntimeCancellationToken, RuntimeStateStore};
use serde_json::Value;

mod action_dispatch;
mod api;
mod branching;
mod contracts;
mod default_variables;
mod frames;
mod initial_state;
mod redaction;
mod variable_operations;

pub use api::*;
pub use contracts::*;
use initial_state::load_initial_state;

struct RuntimeExecutor<'a> {
    graph: RuntimeGraph,
    context: RuntimeContext,
    logs: Vec<RuntimeLogEntry>,
    action_handler: &'a dyn RuntimeActionHandler,
    cancellation: RuntimeCancellationToken,
    state_store: Option<&'a dyn RuntimeStateStore>,
    secret_names: Vec<String>,
    secret_values: Vec<Value>,
}

impl<'a> RuntimeExecutor<'a> {
    fn new(
        graph: RuntimeGraph,
        identity: RunIdentity,
        trigger_payload: Value,
        resources: RuntimeExecutionResources<'a>,
    ) -> Result<Self, RuntimeError> {
        let initial_state = load_initial_state(
            &graph,
            &identity.script_id,
            resources.state_store,
            resources.default_variables,
            resources.secrets,
        )?;
        Ok(Self {
            graph,
            context: RuntimeContext {
                cancellation: resources.cancellation.clone(),
                identity,
                package_path: resources.package_path,
                trigger_payload,
                variables: initial_state.variables,
            },
            logs: Vec::new(),
            action_handler: resources.action_handler,
            cancellation: resources.cancellation,
            state_store: resources.state_store,
            secret_names: initial_state.secret_names,
            secret_values: initial_state.secret_values,
        })
    }

    fn run_from_trigger(&mut self) -> Result<RunReport, RuntimeError> {
        self.ensure_not_cancelled()?;
        let trigger_node_id = self.context.identity.trigger_node_id.clone();
        self.push_runtime_log(
            "info",
            format!("Trigger {} started.", trigger_node_id),
            Some(trigger_node_id.clone()),
        );
        self.seed_trigger_payload_outputs(&trigger_node_id);

        let mut frames = vec![RuntimeFrame::Follow {
            source_node_id: trigger_node_id,
            handle: "out".to_owned(),
            stop_at_node_id: None,
        }];

        while let Some(frame) = frames.pop() {
            self.ensure_not_cancelled()?;
            self.process_frame(frame, &mut frames)?;
        }

        self.push_runtime_log("info", "Run completed.", None);
        Ok(RunReport {
            identity: self.context.identity.clone(),
            logs: self.logs.clone(),
            variables: self.context.variables.clone(),
        })
    }

    fn seed_trigger_payload_outputs(&mut self, trigger_node_id: &str) {
        match self.context.trigger_payload.clone() {
            Value::Object(payload) => {
                for (key, value) in payload {
                    self.set_variable(format!("{trigger_node_id}.{key}"), value);
                }
            }
            Value::Null => {}
            value => {
                self.set_variable(format!("{trigger_node_id}.payload"), value);
            }
        }
    }

    fn ensure_not_cancelled(&self) -> Result<(), RuntimeError> {
        if self.cancellation.is_cancelled() {
            Err(RuntimeError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn set_variable(&mut self, name: String, value: Value) {
        self.context.variables.insert(name.clone(), value);
        refresh_derived_variable_metadata(&mut self.context.variables, &name);
    }

    fn push_runtime_log(
        &mut self,
        level: &str,
        message: impl Into<String>,
        node_id: Option<String>,
    ) {
        self.logs.push(RuntimeLogEntry {
            level: level.to_owned(),
            message: message.into(),
            node_id,
            timestamp_unix_ms: contracts::unix_timestamp_millis_now(),
        });
    }
}
