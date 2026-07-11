use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "baudbound")]
#[command(about = "BaudBound runner")]
#[command(version)]
pub struct Cli {
    /// Path to runner TOML configuration. Defaults to BAUDBOUND_CONFIG or <BAUDBOUND_HOME>/config.toml.
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print or initialize runner TOML configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Show runner status. This is also used when no command is provided.
    Status {
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Open the native desktop runner UI.
    Ui,
    /// Check native desktop action support on this machine.
    Doctor {
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Validate a .bbs package without importing it.
    Validate {
        /// Path to the .bbs package.
        package: PathBuf,
    },
    /// Inspect package metadata and contents.
    Inspect {
        /// Path to a .bbs package.
        target: String,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run long-lived trigger listeners.
    Serve {
        /// Print listener preflight status and exit without starting services.
        #[arg(long)]
        dry_run: bool,
        /// Print machine-readable JSON for --dry-run.
        #[arg(long)]
        json: bool,
        /// Stop after the first due schedule batch.
        #[arg(long)]
        once: bool,
        /// Dispatch all schedule triggers once immediately before waiting for intervals.
        #[arg(long)]
        run_schedules_immediately: bool,
        /// Read hotkey expressions from stdin and dispatch matching desktop hotkey triggers.
        #[arg(long)]
        hotkey_stdin: bool,
        /// Enable local webhook trigger listener.
        #[arg(long)]
        webhooks: bool,
        /// Webhook listener bind address.
        #[arg(long)]
        webhook_bind: Option<String>,
        /// Webhook listener port.
        #[arg(long)]
        webhook_port: Option<u16>,
        /// Maximum webhook request body size in bytes.
        #[arg(long, value_parser = parse_positive_usize)]
        max_webhook_body_bytes: Option<usize>,
        /// Enable local WebSocket trigger listener.
        #[arg(long)]
        websockets: bool,
        /// WebSocket listener bind address.
        #[arg(long)]
        websocket_bind: Option<String>,
        /// WebSocket listener port.
        #[arg(long)]
        websocket_port: Option<u16>,
        /// Maximum WebSocket message size in bytes.
        #[arg(long, value_parser = parse_positive_usize)]
        max_websocket_message_bytes: Option<usize>,
        /// Maximum number of concurrent WebSocket connections.
        #[arg(long, value_parser = parse_positive_usize)]
        max_websocket_connections: Option<usize>,
        /// Seconds between installed trigger registration reload checks.
        #[arg(long)]
        reload_interval_seconds: Option<u64>,
    },
    /// Installed script lifecycle and execution commands.
    Script {
        #[command(subcommand)]
        command: ScriptCommand,
    },
    /// Inspect or dispatch desktop hotkey trigger registrations.
    Hotkey {
        #[command(subcommand)]
        command: HotkeyCommand,
    },
    /// Configure encrypted values for declared script secrets.
    Secret {
        #[command(subcommand)]
        command: SecretCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum SecretCommand {
    /// Generate a new base64 key for BAUDBOUND_SECRET_KEY.
    GenerateKey,
    /// List declared secrets and whether each has a configured value.
    List {
        script: String,
        #[arg(long)]
        json: bool,
    },
    /// Prompt securely for a declared secret value.
    Set { script: String, name: String },
    /// Remove a configured secret value.
    Remove { script: String, name: String },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the resolved config path.
    Path,
    /// Print a complete example runner.toml.
    Print,
    /// Write a starter config file to the resolved config path.
    Init {
        /// Overwrite an existing config file.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum HotkeyCommand {
    /// List enabled hotkey trigger bindings.
    List {
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Dispatch a hotkey event into matching enabled scripts.
    Dispatch {
        /// Hotkey expression, for example Ctrl+Alt+B.
        key: String,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Listen for newline-delimited hotkey events from stdin and dispatch matches.
    Listen {
        /// Read hotkey expressions from stdin. Native OS hooks will use this dispatch path later.
        #[arg(long)]
        stdin: bool,
        /// Print one machine-readable JSON object per input event.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ScriptCommand {
    /// Import a package into local runner storage.
    Import {
        /// Path to the .bbs package.
        package: PathBuf,
    },
    /// Update an installed script from a new .bbs package with the same manifest id.
    Update {
        /// Path to the replacement .bbs package.
        package: PathBuf,
    },
    /// List installed scripts.
    List {
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show installed script health.
    Status {
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Inspect one installed script.
    Inspect {
        /// Installed script id or name.
        script: String,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Enable an installed script for long-lived trigger services.
    Enable {
        /// Installed script id or name.
        script: String,
    },
    /// Disable an installed script without removing it.
    Disable {
        /// Installed script id or name.
        script: String,
    },
    /// Remove an installed script.
    Remove {
        /// Installed script id or name.
        script: String,
    },
    /// Show the current approval for an installed script.
    Approval {
        /// Installed script id or name.
        script: String,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// List trigger registrations for installed scripts.
    Triggers {
        /// Optional installed script id or name to filter by.
        script: Option<String>,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Dispatch a trigger event into an installed script.
    DispatchTrigger {
        /// Installed script id or name.
        script: String,
        /// Trigger node id to start from.
        trigger: String,
        /// JSON payload exposed as trigger node output data.
        #[arg(long)]
        payload_json: Option<String>,
    },
    /// Approve an installed script for its current package hash and declared permissions.
    Approve {
        /// Installed script id or name.
        script: String,
    },
    /// Revoke an installed script approval.
    RevokeApproval {
        /// Installed script id or name.
        script: String,
    },
    /// Run an installed script by id or name.
    Run {
        /// Installed script id or name.
        script: String,
        /// Trigger node id to start from. Defaults to the script manual trigger.
        #[arg(long)]
        trigger: Option<String>,
        /// JSON payload exposed as trigger node output data.
        #[arg(long)]
        payload_json: Option<String>,
    },
    /// Show stored runner run history.
    Logs {
        /// Installed script id or name to filter by.
        #[arg(long)]
        script: Option<String>,
        /// Maximum number of runs to show.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("expected a positive integer, found {value:?}"))?;
    if parsed == 0 {
        return Err("value must be greater than zero".to_owned());
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command, SecretCommand, parse_positive_usize};

    #[test]
    fn positive_size_parser_rejects_zero_and_invalid_values() {
        assert_eq!(parse_positive_usize("1").expect("one is positive"), 1);
        assert!(parse_positive_usize("0").is_err());
        assert!(parse_positive_usize("invalid").is_err());
    }

    #[test]
    fn parses_grouped_secret_management_commands() {
        let cli = Cli::try_parse_from(["baudbound", "secret", "list", "script-1", "--json"])
            .expect("secret list command should parse");
        assert!(matches!(
            cli.command,
            Some(Command::Secret {
                command: SecretCommand::List { script, json: true }
            }) if script == "script-1"
        ));
    }
}
