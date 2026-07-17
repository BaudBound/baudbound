use anyhow::{Context, Result};
use baudbound_storage::{SharedSettings, SqliteRunnerStore, TimeFormat};

use crate::cli::{SetSettingCommand, SettingsCommand, TimeFormatValue};

pub fn handle_settings_command(store: &SqliteRunnerStore, command: SettingsCommand) -> Result<()> {
    match command {
        SettingsCommand::Show { json } => show_settings(store, json),
        SettingsCommand::Set { command } => set_setting(store, command),
    }
}

fn show_settings(store: &SqliteRunnerStore, json: bool) -> Result<()> {
    let shared = store
        .read_application_settings()
        .context("failed to read application settings")?
        .shared;
    if json {
        println!("{}", serde_json::to_string_pretty(&shared)?);
    } else {
        println!("Time format: {}", shared.time_format.as_str());
    }
    Ok(())
}

fn set_setting(store: &SqliteRunnerStore, command: SetSettingCommand) -> Result<()> {
    let shared = match command {
        SetSettingCommand::TimeFormat { value } => SharedSettings {
            time_format: match value {
                TimeFormatValue::TwelveHour => TimeFormat::TwelveHour,
                TimeFormatValue::TwentyFourHour => TimeFormat::TwentyFourHour,
            },
        },
    };
    store
        .write_shared_settings(&shared)
        .context("failed to save shared application settings")?;
    println!("Time format set to {}.", shared.time_format.as_str());
    Ok(())
}
