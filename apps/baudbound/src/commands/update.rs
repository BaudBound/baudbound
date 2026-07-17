use anyhow::Result;
use baudbound_storage::SqliteRunnerStore;

use crate::updates;

pub fn check(store: &SqliteRunnerStore, json: bool) -> Result<()> {
    let result = updates::check_now(store)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else if result.update_available {
        println!(
            "BaudBound {} is available. You are running {}.",
            result.latest_version, result.current_version
        );
        println!("Download it from https://github.com/NATroutter/BaudBound/releases/latest");
    } else {
        println!("BaudBound {} is up to date.", result.current_version);
    }
    Ok(())
}
