use anyhow::{Context, Result};
use baudbound_core::RunnerConfig;
use baudbound_storage::{ScriptStore, SqliteRunnerStore};

use crate::{output::print_run_record, time_format::CliTimeFormatter};

pub(super) fn print_logs(
    config: &RunnerConfig,
    store: &SqliteRunnerStore,
    script: Option<String>,
    limit: usize,
    json: bool,
) -> Result<()> {
    let records = store
        .list_run_records(script.as_deref(), Some(limit))
        .context("failed to list run logs")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&records)?);
    } else if records.is_empty() {
        println!("No run logs found.");
    } else {
        let time = CliTimeFormatter::from_config(config);
        for record in records {
            print_run_record(&record, time)?;
        }
    }
    Ok(())
}
