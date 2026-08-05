use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use baudbound_actions::SerialConnectionRegistry;
use baudbound_core::{RunnerConfig, serial_device_configs_from_settings};
use baudbound_runtime::ResourceLimit;
use baudbound_triggers::{
    SerialDeviceConfig as TriggerSerialDeviceConfig, SerialPortRebindSink,
    WebSocketConnectionRegistry,
};
use toml_edit::{DocumentMut, value};

use crate::trigger_monitor::TriggerMonitor;

#[derive(Clone)]
pub struct ServeOptions {
    pub(crate) file_watch_enabled: bool,
    pub(crate) hotkeys_enabled: bool,
    pub(crate) hotkey_stdin_enabled: bool,
    pub max_webhook_body_bytes: usize,
    pub max_webhook_connections: usize,
    pub(crate) webhook_max_unauthenticated_connections: ResourceLimit,
    pub(crate) webhook_pre_auth_requests_per_minute_global: ResourceLimit,
    pub(crate) webhook_pre_auth_requests_per_minute_per_address: ResourceLimit,
    pub(crate) webhook_header_read_timeout_ms: ResourceLimit,
    pub(crate) webhook_pre_auth_timeout_ms: ResourceLimit,
    pub(crate) webhook_body_read_progress_timeout_ms: ResourceLimit,
    pub(crate) webhook_body_read_timeout_ms: ResourceLimit,
    pub(crate) webhook_max_header_bytes: ResourceLimit,
    pub max_websocket_connections: usize,
    pub max_websocket_message_bytes: usize,
    pub(crate) websocket_max_unauthenticated_connections: ResourceLimit,
    pub(crate) websocket_pre_auth_requests_per_minute_global: ResourceLimit,
    pub(crate) websocket_pre_auth_requests_per_minute_per_address: ResourceLimit,
    pub(crate) websocket_handshake_timeout_ms: ResourceLimit,
    pub(crate) once: bool,
    pub(crate) process_watch_enabled: bool,
    pub(crate) allow_public_network_listeners: bool,
    pub(crate) reload_check_interval: Duration,
    pub(crate) run_schedules_immediately: bool,
    pub(crate) max_schedule_catch_up_events_per_poll: ResourceLimit,
    pub(crate) schedules_enabled: bool,
    pub(crate) serial_enabled: bool,
    pub(crate) serial_devices: Vec<TriggerSerialDeviceConfig>,
    pub(crate) serial_connections: Arc<SerialConnectionRegistry>,
    pub(crate) serial_port_rebind_sink: Option<Arc<dyn SerialPortRebindSink>>,
    pub(crate) startup_enabled: bool,
    pub(crate) trigger_monitor: Option<TriggerMonitor>,
    pub(crate) webhook_allow_browser_origins: BTreeSet<String>,
    pub(crate) webhook_allow_unauthenticated_public_bind: bool,
    pub webhook_bind: String,
    pub webhook_port: u16,
    pub(crate) webhooks_enabled: bool,
    pub(crate) websocket_allow_browser_origins: BTreeSet<String>,
    pub(crate) websocket_allow_unauthenticated_public_bind: bool,
    pub websocket_bind: String,
    pub websocket_port: u16,
    pub(crate) websocket_registry: Arc<WebSocketConnectionRegistry>,
    pub(crate) websockets_enabled: bool,
}

pub struct ServeOverrides {
    pub hotkey_stdin: bool,
    pub max_webhook_body_bytes: Option<usize>,
    pub max_websocket_connections: Option<usize>,
    pub max_websocket_message_bytes: Option<usize>,
    pub webhook_bind: Option<String>,
    pub webhook_port: Option<u16>,
    pub webhooks: bool,
    pub websocket_bind: Option<String>,
    pub websocket_port: Option<u16>,
    pub websockets: bool,
    pub reload_interval_seconds: Option<u64>,
}

impl ServeOptions {
    pub fn from_config(
        config: &RunnerConfig,
        overrides: ServeOverrides,
        once: bool,
        run_schedules_immediately: bool,
        websocket_registry: Arc<WebSocketConnectionRegistry>,
    ) -> Self {
        let serial_devices = serial_device_configs_from_settings(&config.serial.devices);
        let serial_connections = Arc::new(SerialConnectionRegistry::new(serial_devices.clone()));
        Self {
            file_watch_enabled: config.triggers.file_watch_enabled,
            hotkeys_enabled: config.triggers.hotkeys_enabled,
            hotkey_stdin_enabled: overrides.hotkey_stdin,
            max_webhook_body_bytes: overrides
                .max_webhook_body_bytes
                .unwrap_or(config.webhooks.max_body_bytes)
                .max(1),
            max_webhook_connections: config.webhooks.max_connections,
            webhook_max_unauthenticated_connections: config
                .webhooks
                .max_unauthenticated_connections,
            webhook_pre_auth_requests_per_minute_global: config
                .webhooks
                .pre_auth_requests_per_minute_global,
            webhook_pre_auth_requests_per_minute_per_address: config
                .webhooks
                .pre_auth_requests_per_minute_per_address,
            webhook_header_read_timeout_ms: config.webhooks.header_read_timeout_ms,
            webhook_pre_auth_timeout_ms: config.webhooks.pre_auth_timeout_ms,
            webhook_body_read_progress_timeout_ms: config.webhooks.body_read_progress_timeout_ms,
            webhook_body_read_timeout_ms: config.webhooks.body_read_timeout_ms,
            webhook_max_header_bytes: config.webhooks.max_header_bytes,
            max_websocket_connections: overrides
                .max_websocket_connections
                .unwrap_or(config.websockets.max_connections),
            max_websocket_message_bytes: overrides
                .max_websocket_message_bytes
                .unwrap_or(config.websockets.max_message_bytes),
            websocket_max_unauthenticated_connections: config
                .websockets
                .max_unauthenticated_connections,
            websocket_pre_auth_requests_per_minute_global: config
                .websockets
                .pre_auth_requests_per_minute_global,
            websocket_pre_auth_requests_per_minute_per_address: config
                .websockets
                .pre_auth_requests_per_minute_per_address,
            websocket_handshake_timeout_ms: config.websockets.handshake_timeout_ms,
            once,
            process_watch_enabled: config.triggers.process_watch_enabled,
            allow_public_network_listeners: config.security.policy.allow_public_network_listeners,
            reload_check_interval: Duration::from_secs(
                overrides
                    .reload_interval_seconds
                    .unwrap_or(config.runner.trigger_reload_seconds)
                    .max(1),
            ),
            run_schedules_immediately,
            max_schedule_catch_up_events_per_poll: config
                .limits
                .max_schedule_catch_up_events_per_poll,
            schedules_enabled: config.triggers.schedules_enabled,
            serial_enabled: config.triggers.serial_enabled,
            serial_devices,
            serial_connections,
            serial_port_rebind_sink: None,
            startup_enabled: config.triggers.startup_enabled,
            trigger_monitor: None,
            webhook_allow_browser_origins: config
                .webhooks
                .allow_browser_origins
                .iter()
                .cloned()
                .collect(),
            webhook_allow_unauthenticated_public_bind: config
                .webhooks
                .allow_unauthenticated_public_bind,
            webhook_bind: overrides
                .webhook_bind
                .unwrap_or_else(|| config.webhooks.bind.clone()),
            webhook_port: overrides.webhook_port.unwrap_or(config.webhooks.port),
            webhooks_enabled: config.triggers.webhooks_enabled || overrides.webhooks,
            websocket_allow_browser_origins: config
                .websockets
                .allow_browser_origins
                .iter()
                .cloned()
                .collect(),
            websocket_allow_unauthenticated_public_bind: config
                .websockets
                .allow_unauthenticated_public_bind,
            websocket_bind: overrides
                .websocket_bind
                .unwrap_or_else(|| config.websockets.bind.clone()),
            websocket_port: overrides.websocket_port.unwrap_or(config.websockets.port),
            websocket_registry,
            websockets_enabled: config.triggers.websockets_enabled || overrides.websockets,
        }
    }

    #[must_use]
    pub fn with_serial_port_rebind_sink(mut self, sink: Arc<dyn SerialPortRebindSink>) -> Self {
        self.serial_port_rebind_sink = Some(sink);
        self
    }

    #[must_use]
    pub fn with_serial_connections(mut self, connections: Arc<SerialConnectionRegistry>) -> Self {
        self.serial_connections = connections;
        self
    }

    #[must_use]
    pub(crate) fn with_trigger_monitor(mut self, monitor: TriggerMonitor) -> Self {
        self.trigger_monitor = Some(monitor);
        self
    }
}

pub struct RunnerConfigSerialPortRebindSink {
    config_path: PathBuf,
    lock: Mutex<()>,
}

impl RunnerConfigSerialPortRebindSink {
    #[must_use]
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            lock: Mutex::new(()),
        }
    }
}

impl SerialPortRebindSink for RunnerConfigSerialPortRebindSink {
    fn update_serial_device_port(&self, device_id: &str, port: &str) -> Result<(), String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "serial port rebind config lock is poisoned".to_owned())?;
        let contents = std::fs::read_to_string(&self.config_path).map_err(|source| {
            format!(
                "failed to read runner config {}: {source}",
                self.config_path.display()
            )
        })?;
        let config = RunnerConfig::from_toml(&contents, &self.config_path)
            .map_err(|source| source.to_string())?;
        let device =
            config.serial.devices.get(device_id).ok_or_else(|| {
                format!("runner config has no serial device entry for {device_id:?}")
            })?;
        if device.port == port {
            return Ok(());
        }
        let mut document = contents.parse::<DocumentMut>().map_err(|source| {
            format!(
                "failed to parse runner config {} for serial port rebind: {source}",
                self.config_path.display()
            )
        })?;
        document["serial"]["devices"][device_id]["port"] = value(port);
        let next_contents = document.to_string();
        RunnerConfig::from_toml(&next_contents, &self.config_path)
            .map_err(|source| source.to_string())?;
        RunnerConfig::write_atomic(&self.config_path, &next_contents)
            .map_err(|source| source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_port_rebind_preserves_the_config_document() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let path = directory.path().join("config.toml");
        std::fs::write(
            &path,
            "# keep this comment\n[serial.devices.controller]\nport = \"COM3\"\n",
        )
        .expect("test config should be written");
        let sink = RunnerConfigSerialPortRebindSink::new(path.clone());

        sink.update_serial_device_port("controller", "COM7")
            .expect("rebind should be persisted");

        let contents = std::fs::read_to_string(&path).expect("config should remain readable");
        let config = RunnerConfig::from_toml(&contents, &path).expect("config should stay valid");
        assert!(contents.contains("# keep this comment"));
        assert_eq!(config.serial.devices["controller"].port, "COM7");
    }
}
