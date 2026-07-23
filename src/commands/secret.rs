use anyhow::{Context, Result};
use baudbound_core::RunnerCore;
use baudbound_storage::SqliteRunnerStore;

use crate::cli::SecretCommand;

pub fn handle_secret_command(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    command: SecretCommand,
) -> Result<()> {
    match command {
        SecretCommand::GenerateKey => {
            println!(
                "BAUDBOUND_SECRET_KEY={}",
                crate::secrets::generate_environment_secret_key()?
            );
        }
        SecretCommand::List { script, json } => {
            let secrets = core.list_installed_secrets(store, &script)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&secrets)?);
            } else if secrets.is_empty() {
                println!("No secrets are declared by {script}.");
            } else {
                for secret in secrets {
                    println!(
                        "{}  type={}  required={}  configured={}",
                        secret.name, secret.value_type, secret.required, secret.configured
                    );
                }
            }
        }
        SecretCommand::Set { script, name } => {
            let value = rpassword::prompt_password(format!("Value for {name}: "))
                .context("failed to read secret value from the terminal")?;
            core.set_installed_secret_from_text(store, &script, &name, &value)?;
            println!("Configured {name} for {script}.");
        }
        SecretCommand::Remove { script, name } => {
            if core.remove_installed_secret(store, &script, &name)? {
                println!("Removed {name} from {script}.");
            } else {
                println!("{name} was not configured for {script}.");
            }
        }
    }
    Ok(())
}
