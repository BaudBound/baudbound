use std::{io::IsTerminal, process::ExitCode, sync::Arc};

use anyhow::{Context, Result, anyhow};
use baudbound_actions::DesktopActionHandler;
use baudbound_core::{RunnerConfig, RunnerCore};
use baudbound_storage::{RunRetentionPolicy, SqliteRunnerStore};
use baudbound_triggers::{SerialPortRebindSink, WebSocketConnectionRegistry};
use clap::Parser;
use desktop_actions::SystemDesktopActionAdapter;

mod cli;
mod commands;
mod desktop_actions;
mod desktop_startup;
mod desktop_ui;
mod output;
mod paths;
mod secrets;
mod service;
mod time_format;
mod updates;
mod windows_console;

use cli::{Cli, Command};

fn main() -> ExitCode {
    let mut cli = Cli::parse();
    let command = cli.command.take().unwrap_or_else(cli::default_command);
    let desktop_launch = matches!(command, Command::Ui { .. });
    windows_console::detach_for_desktop_release(&command);

    match run(cli, command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if desktop_launch {
                desktop_startup::report_error(&error);
            } else {
                eprintln!("Error: {error:#}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli, command: Command) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let runner_home = paths::default_runner_home();
    let config_path = cli
        .config
        .clone()
        .unwrap_or_else(|| paths::default_config_path(&runner_home));
    let command = match command {
        Command::Config { command } => {
            return commands::config::handle_config_command(&config_path, command);
        }
        command => command,
    };

    let runner_config = RunnerConfig::load_or_init(&config_path)
        .with_context(|| format!("failed to load runner config {}", config_path.display()))?;
    let websocket_registry = Arc::new(WebSocketConnectionRegistry::new());
    let core = RunnerCore::from_config(&runner_config)
        .with_websocket_sink(Arc::clone(&websocket_registry));
    let action_handler = Arc::new(DesktopActionHandler::new(
        core.headless_action_handler(),
        SystemDesktopActionAdapter,
    ));
    let core = core.with_action_handler(action_handler);
    let secret_cipher = if matches!(&command, Command::Ui { .. }) {
        Some(secrets::desktop_secret_cipher()?)
    } else {
        secrets::headless_secret_cipher_from_environment()?
    };
    let mut store = SqliteRunnerStore::open(paths::default_database_path(&runner_home))
        .context("failed to open runner database")?;
    store
        .set_run_retention_policy(RunRetentionPolicy::new(
            runner_config.runner.run_history_max_records,
            runner_config.runner.run_history_max_age_days,
        ))
        .context("failed to apply run-history retention")?;
    if let Some(secret_cipher) = secret_cipher {
        store = store.with_secret_cipher(secret_cipher);
    }
    check_for_automatic_cli_update(&command, &runner_config, &store);

    dispatch_command(
        command,
        &config_path,
        &runner_config,
        &websocket_registry,
        &core,
        &store,
    )
}

fn check_for_automatic_cli_update(
    command: &Command,
    config: &RunnerConfig,
    store: &SqliteRunnerStore,
) {
    if !config.updates.automatic_checks
        || !std::io::stdout().is_terminal()
        || !matches!(
            command,
            Command::Status { json: false }
                | Command::Serve { json: false, .. }
                | Command::Validate { .. }
        )
    {
        return;
    }
    let due = match updates::check_is_due(store, config.updates.check_interval_hours) {
        Ok(due) => due,
        Err(error) => {
            tracing::debug!(%error, "failed to inspect update check schedule");
            return;
        }
    };
    if !due {
        return;
    }
    match updates::check_now(store) {
        Ok(result) if result.update_available => eprintln!(
            "BaudBound {} is available. Run `baudbound update check` for details.",
            result.latest_version
        ),
        Ok(_) => {}
        Err(error) => tracing::debug!(%error, "automatic update check failed"),
    }
}

fn dispatch_command(
    command: Command,
    config_path: &std::path::Path,
    runner_config: &RunnerConfig,
    websocket_registry: &Arc<WebSocketConnectionRegistry>,
    core: &RunnerCore,
    store: &SqliteRunnerStore,
) -> Result<()> {
    match command {
        Command::Config { .. } => unreachable!("config command returns before runner config loads"),
        Command::Status { json } => {
            commands::status::print_app_status(runner_config, core, store, json)
        }
        Command::Ui { autostart } => desktop_ui::run_desktop_ui(
            config_path.to_path_buf(),
            core.clone(),
            store.clone(),
            runner_config.clone(),
            Arc::clone(websocket_registry),
            autostart,
        ),
        Command::Doctor { json } => commands::doctor::print_desktop_doctor(json),
        Command::Validate { package } => commands::package::validate_package(core, package),
        Command::Inspect { target, json } => commands::package::inspect_package(core, target, json),
        Command::Serve {
            dry_run,
            json,
            once,
            run_schedules_immediately,
            hotkey_stdin,
            webhooks,
            webhook_bind,
            webhook_port,
            max_webhook_body_bytes,
            websockets,
            websocket_bind,
            websocket_port,
            max_websocket_message_bytes,
            max_websocket_connections,
            reload_interval_seconds,
        } => {
            let options = service::ServeOptions::from_config(
                runner_config,
                service::ServeOverrides {
                    hotkey_stdin,
                    max_webhook_body_bytes,
                    max_websocket_message_bytes,
                    max_websocket_connections,
                    webhook_bind,
                    webhook_port,
                    webhooks,
                    websocket_bind,
                    websocket_port,
                    websockets,
                    reload_interval_seconds,
                },
                once,
                run_schedules_immediately,
                Arc::clone(websocket_registry),
            )
            .with_serial_port_rebind_sink(Arc::new(service::RunnerConfigSerialPortRebindSink::new(
                config_path.to_path_buf(),
            )) as Arc<dyn SerialPortRebindSink>);
            if dry_run {
                service::print_serve_preflight(core, store, &options, json)
            } else if json {
                Err(anyhow!("serve --json is only supported with --dry-run"))
            } else {
                let update_worker = updates::AutomaticUpdateWorker::start(
                    store.clone(),
                    runner_config.updates.clone(),
                );
                let result = service::serve_triggers(core, store, options);
                drop(update_worker);
                result
            }
        }
        Command::Script { command } => {
            commands::script::handle_script_command(runner_config, core, store, command)
        }
        Command::Hotkey { command } => {
            commands::hotkey::handle_hotkey_command(core, store, command)
        }
        Command::Secret { command } => {
            commands::secret::handle_secret_command(core, store, command)
        }
        Command::TriggerAuth { command } => {
            commands::trigger_auth::handle_trigger_auth_command(core, store, command)
        }
        Command::Update { command } => match command {
            cli::UpdateCommand::Check { json } => commands::update::check(store, json),
        },
    }
}
