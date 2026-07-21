use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use baudbound_core::RunnerCore;
use baudbound_runtime::RuntimeCancellationToken;
use baudbound_storage::SqliteRunnerStore;
use serde_json::Value;

use crate::console;

use super::{
    ServiceStatusNotifier,
    dispatch::{
        dispatch_due_schedules, dispatch_hotkey_stdin_events, dispatch_startup_events,
        queue_trigger_event, queue_trigger_events, record_trigger_completions,
    },
    executor::TriggerExecutor,
    heartbeat::ServeStatusTracker,
    hotkey_stdin::spawn_hotkey_stdin_reader,
    idle::{print_idle_service_explanation, should_exit_idle_service},
    ipc::{ServiceControlCommand, ServiceControlServer},
    options::ServeOptions,
    shutdown::install_shutdown_handler,
    summary::print_service_summary,
    triggers::{load_trigger_services, reload_trigger_services_if_changed},
    webhooks::WebhookHost,
};

pub fn serve_triggers(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    options: ServeOptions,
) -> Result<()> {
    serve_triggers_with_control(core, store, options, ServeRuntimeControl::cli()?)
}

pub struct ServeRuntimeControl {
    shutdown_requested: Arc<AtomicBool>,
    status_change_notifier: Option<ServiceStatusNotifier>,
    stop_label: &'static str,
}

impl ServeRuntimeControl {
    fn cli() -> Result<Self> {
        Ok(Self {
            shutdown_requested: install_shutdown_handler()?,
            status_change_notifier: None,
            stop_label: "Shutdown requested",
        })
    }

    pub fn desktop(shutdown_requested: Arc<AtomicBool>) -> Self {
        Self {
            shutdown_requested,
            status_change_notifier: None,
            stop_label: "Desktop background runner stop requested",
        }
    }

    pub fn with_status_change_notifier(mut self, notifier: ServiceStatusNotifier) -> Self {
        self.status_change_notifier = Some(notifier);
        self
    }
}

pub fn serve_triggers_with_control(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    options: ServeOptions,
    control: ServeRuntimeControl,
) -> Result<()> {
    const MAX_IDLE_SLEEP: Duration = Duration::from_millis(250);

    const TRIGGER_CHANNEL_CAPACITY: usize = 1024;
    let (trigger_sender, trigger_receiver) = mpsc::sync_channel(TRIGGER_CHANNEL_CAPACITY);
    let (hotkey_sender, hotkey_receiver) = mpsc::channel();
    if options.hotkey_stdin_enabled {
        spawn_hotkey_stdin_reader(hotkey_sender);
    }
    let service_control = ServiceControlServer::bind()?;
    let cancellation = RuntimeCancellationToken::new();
    let mut trigger_executor = TriggerExecutor::new(core, store, "listener", cancellation.clone())
        .map_err(|error| anyhow!("failed to start trigger execution workers: {error}"))?;
    let status_revision = store
        .read_service_status()
        .context("failed to read the previous runner service status")?
        .as_ref()
        .and_then(|status| status.get("status_revision"))
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let mut status = ServeStatusTracker::start(
        service_control.descriptor().clone(),
        control.status_change_notifier.clone(),
        status_revision,
    );
    let mut services =
        load_trigger_services(core, store, &options, &trigger_sender, &cancellation)?;
    let mut dispatched_any_event =
        dispatch_startup_events(&mut services.startup, &mut trigger_executor, &mut status);
    status.write_running(store, &options, &services)?;

    if services.is_idle() {
        print_idle_service_explanation(&options);
        if should_exit_idle_service(&options) {
            stop_runtime(
                store,
                &options,
                &mut services,
                &mut trigger_executor,
                &cancellation,
                &mut status,
            )?;
            return Ok(());
        }
    }

    if options.run_schedules_immediately {
        services.schedules.mark_all_due_now(Instant::now());
    }

    print_service_summary(&services, store);
    console::info(format_args!(
        "Trigger registrations reload every {} second{}.",
        options.reload_check_interval.as_secs(),
        if options.reload_check_interval.as_secs() == 1 {
            ""
        } else {
            "s"
        }
    ));
    console::info(format_args!("Press Ctrl+C to stop."));

    let mut next_reload_check = Instant::now() + options.reload_check_interval;
    loop {
        if control.shutdown_requested.load(Ordering::SeqCst) {
            console::info(format_args!(
                "{}. Stopping trigger listener services.",
                control.stop_label
            ));
            stop_runtime(
                store,
                &options,
                &mut services,
                &mut trigger_executor,
                &cancellation,
                &mut status,
            )?;
            return Ok(());
        }

        let service_control_command = service_control
            .poll_command()
            .context("runner control IPC failed")?;
        if matches!(service_control_command, Some(ServiceControlCommand::Stop)) {
            console::info(format_args!(
                "Service stop requested. Stopping trigger listener services."
            ));
            stop_runtime(
                store,
                &options,
                &mut services,
                &mut trigger_executor,
                &cancellation,
                &mut status,
            )?;
            return Ok(());
        }

        let reload_requested = store
            .consume_trigger_reload_request()
            .context("failed to read trigger reload signal")?;
        let control_reload_requested =
            matches!(service_control_command, Some(ServiceControlCommand::Reload));
        if control_reload_requested || reload_requested || Instant::now() >= next_reload_check {
            let (reloaded_services, did_reload) = reload_trigger_services_if_changed(
                core,
                store,
                &options,
                &trigger_sender,
                services,
                &cancellation,
            )?;
            if did_reload {
                if control_reload_requested {
                    console::info(format_args!(
                        "Service reload requested. Reloaded listener services."
                    ));
                } else if reload_requested {
                    console::info(format_args!(
                        "Trigger reload requested. Reloaded listener services."
                    ));
                } else {
                    console::info(format_args!(
                        "Trigger registrations changed. Reloaded listener services."
                    ));
                }
                print_service_summary(&reloaded_services, store);
                if reloaded_services.is_idle() {
                    console::info(format_args!(
                        "No trigger listener services are currently active."
                    ));
                }
            } else if control_reload_requested {
                console::info(format_args!(
                    "Service reload requested, but registrations did not change."
                ));
            } else if reload_requested {
                console::info(format_args!(
                    "Trigger reload requested, but registrations did not change."
                ));
            }
            services = reloaded_services;
            status.mark_reloaded();
            next_reload_check = Instant::now() + options.reload_check_interval;
            status.write_running(store, &options, &services)?;
        }
        status.write_if_changed_or_due(store, &options, &services)?;

        if let Some(host) = services.webhook_host.as_mut() {
            dispatched_any_event |= host.poll(&mut status);
        }
        dispatched_any_event |= record_trigger_completions(&mut trigger_executor, &mut status);

        dispatched_any_event |=
            dispatch_due_schedules(&mut services.schedules, &mut trigger_executor, &mut status);
        dispatched_any_event |= dispatch_hotkey_stdin_events(
            &services.hotkey_service,
            &hotkey_receiver,
            &mut trigger_executor,
            &mut status,
        );
        queue_trigger_events(&trigger_receiver, &mut trigger_executor, &mut status);

        let webhook_execution_pending = services
            .webhook_host
            .as_ref()
            .is_some_and(WebhookHost::has_pending_execution);
        if options.once
            && dispatched_any_event
            && !trigger_executor.has_pending()
            && !webhook_execution_pending
        {
            stop_runtime(
                store,
                &options,
                &mut services,
                &mut trigger_executor,
                &cancellation,
                &mut status,
            )?;
            return Ok(());
        }

        let mut wait_duration = services
            .schedules
            .time_until_next_due(Instant::now())
            .unwrap_or(MAX_IDLE_SLEEP)
            .min(MAX_IDLE_SLEEP)
            .min(next_reload_check.saturating_duration_since(Instant::now()))
            .min(status.time_until_next_heartbeat());
        if let Some(webhook_poll_interval) = services
            .webhook_host
            .as_ref()
            .and_then(|host| host.response_poll_interval())
        {
            wait_duration = wait_duration.min(webhook_poll_interval);
        }

        if let Some(host) = &mut services.webhook_host {
            match host.server.recv_timeout(wait_duration) {
                Ok(Some(request)) => {
                    host.accept_request(request, options.max_webhook_body_bytes);
                }
                Ok(None) => {}
                Err(error) => return Err(anyhow!("webhook listener failed: {error}")),
            }
        } else if !services.file_watch_service.is_empty()
            || !services.process_started_service.is_empty()
            || !services.serial_input_service.is_empty()
            || !services.websocket_service.is_empty()
        {
            match trigger_receiver.recv_timeout(wait_duration) {
                Ok(event) => {
                    queue_trigger_event(&mut trigger_executor, event, &mut status);
                    queue_trigger_events(&trigger_receiver, &mut trigger_executor, &mut status);
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

fn stop_runtime(
    store: &SqliteRunnerStore,
    options: &ServeOptions,
    services: &mut super::triggers::TriggerServices,
    trigger_executor: &mut TriggerExecutor,
    cancellation: &RuntimeCancellationToken,
    status: &mut ServeStatusTracker,
) -> Result<()> {
    let stopped_status = status.prepare_stopped(store, options, services);
    cancellation.cancel();
    services.shutdown(options);
    trigger_executor
        .shutdown()
        .map_err(|error| anyhow!("failed to stop trigger execution workers: {error}"))?;
    status.write_prepared_stopped(store, &stopped_status)
}
