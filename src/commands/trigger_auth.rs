use anyhow::{Result, bail};
use baudbound_core::RunnerCore;
use baudbound_storage::{NetworkTriggerType, SqliteRunnerStore};

use crate::cli::{NetworkTriggerTypeValue, TriggerAuthCommand};

pub fn handle_trigger_auth_command(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    command: TriggerAuthCommand,
) -> Result<()> {
    match command {
        TriggerAuthCommand::List { script, json } => {
            let statuses = core.list_trigger_auth(store, &script)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&statuses)?);
            } else if statuses.is_empty() {
                println!("{script} has no webhook or WebSocket trigger authentication records.");
            } else {
                for status in statuses {
                    println!(
                        "{}  type={}  protected={}  token_ends_in={}",
                        status.node_id,
                        trigger_type_label(&status.trigger_type),
                        status.auth_enabled,
                        status.token_preview
                    );
                }
            }
        }
        TriggerAuthCommand::Rotate {
            script,
            node_id,
            trigger_type,
            json,
        } => {
            let generated =
                core.rotate_trigger_token(store, &script, &node_id, trigger_type.into())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&generated)?);
            } else {
                println!("Token: {}", generated.token);
                println!(
                    "Save this token now. It cannot be shown again. Generate a new token if it is lost."
                );
            }
        }
        TriggerAuthCommand::Enable {
            script,
            node_id,
            trigger_type,
        } => {
            core.set_trigger_auth_enabled(store, &script, &node_id, trigger_type.into(), true)?;
            println!("Enabled authentication for {script}:{node_id}.");
        }
        TriggerAuthCommand::Disable {
            script,
            node_id,
            trigger_type,
            yes,
        } => {
            if !yes {
                bail!(
                    "refusing to disable authentication without --yes; callers that can reach the listener will be able to trigger this script without a token"
                );
            }
            core.set_trigger_auth_enabled(store, &script, &node_id, trigger_type.into(), false)?;
            println!("Disabled authentication for {script}:{node_id}.");
        }
    }
    Ok(())
}

impl From<NetworkTriggerTypeValue> for NetworkTriggerType {
    fn from(value: NetworkTriggerTypeValue) -> Self {
        match value {
            NetworkTriggerTypeValue::Webhook => Self::Webhook,
            NetworkTriggerTypeValue::Websocket => Self::Websocket,
        }
    }
}

fn trigger_type_label(trigger_type: &NetworkTriggerType) -> &'static str {
    match trigger_type {
        NetworkTriggerType::Webhook => "webhook",
        NetworkTriggerType::Websocket => "websocket",
    }
}
