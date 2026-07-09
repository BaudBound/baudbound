use std::{
    sync::{Arc, mpsc::Sender},
    time::{Instant, SystemTime},
};

use anyhow::{Context, Result};
use baudbound_core::{RunnerCore, TriggerEvent, TriggerRegistration};
use baudbound_storage::FilesystemScriptStore;
use baudbound_triggers::{
    FileWatchService, HotkeyService, ProcessStartedService, ScheduleService, SerialInputService,
    StartupService, WebSocketService,
};

use super::{
    options::ServeOptions,
    webhooks::{WebhookHost, build_webhook_host},
};

pub(super) struct TriggerServices {
    pub(super) file_watch_service: FileWatchService,
    pub(super) hotkey_service: HotkeyService,
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

impl TriggerRegistrationSet {
    fn load(core: &RunnerCore, store: &FilesystemScriptStore, operation: &str) -> Result<Self> {
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
            && self.process_started_service.is_empty()
            && self.serial_input_service.is_empty()
            && self.startup.is_empty()
            && self.webhook_host.is_none()
            && self.websocket_service.is_empty()
    }
}

pub(super) fn load_trigger_services(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    options: &ServeOptions,
    trigger_sender: &Sender<TriggerEvent>,
    previous_webhook_host: Option<WebhookHost>,
) -> Result<TriggerServices> {
    let registration_set = TriggerRegistrationSet::load(core, store, "load")?;

    build_trigger_services(
        registration_set,
        options,
        trigger_sender,
        previous_webhook_host,
    )
}

pub(super) fn reload_trigger_services_if_changed(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    options: &ServeOptions,
    trigger_sender: &Sender<TriggerEvent>,
    mut current: TriggerServices,
) -> Result<(TriggerServices, bool)> {
    let registration_set = TriggerRegistrationSet::load(core, store, "reload")?;

    if registration_set.fingerprint == current.registrations_fingerprint {
        return Ok((current, false));
    }

    let previous_webhook_host = current.webhook_host.take();
    let mut services = build_trigger_services(
        registration_set,
        options,
        trigger_sender,
        previous_webhook_host,
    )?;
    services.startup.drain_events();
    Ok((services, true))
}

fn build_trigger_services(
    registration_set: TriggerRegistrationSet,
    options: &ServeOptions,
    trigger_sender: &Sender<TriggerEvent>,
    previous_webhook_host: Option<WebhookHost>,
) -> Result<TriggerServices> {
    let registrations = registration_set.registrations;
    let schedules = if options.schedules_enabled {
        ScheduleService::from_registrations(registrations.clone(), Instant::now())
            .context("failed to register schedule triggers")?
    } else {
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
        ProcessStartedService::start(registrations.clone(), trigger_sender.clone())
            .context("failed to register process started triggers")?
    } else {
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
    let websocket_service = if options.websockets_enabled {
        WebSocketService::start(
            registrations.clone(),
            &options.websocket_bind,
            options.websocket_port,
            options.max_websocket_message_bytes,
            trigger_sender.clone(),
            Arc::clone(&options.websocket_registry),
        )
        .context("failed to register WebSocket triggers")?
    } else {
        WebSocketService::empty(Arc::clone(&options.websocket_registry))
    };
    let webhook_host = build_webhook_host(registrations, options, previous_webhook_host)?;

    Ok(TriggerServices {
        file_watch_service,
        hotkey_service,
        process_started_service,
        registrations_fingerprint: registration_set.fingerprint,
        schedules,
        serial_input_service,
        startup,
        webhook_host,
        websocket_service,
    })
}
