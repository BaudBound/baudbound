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
use baudbound_storage::{FilesystemScriptStore, ServiceControlCommand};

use super::{
    control::{
        consume_service_control_request, install_shutdown_handler, spawn_hotkey_stdin_reader,
    },
    dispatch::{
        dispatch_due_schedules, dispatch_hotkey_stdin_events, dispatch_startup_events,
        dispatch_trigger_event, dispatch_trigger_events,
    },
    heartbeat::ServeStatusTracker,
    idle::{print_idle_service_explanation, should_exit_idle_service},
    options::ServeOptions,
    summary::print_service_summary,
    triggers::{load_trigger_services, reload_trigger_services_if_changed},
    webhooks::handle_webhook_request,
};

pub fn serve_triggers(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    options: ServeOptions,
) -> Result<()> {
    serve_triggers_with_control(core, store, options, ServeRuntimeControl::cli()?)
}

pub struct ServeRuntimeControl {
    shutdown_requested: Arc<AtomicBool>,
    consume_service_control_requests: bool,
    stop_label: &'static str,
}

impl ServeRuntimeControl {
    fn cli() -> Result<Self> {
        Ok(Self {
            shutdown_requested: install_shutdown_handler()?,
            consume_service_control_requests: true,
            stop_label: "Shutdown requested",
        })
    }

    pub fn desktop(shutdown_requested: Arc<AtomicBool>) -> Self {
        Self {
            shutdown_requested,
            consume_service_control_requests: false,
            stop_label: "Desktop background runner stop requested",
        }
    }
}

pub fn serve_triggers_with_control(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    options: ServeOptions,
    control: ServeRuntimeControl,
) -> Result<()> {
    const MAX_IDLE_SLEEP: Duration = Duration::from_secs(1);

    let (trigger_sender, trigger_receiver) = mpsc::channel();
    let (hotkey_sender, hotkey_receiver) = mpsc::channel();
    if options.hotkey_stdin_enabled {
        spawn_hotkey_stdin_reader(hotkey_sender);
    }
    let mut status = ServeStatusTracker::start();
    let mut services = load_trigger_services(core, store, &options, &trigger_sender, None)?;
    let mut dispatched_any_event =
        dispatch_startup_events(core, store, &mut services.startup, &mut status);
    status.write_running(store, &options, &services)?;

    if services.is_idle() {
        print_idle_service_explanation(&options);
        if should_exit_idle_service(&options) {
            status.write_stopped(store, &options, &services)?;
            return Ok(());
        }
    }

    if options.run_schedules_immediately {
        services.schedules.mark_all_due_now(Instant::now());
    }

    print_service_summary(&services, store);
    println!(
        "Trigger registrations reload every {} second{}.",
        options.reload_check_interval.as_secs(),
        if options.reload_check_interval.as_secs() == 1 {
            ""
        } else {
            "s"
        }
    );
    if control.consume_service_control_requests {
        println!("Press Ctrl+C to stop.");
    }

    let mut next_reload_check = Instant::now() + options.reload_check_interval;
    loop {
        if control.shutdown_requested.load(Ordering::SeqCst) {
            println!(
                "{}. Stopping trigger listener services.",
                control.stop_label
            );
            status.write_stopped(store, &options, &services)?;
            return Ok(());
        }

        let service_control = if control.consume_service_control_requests {
            consume_service_control_request(store)?
        } else {
            None
        };
        if matches!(service_control, Some(ServiceControlCommand::Stop)) {
            println!("Service stop requested. Stopping trigger listener services.");
            status.write_stopped(store, &options, &services)?;
            return Ok(());
        }

        let reload_requested = store
            .consume_trigger_reload_request()
            .context("failed to read trigger reload signal")?;
        let control_reload_requested =
            matches!(service_control, Some(ServiceControlCommand::Reload));
        if control_reload_requested || reload_requested || Instant::now() >= next_reload_check {
            let (reloaded_services, did_reload) = reload_trigger_services_if_changed(
                core,
                store,
                &options,
                &trigger_sender,
                services,
            )?;
            if did_reload {
                if control_reload_requested {
                    println!("Service reload requested. Reloaded listener services.");
                } else if reload_requested {
                    println!("Trigger reload requested. Reloaded listener services.");
                } else {
                    println!("Trigger registrations changed. Reloaded listener services.");
                }
                print_service_summary(&reloaded_services, store);
                if reloaded_services.is_idle() {
                    println!("No trigger listener services are currently active.");
                }
            } else if control_reload_requested {
                println!("Service reload requested, but registrations did not change.");
            } else if reload_requested {
                println!("Trigger reload requested, but registrations did not change.");
            }
            services = reloaded_services;
            status.mark_reloaded();
            next_reload_check = Instant::now() + options.reload_check_interval;
            status.write_running(store, &options, &services)?;
        }
        status.write_heartbeat_if_due(store, &options, &services)?;

        dispatched_any_event |=
            dispatch_due_schedules(core, store, &mut services.schedules, &mut status);
        dispatched_any_event |= dispatch_hotkey_stdin_events(
            core,
            store,
            &services.hotkey_service,
            &hotkey_receiver,
            &mut status,
        );
        dispatched_any_event |=
            dispatch_trigger_events(core, store, &trigger_receiver, &mut status);

        if options.once && dispatched_any_event {
            status.write_stopped(store, &options, &services)?;
            return Ok(());
        }

        let wait_duration = services
            .schedules
            .time_until_next_due(Instant::now())
            .unwrap_or(MAX_IDLE_SLEEP)
            .min(MAX_IDLE_SLEEP)
            .min(next_reload_check.saturating_duration_since(Instant::now()))
            .min(status.time_until_next_heartbeat());

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
                        &mut status,
                    )?;
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
                    dispatched_any_event = true;
                    dispatch_trigger_event(core, store, event, &mut status);
                    dispatched_any_event |=
                        dispatch_trigger_events(core, store, &trigger_receiver, &mut status);
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
