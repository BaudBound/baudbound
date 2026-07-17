use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use baudbound_core::{RunnerConfig, serial_device_configs_from_settings};
use baudbound_triggers::{
    SerialDeviceConfig as TriggerSerialDeviceConfig, SerialPortRebindSink,
    WebSocketConnectionRegistry,
};
use toml_edit::{DocumentMut, value};

#[derive(Clone)]
pub struct ServeOptions {
    pub(crate) file_watch_enabled: bool,
    pub(crate) hotkeys_enabled: bool,
    pub(crate) hotkey_stdin_enabled: bool,
    pub max_webhook_body_bytes: usize,
    pub max_websocket_connections: usize,
    pub max_websocket_message_bytes: usize,
    pub(crate) once: bool,
    pub(crate) process_watch_enabled: bool,
    pub(crate) reload_check_interval: Duration,
    pub(crate) run_schedules_immediately: bool,
    pub(crate) schedules_enabled: bool,
    pub(crate) serial_enabled: bool,
    pub(crate) serial_devices: Vec<TriggerSerialDeviceConfig>,
    pub(crate) serial_port_rebind_sink: Option<Arc<dyn SerialPortRebindSink>>,
    pub(crate) startup_enabled: bool,
    pub webhook_bind: String,
    pub webhook_port: u16,
    pub(crate) webhooks_enabled: bool,
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
        Self {
            file_watch_enabled: config.triggers.file_watch_enabled,
            hotkeys_enabled: config.triggers.hotkeys_enabled,
            hotkey_stdin_enabled: overrides.hotkey_stdin,
            max_webhook_body_bytes: overrides
                .max_webhook_body_bytes
                .unwrap_or(config.webhooks.max_body_bytes)
                .max(1),
            max_websocket_connections: overrides
                .max_websocket_connections
                .unwrap_or(config.websockets.max_connections),
            max_websocket_message_bytes: overrides
                .max_websocket_message_bytes
                .unwrap_or(config.websockets.max_message_bytes),
            once,
            process_watch_enabled: config.triggers.process_watch_enabled,
            reload_check_interval: Duration::from_secs(
                overrides
                    .reload_interval_seconds
                    .unwrap_or(config.runner.trigger_reload_seconds)
                    .max(1),
            ),
            run_schedules_immediately,
            schedules_enabled: config.triggers.schedules_enabled,
            serial_enabled: config.triggers.serial_enabled,
            serial_devices: serial_device_configs_from_settings(&config.serial.devices)
                .into_iter()
                .map(|device| TriggerSerialDeviceConfig {
                    auto_reconnect: device.auto_reconnect,
                    auto_rebind_port: device.auto_rebind_port,
                    baud_rate: device.baud_rate,
                    data_bits: device.data_bits,
                    device_id: device.device_id,
                    flow_control: device.flow_control,
                    manufacturer: device.manufacturer,
                    parity: device.parity,
                    port: device.port,
                    product_id: device.product_id,
                    product: device.product,
                    read_mode: device.read_mode,
                    serial_number: device.serial_number,
                    stop_bits: device.stop_bits,
                    validate_usb_identity: device.validate_usb_identity,
                    vendor_id: device.vendor_id,
                })
                .collect(),
            serial_port_rebind_sink: None,
            startup_enabled: config.triggers.startup_enabled,
            webhook_bind: overrides
                .webhook_bind
                .unwrap_or_else(|| config.webhooks.bind.clone()),
            webhook_port: overrides.webhook_port.unwrap_or(config.webhooks.port),
            webhooks_enabled: config.triggers.webhooks_enabled || overrides.webhooks,
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
        std::fs::write(&self.config_path, next_contents).map_err(|source| {
            format!(
                "failed to write runner config {}: {source}",
                self.config_path.display()
            )
        })
    }
}
