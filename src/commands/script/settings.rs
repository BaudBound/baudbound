use anyhow::Result;
use baudbound_core::RunnerCore;
use baudbound_storage::SqliteRunnerStore;

use crate::cli::ScriptSettingsCommand;

pub(super) fn handle_settings_command(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    command: ScriptSettingsCommand,
) -> Result<()> {
    match command {
        ScriptSettingsCommand::List { script, json } => {
            let settings = core.list_installed_script_settings(store, &script)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&settings)?);
            } else if settings.is_empty() {
                println!("No Script Settings are declared by {script}.");
            } else {
                for setting in settings {
                    println!(
                        "{}  type={}  required={}  configured={}  effective={}",
                        setting.name,
                        setting.value_type,
                        setting.required,
                        setting.configured,
                        setting
                            .effective_value
                            .as_ref()
                            .map_or_else(|| "missing".to_owned(), serde_json::Value::to_string)
                    );
                }
            }
        }
        ScriptSettingsCommand::Set {
            script,
            name,
            value,
        } => {
            core.set_installed_script_setting_from_text(store, &script, &name, &value)?;
            println!("Configured {name} for {script}.");
        }
        ScriptSettingsCommand::Unset { script, name } => {
            if core.remove_installed_script_setting(store, &script, &name)? {
                println!("Reset {name} to its package default for {script}.");
            } else {
                println!("{name} did not have a configured override for {script}.");
            }
        }
        ScriptSettingsCommand::Reset { script } => {
            let configured = core
                .list_installed_script_settings(store, &script)?
                .into_iter()
                .filter(|setting| setting.configured)
                .map(|setting| setting.name)
                .collect::<Vec<_>>();
            for name in &configured {
                core.remove_installed_script_setting(store, &script, name)?;
            }
            println!(
                "Reset {} configured Script Setting override{} for {script}.",
                configured.len(),
                if configured.len() == 1 { "" } else { "s" }
            );
        }
    }
    Ok(())
}
