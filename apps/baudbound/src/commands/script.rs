use anyhow::Result;
use baudbound_core::RunnerCore;
use baudbound_storage::SqliteRunnerStore;

use crate::cli::ScriptCommand;

use super::status::{print_script_status, print_trigger_registrations_for_script};

mod approval;
mod execution;
mod lifecycle;
mod logs;

pub fn handle_script_command(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    command: ScriptCommand,
) -> Result<()> {
    match command {
        ScriptCommand::Import { package } => {
            lifecycle::import_script(core, store, package)?;
        }
        ScriptCommand::Update { package } => {
            lifecycle::update_script(core, store, package)?;
        }
        ScriptCommand::List { json } => {
            lifecycle::list_scripts(core, store, json)?;
        }
        ScriptCommand::Status { json } => {
            print_script_status(core, store, json)?;
        }
        ScriptCommand::Inspect { script, json } => {
            lifecycle::inspect_script(core, store, script, json)?;
        }
        ScriptCommand::Enable { script } => {
            lifecycle::set_script_enabled(core, store, script, true)?;
        }
        ScriptCommand::Disable { script } => {
            lifecycle::set_script_enabled(core, store, script, false)?;
        }
        ScriptCommand::Remove { script } => {
            lifecycle::remove_script(core, store, script)?;
        }
        ScriptCommand::Approval { script, json } => {
            approval::print_approval(store, script, json)?;
        }
        ScriptCommand::Triggers { script, json } => {
            print_trigger_registrations_for_script(core, store, script.as_deref(), json)?;
        }
        ScriptCommand::DispatchTrigger {
            script,
            trigger,
            payload_json,
        } => {
            execution::dispatch_trigger_command(core, store, script, trigger, payload_json)?;
        }
        ScriptCommand::Approve { script } => {
            approval::approve_script(core, store, script)?;
        }
        ScriptCommand::RevokeApproval { script } => {
            approval::revoke_approval(core, store, script)?;
        }
        ScriptCommand::Run {
            script,
            trigger,
            payload_json,
        } => {
            execution::run_script(core, store, script, trigger, payload_json)?;
        }
        ScriptCommand::Logs {
            script,
            limit,
            json,
        } => {
            logs::print_logs(store, script, limit, json)?;
        }
    }

    Ok(())
}
