use std::path::Path;

use anyhow::{Context, Result, anyhow};
use baudbound_core::RunnerConfig;

use crate::cli::ConfigCommand;

pub fn handle_config_command(config_path: &Path, command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Path => {
            println!("{}", config_path.display());
        }
        ConfigCommand::Print => {
            print!("{}", RunnerConfig::template_toml());
        }
        ConfigCommand::Init { force } => {
            write_config_template(config_path, force)?;
            println!("Wrote runner config template to {}", config_path.display());
        }
    }

    Ok(())
}

fn write_config_template(config_path: &Path, force: bool) -> Result<()> {
    if config_path.exists() && !force {
        return Err(anyhow!(
            "config file already exists at {}; pass --force to overwrite",
            config_path.display()
        ));
    }

    RunnerConfig::write_template(config_path)
        .with_context(|| format!("failed to write config file {}", config_path.display()))
}
