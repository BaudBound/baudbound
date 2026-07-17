use std::{
    sync::{Arc, mpsc::SyncSender},
    time::{Instant, SystemTime},
};

use anyhow::{Context, Result};
use baudbound_core::{RunnerCore, TriggerEvent, TriggerRegistration};
use baudbound_runtime::RuntimeCancellationToken;
use baudbound_storage::SqliteRunnerStore;
use baudbound_triggers::{
    FileWatchService, HotkeyService, NativeHotkeyService, ProcessStartedService, ScheduleService,
    SerialInputService, StartupService, WebSocketService, WebSocketServiceConfig,
};

use super::{
    options::ServeOptions,
    webhooks::{WebhookHost, build_webhook_host},
};

pub(super) struct TriggerServices {
    pub(super) file_watch_service: FileWatchService,
    pub(super) hotkey_service: HotkeyService,
    pub(super) native_hotkey_service: NativeHotkeyService,
    pub(super) process_started_service: ProcessStartedService,
    registrations_fingerprint: String,
    pub(super) schedules: ScheduleService,
    pub(super) serial_input_service: SerialInputService,
    pub(super) startup: StartupService,
    pub(super) webhook_host: Option<WebhookHost>,
    pub(super) websocket_service: WebSocketService,
}

struct TriggerRegistrationSet {
    fingerprint: String,
    registrations: Vec<TriggerRegistration>,
}

struct ReusableTriggerServices {
    native_hotkey: Option<NativeHotkeyService>,
    process_started: Option<ProcessStartedService>,
    schedules: Option<ScheduleService>,
    webhook: Option<WebhookHost>,
    websocket: Option<WebSocketService>,
}

impl ReusableTriggerServices {
    fn empty() -> Self {
        Self {
            native_hotkey: None,
            process_started: None,
            schedules: None,
            webhook: None,
            websocket: None,
        }
    }

    fn take_from(current: &mut TriggerServices, options: &ServeOptions) -> Self {
        Self {
            native_hotkey: Some(std::mem::replace(
                &mut current.native_hotkey_service,
                NativeHotkeyService::empty(),
            )),
            process_started: Some(std::mem::replace(
                &mut current.process_started_service,
                ProcessStartedService::empty(),
            )),
            schedules: Some(std::mem::replace(
                &mut current.schedules,
                ScheduleService::empty(),
            )),
            webhook: current.webhook_host.take(),
            websocket: Some(std::mem::replace(
                &mut current.websocket_service,
                WebSocketService::empty(Arc::clone(&options.websocket_registry)),
            )),
        }
    }
}

impl TriggerRegistrationSet {
    fn load(core: &RunnerCore, store: &SqliteRunnerStore, operation: &str) -> Result<Self> {
        let registrations = core
            .list_trigger_registrations(store, None)
            .with_context(|| format!("failed to {operation} trigger registrations"))?;
        let fingerprint = serde_json::to_string(&registrations)
            .context("failed to fingerprint trigger registrations")?;

        Ok(Self {
            fingerprint,
            registrations,
        })
    }
}

impl TriggerServices {
    pub(super) fn is_idle(&self) -> bool {
        self.schedules.is_empty()
            && self.file_watch_service.is_empty()
            && self.hotkey_service.is_empty()
            && self.native_hotkey_service.is_empty()
            && self.process_started_service.is_empty()
            && self.serial_input_service.is_empty()
            && self.startup.is_empty()
            && self.webhook_host.is_none()
            && self.websocket_service.is_empty()
    }

    pub(super) fn shutdown(&mut self, options: &ServeOptions) {
        self.webhook_host.take();
        drop(std::mem::replace(
            &mut self.native_hotkey_service,
            NativeHotkeyService::empty(),
        ));
        drop(std::mem::replace(
            &mut self.websocket_service,
            WebSocketService::empty(Arc::clone(&options.websocket_registry)),
        ));
        drop(std::mem::replace(
            &mut self.serial_input_service,
            SerialInputService::empty(),
        ));
        drop(std::mem::replace(
            &mut self.process_started_service,
            ProcessStartedService::empty(),
        ));
        drop(std::mem::replace(
            &mut self.file_watch_service,
            FileWatchService::empty(),
        ));
        self.hotkey_service = HotkeyService::empty();
        self.schedules = ScheduleService::empty();
        self.startup = StartupService::empty();
    }
}

pub(super) fn load_trigger_services(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    options: &ServeOptions,
    trigger_sender: &SyncSender<TriggerEvent>,
    cancellation: &RuntimeCancellationToken,
) -> Result<TriggerServices> {
    let registration_set = TriggerRegistrationSet::load(core, store, "load")?;

    build_trigger_services(
        core,
        store,
        registration_set,
        options,
        trigger_sender,
        cancellation,
        ReusableTriggerServices::empty(),
    )
}

pub(super) fn reload_trigger_services_if_changed(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    options: &ServeOptions,
    trigger_sender: &SyncSender<TriggerEvent>,
    mut current: TriggerServices,
    cancellation: &RuntimeCancellationToken,
) -> Result<(TriggerServices, bool)> {
    let registration_set = TriggerRegistrationSet::load(core, store, "reload")?;

    if registration_set.fingerprint == current.registrations_fingerprint {
        return Ok((current, false));
    }

    drop(std::mem::replace(
        &mut current.file_watch_service,
        FileWatchService::empty(),
    ));
    let reusable = ReusableTriggerServices::take_from(&mut current, options);
    let mut services = build_trigger_services(
        core,
        store,
        registration_set,
        options,
        trigger_sender,
        cancellation,
        reusable,
    )?;
    services.startup.drain_events();
    Ok((services, true))
}

fn build_trigger_services(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    registration_set: TriggerRegistrationSet,
    options: &ServeOptions,
    trigger_sender: &SyncSender<TriggerEvent>,
    cancellation: &RuntimeCancellationToken,
    reusable: ReusableTriggerServices,
) -> Result<TriggerServices> {
    let ReusableTriggerServices {
        native_hotkey: previous_native_hotkey_service,
        process_started: previous_process_started_service,
        schedules: previous_schedules,
        webhook: previous_webhook_host,
        websocket: previous_websocket_service,
    } = reusable;
    let registrations = registration_set.registrations;
    let schedules = if options.schedules_enabled {
        ScheduleService::start_or_reconfigure(
            registrations.clone(),
            Instant::now(),
            previous_schedules,
        )
        .context("failed to register schedule triggers")?
    } else {
        drop(previous_schedules);
        ScheduleService::empty()
    };
    let startup = if options.startup_enabled {
        StartupService::from_registrations(registrations.clone(), SystemTime::now())
            .context("failed to register startup triggers")?
    } else {
        StartupService::empty()
    };
    let file_watch_service = if options.file_watch_enabled {
        FileWatchService::start(registrations.clone(), trigger_sender.clone())
            .context("failed to register file watch triggers")?
    } else {
        FileWatchService::empty()
    };
    let process_started_service = if options.process_watch_enabled {
        ProcessStartedService::start_or_reconfigure(
            registrations.clone(),
            trigger_sender.clone(),
            previous_process_started_service,
        )
        .context("failed to register process started triggers")?
    } else {
        drop(previous_process_started_service);
        ProcessStartedService::empty()
    };
    let serial_input_service = if options.serial_enabled {
        SerialInputService::start(
            registrations.clone(),
            options.serial_devices.clone(),
            trigger_sender.clone(),
            options.serial_port_rebind_sink.clone(),
        )
        .context("failed to register serial input triggers")?
    } else {
        SerialInputService::empty()
    };
    let hotkey_service = if options.hotkey_stdin_enabled {
        HotkeyService::from_registrations(registrations.clone())
            .context("failed to register hotkey triggers")?
    } else {
        HotkeyService::empty()
    };
    let native_hotkey_service = NativeHotkeyService::start_or_reconfigure(
        registrations.clone(),
        trigger_sender.clone(),
        previous_native_hotkey_service,
    )
    .context("failed to register native hotkey triggers")?;
    let websocket_service = if options.websockets_enabled {
        WebSocketService::start_or_reconfigure(
            registrations.clone(),
            WebSocketServiceConfig {
                bind: options.websocket_bind.clone(),
                max_connections: options.max_websocket_connections,
                max_message_bytes: options.max_websocket_message_bytes,
                port: options.websocket_port,
            },
            trigger_sender.clone(),
            Arc::clone(&options.websocket_registry),
            previous_websocket_service,
        )
        .context("failed to register WebSocket triggers")?
    } else {
        drop(previous_websocket_service);
        WebSocketService::empty(Arc::clone(&options.websocket_registry))
    };
    let webhook_host = build_webhook_host(
        core,
        store,
        registrations,
        options,
        previous_webhook_host,
        cancellation,
    )?;

    Ok(TriggerServices {
        file_watch_service,
        hotkey_service,
        native_hotkey_service,
        process_started_service,
        registrations_fingerprint: registration_set.fingerprint,
        schedules,
        serial_input_service,
        startup,
        webhook_host,
        websocket_service,
    })
}
