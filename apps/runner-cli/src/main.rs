use std::{
    collections::BTreeMap,
    io::Read,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context, Result, anyhow};
use baudbound_core::{
    RunReport, RunnerConfig, RunnerCore, TriggerEvent, TriggerRegistration,
    serial_device_configs_from_settings,
};
use baudbound_storage::{
    FilesystemScriptStore, InstalledScript, ScriptApproval, ScriptStore, StoredRunRecord,
};
use baudbound_triggers::{
    FileWatchService, ScheduleService, SerialDeviceConfig as TriggerSerialDeviceConfig,
    SerialInputService, WebhookRequest, WebhookResponse, WebhookService,
};
use clap::{Parser, Subcommand};
use tiny_http::{Header, Request, Response, Server, StatusCode};

#[derive(Debug, Parser)]
#[command(name = "baudbound")]
#[command(about = "BaudBound command-line runner")]
#[command(version)]
struct Cli {
    /// Path to runner TOML configuration. Defaults to BAUDBOUND_CONFIG or <BAUDBOUND_HOME>/config.toml.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate a .bbs package without importing it.
    Validate {
        /// Path to the .bbs package.
        package: PathBuf,
    },
    /// Inspect package metadata and contents.
    Inspect {
        /// Path to a .bbs package, or installed script id/name when --installed is used.
        target: String,
        /// Inspect an installed script instead of a package file.
        #[arg(long)]
        installed: bool,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
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
    /// Remove an installed script.
    Remove {
        /// Installed script id or name.
        script: String,
    },
    /// Approve an installed script for its current package hash and declared permissions.
    Approve {
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
    /// List trigger registrations discovered from installed scripts.
    Triggers {
        /// Installed script id or name to filter by.
        #[arg(long)]
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
    /// Run long-lived trigger listeners.
    Serve {
        /// Stop after the first due schedule batch.
        #[arg(long)]
        once: bool,
        /// Dispatch all schedule triggers once immediately before waiting for intervals.
        #[arg(long)]
        run_schedules_immediately: bool,
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

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let runner_home = default_runner_home();
    let config_path = cli
        .config
        .clone()
        .unwrap_or_else(|| default_config_path(&runner_home));
    let runner_config = RunnerConfig::load_or_default(&config_path)
        .with_context(|| format!("failed to load runner config {}", config_path.display()))?;
    let core = RunnerCore::from_config(&runner_config);
    let store = FilesystemScriptStore::new(runner_home);

    match cli.command {
        Command::Validate { package } => {
            let summary = core
                .validate_package(&package)
                .with_context(|| format!("failed to validate {}", package.display()))?;
            println!(
                "Package valid: {} (package v{}, runtime v{}, {}, {} asset{})",
                summary.script_name,
                summary.package_format_version,
                summary.script_language_version,
                summary.target_runtime,
                summary.asset_count,
                if summary.asset_count == 1 { "" } else { "s" }
            );
        }
        Command::Inspect {
            target,
            installed,
            json,
        } => {
            if installed {
                let script = core
                    .inspect_installed(&store, &target)
                    .with_context(|| format!("failed to inspect installed script {target:?}"))?;

                if json {
                    println!("{}", serde_json::to_string_pretty(&script)?);
                } else {
                    print_installed_script(&script);
                }
            } else {
                let package = PathBuf::from(&target);
                let inspection = core
                    .inspect_package(&package)
                    .with_context(|| format!("failed to inspect {}", package.display()))?;

                if json {
                    let output = serde_json::json!({
                        "summary": inspection.summary,
                        "entries": inspection.entries,
                    });
                    println!("{}", serde_json::to_string_pretty(&output)?);
                } else {
                    println!("Script: {}", inspection.summary.script_name);
                    println!("Target runtime: {}", inspection.summary.target_runtime);
                    println!(
                        "Package version: {}",
                        inspection.summary.package_format_version
                    );
                    println!(
                        "Runtime version: {}",
                        inspection.summary.script_language_version
                    );
                    println!("Files:");
                    for entry in inspection.entries {
                        println!("  - {entry}");
                    }
                }
            }
        }
        Command::Import { package } => {
            let script = core
                .import_package(&store, &package)
                .with_context(|| format!("failed to import {}", package.display()))?;
            println!(
                "Imported {} ({}) as {} into {}",
                script.name,
                script.id,
                script.package_file_name,
                store.root().display()
            );
        }
        Command::Update { package } => {
            let script = core
                .update_package(&store, &package)
                .with_context(|| format!("failed to update from {}", package.display()))?;
            println!(
                "Updated {} ({}) as {}",
                script.name, script.id, script.package_file_name
            );
        }
        Command::List { json } => {
            let scripts = core
                .list_installed(&store)
                .context("failed to list installed scripts")?;
            if json {
                println!("{}", serde_json::to_string_pretty(&scripts)?);
            } else if scripts.is_empty() {
                println!("No scripts installed.");
            } else {
                for script in scripts {
                    println!(
                        "{}  {}  {}  {}",
                        script.id,
                        if script.enabled {
                            "enabled "
                        } else {
                            "disabled"
                        },
                        script.risk_level,
                        script.name
                    );
                }
            }
        }
        Command::Remove { script } => {
            let removed = core
                .remove_installed(&store, &script)
                .with_context(|| format!("failed to remove installed script {script:?}"))?;
            println!("Removed {} ({})", removed.name, removed.id);
        }
        Command::Approve { script } => {
            let approval = core
                .approve_installed(&store, &script)
                .with_context(|| format!("failed to approve installed script {script:?}"))?;
            println!(
                "Approved {} for package {}",
                approval.script_id, approval.package_hash
            );
            print_approval_permissions(&approval);
        }
        Command::Approval { script, json } => {
            let approval = store
                .find_script_approval(&script)
                .with_context(|| format!("failed to inspect approval for {script:?}"))?;
            match (approval, json) {
                (Some(approval), true) => {
                    println!("{}", serde_json::to_string_pretty(&approval)?);
                }
                (Some(approval), false) => {
                    println!("Approved script: {}", approval.script_id);
                    println!("Package hash: {}", approval.package_hash);
                    println!("Approved at: {}", approval.approved_at_unix);
                    print_approval_permissions(&approval);
                }
                (None, true) => println!("null"),
                (None, false) => println!("No approval stored for {script:?}."),
            }
        }
        Command::Triggers { script, json } => {
            let registrations = core
                .list_trigger_registrations(&store, script.as_deref())
                .context("failed to list trigger registrations")?;
            if json {
                println!("{}", serde_json::to_string_pretty(&registrations)?);
            } else if registrations.is_empty() {
                println!("No trigger registrations found.");
            } else {
                print_trigger_registrations(&registrations);
            }
        }
        Command::DispatchTrigger {
            script,
            trigger,
            payload_json,
        } => {
            let payload = parse_payload_json(payload_json)?;
            let installed = core
                .inspect_installed(&store, &script)
                .with_context(|| format!("failed to resolve installed script {script:?}"))?;
            let report = core
                .dispatch_trigger_event(
                    &store,
                    TriggerEvent {
                        node_id: trigger,
                        payload,
                        script_id: installed.id,
                    },
                )
                .with_context(|| format!("failed to dispatch trigger event for {script:?}"))?;
            print_run_report(report);
        }
        Command::Serve {
            once,
            run_schedules_immediately,
            webhooks,
            webhook_bind,
            webhook_port,
            max_webhook_body_bytes,
        } => serve_triggers(
            &core,
            &store,
            ServeOptions::from_config(
                &runner_config,
                ServeOverrides {
                    max_webhook_body_bytes,
                    webhook_bind,
                    webhook_port,
                    webhooks,
                },
                once,
                run_schedules_immediately,
            ),
        )?,
        Command::RevokeApproval { script } => {
            let revoked = core
                .revoke_approval(&store, &script)
                .with_context(|| format!("failed to revoke approval for {script:?}"))?;
            if let Some(approval) = revoked {
                println!("Revoked approval for {}", approval.script_id);
            } else {
                println!("No approval was stored for {script:?}.");
            }
        }
        Command::Run {
            script,
            trigger,
            payload_json,
        } => {
            let payload = parse_payload_json(payload_json)?;
            let report = core
                .run_installed_with_trigger(&store, &script, trigger.as_deref(), payload)
                .with_context(|| format!("failed to run installed script {script:?}"))?;
            print_run_report(report);
        }
        Command::Logs {
            script,
            limit,
            json,
        } => {
            let records = store
                .list_run_records(script.as_deref(), Some(limit))
                .context("failed to list run logs")?;
            if json {
                println!("{}", serde_json::to_string_pretty(&records)?);
            } else if records.is_empty() {
                println!("No run logs found.");
            } else {
                for record in records {
                    print_run_record(&record);
                }
            }
        }
    }

    Ok(())
}

struct ServeOptions {
    file_watch_enabled: bool,
    max_webhook_body_bytes: usize,
    once: bool,
    run_schedules_immediately: bool,
    schedules_enabled: bool,
    serial_enabled: bool,
    serial_devices: Vec<TriggerSerialDeviceConfig>,
    webhook_bind: String,
    webhook_port: u16,
    webhooks_enabled: bool,
}

struct ServeOverrides {
    max_webhook_body_bytes: Option<usize>,
    webhook_bind: Option<String>,
    webhook_port: Option<u16>,
    webhooks: bool,
}

impl ServeOptions {
    fn from_config(
        config: &RunnerConfig,
        overrides: ServeOverrides,
        once: bool,
        run_schedules_immediately: bool,
    ) -> Self {
        Self {
            file_watch_enabled: config.triggers.file_watch_enabled,
            max_webhook_body_bytes: overrides
                .max_webhook_body_bytes
                .unwrap_or(config.webhooks.max_body_bytes)
                .max(1),
            once,
            run_schedules_immediately,
            schedules_enabled: config.triggers.schedules_enabled,
            serial_enabled: config.triggers.serial_enabled,
            serial_devices: serial_device_configs_from_settings(&config.serial.devices)
                .into_iter()
                .map(|device| TriggerSerialDeviceConfig {
                    auto_reconnect: device.auto_reconnect,
                    baud_rate: device.baud_rate,
                    data_bits: device.data_bits,
                    device_id: device.device_id,
                    flow_control: device.flow_control,
                    parity: device.parity,
                    port: device.port,
                    product_id: device.product_id,
                    read_mode: device.read_mode,
                    stop_bits: device.stop_bits,
                    validate_usb_identity: device.validate_usb_identity,
                    vendor_id: device.vendor_id,
                })
                .collect(),
            webhook_bind: overrides
                .webhook_bind
                .unwrap_or_else(|| config.webhooks.bind.clone()),
            webhook_port: overrides.webhook_port.unwrap_or(config.webhooks.port),
            webhooks_enabled: config.triggers.webhooks_enabled || overrides.webhooks,
        }
    }
}

fn serve_triggers(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    options: ServeOptions,
) -> Result<()> {
    const MAX_IDLE_SLEEP: Duration = Duration::from_secs(1);
    const RELOAD_CHECK_INTERVAL: Duration = Duration::from_secs(2);

    let (trigger_sender, trigger_receiver) = mpsc::channel();
    let mut services = load_trigger_services(core, store, &options, &trigger_sender, None)?;

    if services.is_idle() {
        if options.schedules_enabled {
            println!("No enabled schedule triggers found.");
        } else {
            println!("Schedule triggers are disabled in runner config.");
        }
        if options.file_watch_enabled {
            println!("No enabled file watch triggers found.");
        } else {
            println!("File watch triggers are disabled in runner config.");
        }
        if !options.webhooks_enabled {
            println!("Webhook listener is disabled. Enable it in config or pass --webhooks.");
        }
        if !options.schedules_enabled
            && !options.file_watch_enabled
            && !options.serial_enabled
            && !options.webhooks_enabled
        {
            return Ok(());
        }
    }

    if options.run_schedules_immediately {
        services.schedules.mark_all_due_now(Instant::now());
    }

    print_service_summary(&services, store);
    println!("Press Ctrl+C to stop.");

    let mut next_reload_check = Instant::now() + RELOAD_CHECK_INTERVAL;
    let mut dispatched_any_event = false;
    loop {
        if Instant::now() >= next_reload_check {
            let (reloaded_services, did_reload) = reload_trigger_services_if_changed(
                core,
                store,
                &options,
                &trigger_sender,
                services,
            )?;
            if did_reload {
                println!("Trigger registrations changed. Reloaded listener services.");
                print_service_summary(&reloaded_services, store);
                if reloaded_services.is_idle() {
                    println!("No trigger listener services are currently active.");
                }
            }
            services = reloaded_services;
            next_reload_check = Instant::now() + RELOAD_CHECK_INTERVAL;
        }

        dispatched_any_event |= dispatch_due_schedules(core, store, &mut services.schedules);
        dispatched_any_event |= dispatch_trigger_events(core, store, &trigger_receiver);

        if options.once && dispatched_any_event {
            return Ok(());
        }

        let wait_duration = services
            .schedules
            .time_until_next_due(Instant::now())
            .unwrap_or(MAX_IDLE_SLEEP)
            .min(MAX_IDLE_SLEEP)
            .min(next_reload_check.saturating_duration_since(Instant::now()));

        if let Some(host) = &services.webhook_host {
            match host.server.recv_timeout(wait_duration) {
                Ok(Some(request)) => {
                    dispatched_any_event = true;
                    handle_webhook_request(
                        core,
                        store,
                        &host.service,
                        request,
                        options.max_webhook_body_bytes,
                    )?;
                }
                Ok(None) => {}
                Err(error) => return Err(anyhow!("webhook listener failed: {error}")),
            }
        } else if !services.file_watch_service.is_empty()
            || !services.serial_input_service.is_empty()
        {
            match trigger_receiver.recv_timeout(wait_duration) {
                Ok(event) => {
                    dispatched_any_event = true;
                    dispatch_trigger_event(core, store, event);
                    dispatched_any_event |= dispatch_trigger_events(core, store, &trigger_receiver);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(anyhow!("trigger listener channel disconnected"));
                }
            }
        } else {
            thread::sleep(wait_duration);
        }
    }
}

struct TriggerServices {
    file_watch_service: FileWatchService,
    registrations_fingerprint: String,
    schedules: ScheduleService,
    serial_input_service: SerialInputService,
    webhook_host: Option<WebhookHost>,
}

impl TriggerServices {
    fn is_idle(&self) -> bool {
        self.schedules.is_empty()
            && self.file_watch_service.is_empty()
            && self.serial_input_service.is_empty()
            && self.webhook_host.is_none()
    }
}

struct WebhookHost {
    server: Server,
    service: WebhookService,
}

fn load_trigger_services(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    options: &ServeOptions,
    trigger_sender: &Sender<TriggerEvent>,
    previous_webhook_host: Option<WebhookHost>,
) -> Result<TriggerServices> {
    let registrations = core
        .list_trigger_registrations(store, None)
        .context("failed to load trigger registrations")?;
    let registrations_fingerprint = serde_json::to_string(&registrations)
        .context("failed to fingerprint trigger registrations")?;

    build_trigger_services(
        registrations,
        registrations_fingerprint,
        options,
        trigger_sender,
        previous_webhook_host,
    )
}

fn reload_trigger_services_if_changed(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    options: &ServeOptions,
    trigger_sender: &Sender<TriggerEvent>,
    mut current: TriggerServices,
) -> Result<(TriggerServices, bool)> {
    let registrations = core
        .list_trigger_registrations(store, None)
        .context("failed to reload trigger registrations")?;
    let registrations_fingerprint = serde_json::to_string(&registrations)
        .context("failed to fingerprint trigger registrations")?;

    if registrations_fingerprint == current.registrations_fingerprint {
        return Ok((current, false));
    }

    let previous_webhook_host = current.webhook_host.take();
    build_trigger_services(
        registrations,
        registrations_fingerprint,
        options,
        trigger_sender,
        previous_webhook_host,
    )
    .map(|services| (services, true))
}

fn build_trigger_services(
    registrations: Vec<TriggerRegistration>,
    registrations_fingerprint: String,
    options: &ServeOptions,
    trigger_sender: &Sender<TriggerEvent>,
    previous_webhook_host: Option<WebhookHost>,
) -> Result<TriggerServices> {
    let schedules = if options.schedules_enabled {
        ScheduleService::from_registrations(registrations.clone(), Instant::now())
            .context("failed to register schedule triggers")?
    } else {
        ScheduleService::empty()
    };
    let file_watch_service = if options.file_watch_enabled {
        FileWatchService::start(registrations.clone(), trigger_sender.clone())
            .context("failed to register file watch triggers")?
    } else {
        FileWatchService::empty()
    };
    let serial_input_service = if options.serial_enabled {
        SerialInputService::start(
            registrations.clone(),
            options.serial_devices.clone(),
            trigger_sender.clone(),
        )
        .context("failed to register serial input triggers")?
    } else {
        SerialInputService::empty()
    };
    let webhook_host = build_webhook_host(registrations, options, previous_webhook_host)?;

    Ok(TriggerServices {
        file_watch_service,
        registrations_fingerprint,
        schedules,
        serial_input_service,
        webhook_host,
    })
}

fn build_webhook_host(
    registrations: Vec<TriggerRegistration>,
    options: &ServeOptions,
    previous_webhook_host: Option<WebhookHost>,
) -> Result<Option<WebhookHost>> {
    if !options.webhooks_enabled {
        return Ok(None);
    }

    let service = WebhookService::from_registrations(registrations)
        .context("failed to register webhook triggers")?;
    if service.is_empty() {
        println!("No enabled webhook triggers found.");
        return Ok(None);
    }

    if let Some(mut host) = previous_webhook_host {
        host.service = service;
        return Ok(Some(host));
    }

    let address = format!("{}:{}", options.webhook_bind, options.webhook_port);
    let server = Server::http(&address)
        .map_err(|error| anyhow!("failed to bind webhook listener on {address}: {error}"))?;
    println!(
        "Serving {} webhook trigger{} on http://{}.",
        service.len(),
        if service.len() == 1 { "" } else { "s" },
        address
    );
    Ok(Some(WebhookHost { server, service }))
}

fn print_service_summary(services: &TriggerServices, store: &FilesystemScriptStore) {
    if !services.schedules.is_empty() {
        println!(
            "Serving {} schedule trigger{} from {}.",
            services.schedules.len(),
            if services.schedules.len() == 1 {
                ""
            } else {
                "s"
            },
            store.root().display()
        );
    }
    if !services.file_watch_service.is_empty() {
        println!(
            "Serving {} file watch trigger{} from {}.",
            services.file_watch_service.len(),
            if services.file_watch_service.len() == 1 {
                ""
            } else {
                "s"
            },
            store.root().display()
        );
    }
    if let Some(host) = &services.webhook_host {
        println!(
            "Serving {} webhook trigger{}.",
            host.service.len(),
            if host.service.len() == 1 { "" } else { "s" },
        );
    }
    if !services.serial_input_service.is_empty() {
        println!(
            "Serving {} serial input trigger{}.",
            services.serial_input_service.len(),
            if services.serial_input_service.len() == 1 {
                ""
            } else {
                "s"
            },
        );
    }
}

fn dispatch_due_schedules(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    schedules: &mut ScheduleService,
) -> bool {
    let mut dispatched_any_event = false;
    let events = schedules.due_events(Instant::now(), SystemTime::now());
    for event in events {
        dispatched_any_event = true;
        println!(
            "Dispatching schedule trigger {} for script {}",
            event.node_id, event.script_id
        );
        match core.dispatch_trigger_event(store, event) {
            Ok(report) => print_run_report(report),
            Err(error) => eprintln!("Schedule dispatch failed: {error}"),
        }
    }
    dispatched_any_event
}

fn dispatch_trigger_events(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    receiver: &Receiver<TriggerEvent>,
) -> bool {
    let mut dispatched_any_event = false;
    for event in receiver.try_iter() {
        dispatched_any_event = true;
        dispatch_trigger_event(core, store, event);
    }
    dispatched_any_event
}

fn dispatch_trigger_event(core: &RunnerCore, store: &FilesystemScriptStore, event: TriggerEvent) {
    println!(
        "Dispatching trigger {} for script {}",
        event.node_id, event.script_id
    );
    match core.dispatch_trigger_event(store, event) {
        Ok(report) => print_run_report(report),
        Err(error) => eprintln!("Trigger dispatch failed: {error}"),
    }
}

fn handle_webhook_request(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    service: &WebhookService,
    mut request: Request,
    max_body_bytes: usize,
) -> Result<()> {
    let webhook_request = match webhook_request_from_http(&mut request, max_body_bytes) {
        Ok(request) => request,
        Err(response) => {
            respond(request, response)?;
            return Ok(());
        }
    };

    let Some(dispatch) = service.dispatch_for_request(&webhook_request) else {
        respond(
            request,
            WebhookResponse {
                body: "Webhook route not found.".to_owned(),
                content_type: "text/plain".to_owned(),
                headers: BTreeMap::new(),
                status_code: 404,
            },
        )?;
        return Ok(());
    };

    println!(
        "Dispatching webhook trigger {} for script {}",
        dispatch.event.node_id, dispatch.event.script_id
    );
    let response = match core.dispatch_trigger_event(store, dispatch.event.clone()) {
        Ok(report) => service.response_for_report(&dispatch, &report),
        Err(error) => WebhookResponse {
            body: format!("Webhook dispatch failed: {error}"),
            content_type: "text/plain".to_owned(),
            headers: BTreeMap::new(),
            status_code: 500,
        },
    };
    respond(request, response)?;
    Ok(())
}

fn webhook_request_from_http(
    request: &mut Request,
    max_body_bytes: usize,
) -> std::result::Result<WebhookRequest, WebhookResponse> {
    let mut body = Vec::new();
    request
        .as_reader()
        .take(max_body_bytes.saturating_add(1) as u64)
        .read_to_end(&mut body)
        .map_err(|source| WebhookResponse {
            body: format!("Failed to read request body: {source}"),
            content_type: "text/plain".to_owned(),
            headers: BTreeMap::new(),
            status_code: 400,
        })?;
    if body.len() > max_body_bytes {
        return Err(WebhookResponse {
            body: format!("Request body exceeds {max_body_bytes} bytes."),
            content_type: "text/plain".to_owned(),
            headers: BTreeMap::new(),
            status_code: 413,
        });
    }

    Ok(WebhookRequest {
        body: String::from_utf8_lossy(&body).into_owned(),
        headers: request
            .headers()
            .iter()
            .map(|header| {
                (
                    header.field.as_str().to_ascii_lowercase().to_string(),
                    header.value.as_str().to_owned(),
                )
            })
            .collect(),
        method: request.method().to_string(),
        path_and_query: request.url().to_owned(),
    })
}

fn respond(request: Request, webhook_response: WebhookResponse) -> Result<()> {
    let mut response = Response::from_string(webhook_response.body)
        .with_status_code(StatusCode(webhook_response.status_code));
    if let Ok(header) = Header::from_bytes("Content-Type", webhook_response.content_type) {
        response.add_header(header);
    }
    for (name, value) in webhook_response.headers {
        if let Ok(header) = Header::from_bytes(name, value) {
            response.add_header(header);
        }
    }
    request
        .respond(response)
        .map_err(|error| anyhow!("failed to write HTTP response: {error}"))
}

fn parse_payload_json(payload_json: Option<String>) -> Result<serde_json::Value> {
    match payload_json {
        Some(payload) => {
            serde_json::from_str(&payload).with_context(|| "failed to parse --payload-json as JSON")
        }
        None => Ok(serde_json::Value::Null),
    }
}

fn default_runner_home() -> PathBuf {
    if let Some(path) = std::env::var_os("BAUDBOUND_HOME") {
        return PathBuf::from(path);
    }

    platform_data_dir().join("BaudBound").join("runner")
}

fn default_config_path(runner_home: &std::path::Path) -> PathBuf {
    if let Some(path) = std::env::var_os("BAUDBOUND_CONFIG") {
        return PathBuf::from(path);
    }

    runner_home.join("config.toml")
}

fn platform_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(path) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(path);
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support");
        }
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(path);
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local").join("share");
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn print_installed_script(script: &InstalledScript) {
    println!("Script: {}", script.name);
    println!("ID: {}", script.id);
    println!("Enabled: {}", script.enabled);
    println!("Risk: {}", script.risk_level);
    println!("Target runtime: {}", script.target_runtime);
    println!("Package hash: {}", script.package_hash);
    println!("Package file: {}", script.package_file_name);
    println!("Package path: {}", script.package_path.display());
    println!("Assets: {}", script.asset_count);
    println!("Package version: {}", script.package_format_version);
    println!("Runtime version: {}", script.script_language_version);
}

fn print_approval_permissions(approval: &ScriptApproval) {
    if approval.approved_permissions.is_empty() {
        println!("Approved permissions: none");
        return;
    }

    println!("Approved permissions:");
    for permission in &approval.approved_permissions {
        println!("  - {permission}");
    }
}

fn print_trigger_registrations(registrations: &[TriggerRegistration]) {
    println!(
        "{:<28}  {:<24}  {:<28}  {:<24}  Type",
        "Script", "Script ID", "Trigger Node", "Action"
    );
    for registration in registrations {
        println!(
            "{:<28}  {:<24}  {:<28}  {:<24}  {}",
            truncate_for_table(&registration.script_name, 28),
            truncate_for_table(&registration.script_id, 24),
            truncate_for_table(&registration.node_id, 28),
            truncate_for_table(&registration.action_type, 24),
            registration.runner_type
        );
    }
}

fn truncate_for_table(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_owned();
    }

    value
        .chars()
        .take(width.saturating_sub(3))
        .collect::<String>()
        + "..."
}

fn print_run_report(report: RunReport) {
    println!("Run: {}", report.identity.run_id);
    println!("Trigger: {}", report.identity.trigger_node_id);
    for log in report.logs {
        match log.node_id {
            Some(node_id) => println!("[{}] [{}] {}", log.level, node_id, log.message),
            None => println!("[{}] {}", log.level, log.message),
        }
    }
    if !report.variables.is_empty() {
        println!("Variables:");
        for (name, value) in report.variables {
            println!("  {name}: {value}");
        }
    }
}

fn print_run_record(record: &StoredRunRecord) {
    println!(
        "Run: {}  script={}  status={}  completed_at={}",
        record.run_id, record.script_id, record.status, record.completed_at_unix
    );
    for log in &record.logs {
        match &log.node_id {
            Some(node_id) => println!("  [{}] [{}] {}", log.level, node_id, log.message),
            None => println!("  [{}] {}", log.level, log.message),
        }
    }
    if !record.variables.is_empty() {
        println!("  Variables:");
        for (name, value) in &record.variables {
            println!("    {name}: {value}");
        }
    }
}
