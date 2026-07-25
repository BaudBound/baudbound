use crate::runtime::{
    DERIVED_VARIABLE_METADATA_SUFFIXES, RuntimeFrame, RuntimeGraph,
    refresh_derived_variable_metadata,
};
use crate::{RuntimeCancellationToken, RuntimeStateStore};
use serde_json::Value;
use std::{io::Write, sync::Arc};

mod action_dispatch;
mod api;
mod branching;
mod contracts;
mod default_variables;
mod frames;
mod http_diagnostics;
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
    logs_truncated: bool,
    observer: Option<Arc<dyn RuntimeRunObserver>>,
    output_limits: RuntimeOutputLimits,
    retained_log_bytes: usize,
    action_handler: &'a dyn RuntimeActionHandler,
    cancellation: RuntimeCancellationToken,
    state_store: Option<&'a dyn RuntimeStateStore>,
    variable_scopes: std::collections::BTreeMap<String, RunVariableScope>,
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
        for (name, value) in &initial_state.variables {
            ensure_value_within_limit(
                name,
                value,
                resources.output_limits.max_runtime_variable_bytes,
            )?;
        }
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
            logs_truncated: false,
            observer: resources.observer,
            output_limits: resources.output_limits,
            retained_log_bytes: 0,
            action_handler: resources.action_handler,
            cancellation: resources.cancellation,
            state_store: resources.state_store,
            variable_scopes: initial_state.variable_scopes,
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
        self.seed_trigger_payload_outputs(&trigger_node_id)?;

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
            variable_scopes: self.variable_scopes.clone(),
            variables: self.context.variables.clone(),
        })
    }

    fn seed_trigger_payload_outputs(&mut self, trigger_node_id: &str) -> Result<(), RuntimeError> {
        match self.context.trigger_payload.clone() {
            Value::Object(payload) => {
                for (key, value) in payload {
                    self.set_variable(
                        format!("{trigger_node_id}.{key}"),
                        value,
                        RunVariableScope::NodeOutput,
                    )?;
                }
            }
            Value::Null => {}
            value => {
                self.set_variable(
                    format!("{trigger_node_id}.payload"),
                    value,
                    RunVariableScope::NodeOutput,
                )?;
            }
        }
        Ok(())
    }

    fn ensure_not_cancelled(&self) -> Result<(), RuntimeError> {
        if self.cancellation.is_cancelled() {
            Err(RuntimeError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn set_variable(
        &mut self,
        name: String,
        value: Value,
        scope: RunVariableScope,
    ) -> Result<(), RuntimeError> {
        ensure_value_within_limit(&name, &value, self.output_limits.max_runtime_variable_bytes)?;
        self.context.variables.insert(name.clone(), value);
        self.variable_scopes.insert(name.clone(), scope);
        refresh_derived_variable_metadata(&mut self.context.variables, &name);
        for suffix in DERIVED_VARIABLE_METADATA_SUFFIXES {
            self.variable_scopes
                .insert(format!("{name}{suffix}"), RunVariableScope::Metadata);
        }
        Ok(())
    }

    fn remove_variable(&mut self, name: &str) {
        self.context.variables.remove(name);
        self.variable_scopes.remove(name);
        refresh_derived_variable_metadata(&mut self.context.variables, name);
        for suffix in DERIVED_VARIABLE_METADATA_SUFFIXES {
            self.variable_scopes.remove(&format!("{name}{suffix}"));
        }
    }

    fn push_runtime_log(
        &mut self,
        level: &str,
        message: impl Into<String>,
        node_id: Option<String>,
    ) {
        let action_type = node_id.as_deref().and_then(|node_id| {
            self.graph
                .node(node_id)
                .ok()
                .map(|node| node.action_type.clone())
        });
        let message = truncate_utf8(
            &message.into(),
            self.output_limits.max_log_entry_bytes,
            "log entry",
        );
        let entry = RuntimeLogEntry {
            action_type,
            level: level.to_owned(),
            message,
            node_id,
            timestamp_unix_ms: contracts::unix_timestamp_millis_now(),
        };
        if let Some(observer) = &self.observer {
            let mut public_entry = entry.clone();
            public_entry.message = self.redact_text(&public_entry.message);
            observer.log_emitted(&self.context.identity, &public_entry);
        }
        let entry_bytes = retained_log_entry_bytes(&entry);
        if self.retained_log_bytes.saturating_add(entry_bytes)
            <= self.output_limits.max_run_log_bytes
        {
            self.retained_log_bytes = self.retained_log_bytes.saturating_add(entry_bytes);
            self.logs.push(entry);
        } else {
            self.retain_log_truncation_marker();
        }
    }

    fn retain_log_truncation_marker(&mut self) {
        if self.logs_truncated {
            return;
        }
        self.logs_truncated = true;
        let marker = RuntimeLogEntry {
            action_type: None,
            level: "warning".to_owned(),
            message: truncate_utf8(
                &format!(
                    "Additional runtime logs were not retained after the configured {} byte per-run limit was reached.",
                    self.output_limits.max_run_log_bytes
                ),
                self.output_limits.max_log_entry_bytes,
                "log entry",
            ),
            node_id: None,
            timestamp_unix_ms: contracts::unix_timestamp_millis_now(),
        };
        let marker_bytes = retained_log_entry_bytes(&marker);
        while self.retained_log_bytes.saturating_add(marker_bytes)
            > self.output_limits.max_run_log_bytes
        {
            let Some(removed) = self.logs.pop() else {
                break;
            };
            self.retained_log_bytes = self
                .retained_log_bytes
                .saturating_sub(retained_log_entry_bytes(&removed));
        }
        if marker_bytes <= self.output_limits.max_run_log_bytes {
            self.retained_log_bytes = self.retained_log_bytes.saturating_add(marker_bytes);
            if let Some(observer) = &self.observer {
                observer.log_emitted(&self.context.identity, &marker);
            }
            self.logs.push(marker);
        }
    }
}

fn ensure_value_within_limit(
    name: &str,
    value: &Value,
    max_bytes: usize,
) -> Result<(), RuntimeError> {
    let mut writer = BoundedSizeWriter::new(max_bytes);
    match serde_json::to_writer(&mut writer, value) {
        Ok(()) => Ok(()),
        Err(_) if writer.exceeded => Err(RuntimeError::State(format!(
            "variable {name:?} exceeds the configured {max_bytes} byte active value limit"
        ))),
        Err(source) => Err(RuntimeError::State(format!(
            "variable {name:?} could not be measured safely: {source}"
        ))),
    }
}

struct BoundedSizeWriter {
    bytes: usize,
    exceeded: bool,
    max_bytes: usize,
}

impl BoundedSizeWriter {
    const fn new(max_bytes: usize) -> Self {
        Self {
            bytes: 0,
            exceeded: false,
            max_bytes,
        }
    }
}

impl Write for BoundedSizeWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.bytes = self.bytes.saturating_add(buffer.len());
        if self.bytes > self.max_bytes {
            self.exceeded = true;
            return Err(std::io::Error::other("active value limit exceeded"));
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn retained_log_entry_bytes(entry: &RuntimeLogEntry) -> usize {
    entry
        .message
        .len()
        .saturating_add(entry.level.len())
        .saturating_add(entry.action_type.as_ref().map_or(0, String::len))
        .saturating_add(entry.node_id.as_ref().map_or(0, String::len))
        .saturating_add(std::mem::size_of::<u64>())
}

fn truncate_utf8(value: &str, max_bytes: usize, label: &str) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let marker = format!(" [TRUNCATED: {label} exceeded {max_bytes} bytes]");
    if marker.len() >= max_bytes {
        return marker.chars().take(max_bytes).collect();
    }
    let mut end = max_bytes - marker.len();
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{}", &value[..end], marker)
}
