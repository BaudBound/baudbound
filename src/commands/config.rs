use std::path::Path;

use anyhow::{Context, Result, anyhow};
use baudbound_core::{RunnerConfig, TimeFormat};

use crate::cli::{ConfigCommand, ConfigKey, TimeFormatValue};

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
        ConfigCommand::Set { key, value } => {
            let mut config = RunnerConfig::load_or_init(config_path)
                .with_context(|| format!("failed to load config file {}", config_path.display()))?;
            match key {
                ConfigKey::DisplayTimeFormat => {
                    config.display.time_format = match value {
                        TimeFormatValue::TwelveHour => TimeFormat::TwelveHour,
                        TimeFormatValue::TwentyFourHour => TimeFormat::TwentyFourHour,
                    };
                }
            }
            config
                .save(config_path)
                .with_context(|| format!("failed to save config file {}", config_path.display()))?;
            println!(
                "Set display.time-format to {}.",
                config.display.time_format.as_str()
            );
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
