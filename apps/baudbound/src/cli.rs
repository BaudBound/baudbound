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
        #[arg(long)]
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
        #[arg(long)]
        max_websocket_message_bytes: Option<usize>,
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
