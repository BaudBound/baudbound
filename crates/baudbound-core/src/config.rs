use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_WEBHOOK_BIND: &str = "127.0.0.1";
pub const DEFAULT_WEBHOOK_PORT: u16 = 43891;
pub const DEFAULT_WEBHOOK_MAX_BODY_BYTES: usize = 1024 * 1024;
pub const DEFAULT_WEBSOCKET_BIND: &str = "127.0.0.1";
pub const DEFAULT_WEBSOCKET_PORT: u16 = 43892;
pub const DEFAULT_WEBSOCKET_MAX_MESSAGE_BYTES: usize = 1024 * 1024;
pub const DEFAULT_WEBSOCKET_MAX_CONNECTIONS: usize = 128;
pub const DEFAULT_TRIGGER_RELOAD_SECONDS: u64 = 2;
pub const DEFAULT_RUN_HISTORY_MAX_RECORDS: usize = 10_000;
pub const DEFAULT_RUN_HISTORY_MAX_AGE_DAYS: u64 = 30;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RunnerConfig {
    pub runner: RunnerSettings,
    pub serial: SerialSettings,
    pub triggers: TriggerSettings,
    pub webhooks: WebhookSettings,
    pub websockets: WebSocketSettings,
}

impl RunnerConfig {
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self, RunnerConfigError> {
        let path = path.as_ref();
        match fs::read_to_string(path) {
            Ok(contents) => Self::from_toml(&contents, path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(RunnerConfigError::Read {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    pub fn load_or_init(path: impl AsRef<Path>) -> Result<Self, RunnerConfigError> {
        let path = path.as_ref();
        match fs::read_to_string(path) {
            Ok(contents) => Self::from_toml(&contents, path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Self::write_template(path)?;
                Self::from_toml(Self::template_toml(), path)
            }
            Err(source) => Err(RunnerConfigError::Read {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    pub fn from_toml(contents: &str, path: impl AsRef<Path>) -> Result<Self, RunnerConfigError> {
        let path = path.as_ref();
        let config: Self = toml::from_str(contents).map_err(|source| RunnerConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        config.validate(path)?;
        Ok(config)
    }

    pub fn to_pretty_toml(&self) -> Result<String, RunnerConfigError> {
        toml::to_string_pretty(self).map_err(|source| RunnerConfigError::Serialize {
            message: source.to_string(),
        })
    }

    fn validate(&self, path: &Path) -> Result<(), RunnerConfigError> {
        if self.runner.run_history_max_records == 0 {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: "runner.run_history_max_records must be greater than zero".to_owned(),
            });
        }
        if self.runner.run_history_max_age_days == 0 {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: "runner.run_history_max_age_days must be greater than zero".to_owned(),
            });
        }
        if self.websockets.max_connections == 0 {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: "websockets.max_connections must be greater than zero".to_owned(),
            });
        }
        if self.websockets.max_message_bytes == 0 {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: "websockets.max_message_bytes must be greater than zero".to_owned(),
            });
        }
        for (device_id, device) in &self.serial.devices {
            if !device.auto_rebind_port {
                continue;
            }
            if !device.validate_usb_identity {
                return Err(RunnerConfigError::Validate {
                    path: path.to_path_buf(),
                    message: format!(
                        "serial device {device_id:?} enables auto_rebind_port but validate_usb_identity is false"
                    ),
                });
            }
            if blank_optional(&device.vendor_id) {
                return Err(RunnerConfigError::Validate {
                    path: path.to_path_buf(),
                    message: format!(
                        "serial device {device_id:?} enables auto_rebind_port but vendor_id is not set"
                    ),
                });
            }
            if blank_optional(&device.product_id) {
                return Err(RunnerConfigError::Validate {
                    path: path.to_path_buf(),
                    message: format!(
                        "serial device {device_id:?} enables auto_rebind_port but product_id is not set"
                    ),
                });
            }
        }
        Ok(())
    }

    pub fn write_template(path: impl AsRef<Path>) -> Result<(), RunnerConfigError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| RunnerConfigError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        fs::write(path, Self::template_toml()).map_err(|source| RunnerConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn runner_name(&self) -> String {
        self.runner
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or("BaudBound Runner")
            .to_owned()
    }

    #[must_use]
    pub fn template_toml() -> &'static str {
        r#"# BaudBound runner configuration.
# The runner creates this file automatically on first start.
# Webhooks and WebSockets are disabled by default.

[runner]
name = "BaudBound Runner"
trigger_reload_seconds = 2
run_history_max_records = 10000
run_history_max_age_days = 30
# Empty or omitted target_runtimes means this runner supports this operating system's default headless and desktop targets.
# For headless service deployments, set this explicitly, for example:
# target_runtimes = ["Generic Headless", "Linux Headless"]
target_runtimes = []

[triggers]
schedules_enabled = true
file_watch_enabled = true
hotkeys_enabled = true
process_watch_enabled = true
serial_enabled = true
startup_enabled = true
webhooks_enabled = false
websockets_enabled = false

# Serial Input Trigger nodes reference only a logical deviceId.
# Define matching runner-side device settings here.
#
# [serial.devices.main_controller]
# port = "COM3"
# baud_rate = 115200
# data_bits = 8
# parity = "none"
# stop_bits = "1"
# flow_control = "none"
# read_mode = "line"
# auto_reconnect = true
# validate_usb_identity = false
# auto_rebind_port = false
# vendor_id = "1A86"
# product_id = "7523"
# serial_number = ""
# manufacturer = ""
# product = ""

[webhooks]
bind = "127.0.0.1"
port = 43891
max_body_bytes = 1048576

[websockets]
bind = "127.0.0.1"
port = 43892
max_message_bytes = 1048576
max_connections = 128
"#
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SerialSettings {
    pub devices: BTreeMap<String, SerialDeviceSettings>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SerialDeviceSettings {
    pub auto_reconnect: bool,
    pub auto_rebind_port: bool,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub flow_control: String,
    pub manufacturer: Option<String>,
    pub parity: String,
    pub port: String,
    pub product_id: Option<String>,
    pub product: Option<String>,
    pub read_mode: String,
    pub serial_number: Option<String>,
    pub stop_bits: String,
    pub validate_usb_identity: bool,
    pub vendor_id: Option<String>,
}

impl Default for SerialDeviceSettings {
    fn default() -> Self {
        Self {
            auto_reconnect: true,
            auto_rebind_port: false,
            baud_rate: 115_200,
            data_bits: 8,
            flow_control: "none".to_owned(),
            manufacturer: None,
            parity: "none".to_owned(),
            port: String::new(),
            product_id: None,
            product: None,
            read_mode: "line".to_owned(),
            serial_number: None,
            stop_bits: "1".to_owned(),
            validate_usb_identity: false,
            vendor_id: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct RunnerSettings {
    pub name: Option<String>,
    pub run_history_max_age_days: u64,
    pub run_history_max_records: usize,
    pub target_runtimes: Vec<String>,
    pub trigger_reload_seconds: u64,
}

impl Default for RunnerSettings {
    fn default() -> Self {
        Self {
            name: None,
            run_history_max_age_days: DEFAULT_RUN_HISTORY_MAX_AGE_DAYS,
            run_history_max_records: DEFAULT_RUN_HISTORY_MAX_RECORDS,
            target_runtimes: Vec::new(),
            trigger_reload_seconds: DEFAULT_TRIGGER_RELOAD_SECONDS,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct TriggerSettings {
    pub file_watch_enabled: bool,
    pub hotkeys_enabled: bool,
    pub process_watch_enabled: bool,
    pub schedules_enabled: bool,
    pub serial_enabled: bool,
    pub startup_enabled: bool,
    pub webhooks_enabled: bool,
    pub websockets_enabled: bool,
}

impl Default for TriggerSettings {
    fn default() -> Self {
        Self {
            file_watch_enabled: true,
            hotkeys_enabled: true,
            process_watch_enabled: true,
            schedules_enabled: true,
            serial_enabled: true,
            startup_enabled: true,
            webhooks_enabled: false,
            websockets_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct WebhookSettings {
    pub bind: String,
    pub max_body_bytes: usize,
    pub port: u16,
}

impl Default for WebhookSettings {
    fn default() -> Self {
        Self {
            bind: DEFAULT_WEBHOOK_BIND.to_owned(),
            max_body_bytes: DEFAULT_WEBHOOK_MAX_BODY_BYTES,
            port: DEFAULT_WEBHOOK_PORT,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct WebSocketSettings {
    pub bind: String,
    pub max_connections: usize,
    pub max_message_bytes: usize,
    pub port: u16,
}

impl Default for WebSocketSettings {
    fn default() -> Self {
        Self {
            bind: DEFAULT_WEBSOCKET_BIND.to_owned(),
            max_connections: DEFAULT_WEBSOCKET_MAX_CONNECTIONS,
            max_message_bytes: DEFAULT_WEBSOCKET_MAX_MESSAGE_BYTES,
            port: DEFAULT_WEBSOCKET_PORT,
        }
    }
}

#[derive(Debug, Error)]
pub enum RunnerConfigError {
    #[error("failed to read runner config {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse runner config {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("failed to serialize runner config: {message}")]
    Serialize { message: String },
    #[error("invalid runner config {path}: {message}")]
    Validate { path: PathBuf, message: String },
    #[error("failed to write runner config {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

fn blank_optional(value: &Option<String>) -> bool {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_uses_safe_defaults() {
        let temporary_directory = tempfile::tempdir().expect("temp dir");
        let config_path = temporary_directory.path().join("runner.toml");

        let config = RunnerConfig::load_or_default(&config_path).expect("config should load");

        assert_eq!(config.runner_name(), "BaudBound Runner");
        assert_eq!(
            config.runner.trigger_reload_seconds,
            DEFAULT_TRIGGER_RELOAD_SECONDS
        );
        assert_eq!(
            config.runner.run_history_max_records,
            DEFAULT_RUN_HISTORY_MAX_RECORDS
        );
        assert_eq!(
            config.runner.run_history_max_age_days,
            DEFAULT_RUN_HISTORY_MAX_AGE_DAYS
        );
        assert!(config.triggers.schedules_enabled);
        assert!(config.triggers.file_watch_enabled);
        assert!(config.triggers.hotkeys_enabled);
        assert!(config.triggers.serial_enabled);
        assert!(!config.triggers.webhooks_enabled);
        assert_eq!(config.webhooks.bind, DEFAULT_WEBHOOK_BIND);
        assert_eq!(config.webhooks.port, DEFAULT_WEBHOOK_PORT);
        assert!(
            !config_path.exists(),
            "load_or_default should preserve legacy no-write behavior"
        );
    }

    #[test]
    fn missing_config_is_initialized_on_load_or_init() {
        let temporary_directory = tempfile::tempdir().expect("temp dir");
        let config_path = temporary_directory
            .path()
            .join("nested")
            .join("runner.toml");

        let config = RunnerConfig::load_or_init(&config_path).expect("config should load");

        assert_eq!(config.runner_name(), "BaudBound Runner");
        assert!(
            config_path.exists(),
            "load_or_init should create a real config file"
        );
        let contents = fs::read_to_string(config_path).expect("config should be readable");
        assert!(contents.contains("[triggers]"));
        assert!(contents.contains("hotkeys_enabled = true"));
    }

    #[test]
    fn parses_configured_trigger_services() {
        let config = RunnerConfig::from_toml(
            r#"
                [runner]
                name = "Server Runner"
                trigger_reload_seconds = 5
                run_history_max_records = 2500
                run_history_max_age_days = 14
                target_runtimes = ["Generic Headless", "Linux Headless"]

                [triggers]
                schedules_enabled = false
                file_watch_enabled = true
                hotkeys_enabled = false
                process_watch_enabled = false
                serial_enabled = false
                startup_enabled = true
                webhooks_enabled = true
                websockets_enabled = true

                [serial.devices.main_controller]
                port = "COM3"
                baud_rate = 115200
                data_bits = 8
                parity = "none"
                stop_bits = "1"
                flow_control = "none"
                read_mode = "line"
                auto_reconnect = true
                auto_rebind_port = true
                validate_usb_identity = true
                vendor_id = "1A86"
                product_id = "7523"
                serial_number = "ABC123"

                [webhooks]
                bind = "0.0.0.0"
                port = 9000
                max_body_bytes = 2048

                [websockets]
                bind = "127.0.0.1"
                port = 9001
                max_message_bytes = 4096
                max_connections = 32
            "#,
            "runner.toml",
        )
        .expect("config should parse");

        assert_eq!(config.runner_name(), "Server Runner");
        assert_eq!(config.runner.trigger_reload_seconds, 5);
        assert_eq!(config.runner.run_history_max_records, 2500);
        assert_eq!(config.runner.run_history_max_age_days, 14);
        assert_eq!(
            config.runner.target_runtimes,
            ["Generic Headless", "Linux Headless"]
        );
        assert!(!config.triggers.schedules_enabled);
        assert!(config.triggers.file_watch_enabled);
        assert!(!config.triggers.hotkeys_enabled);
        assert!(!config.triggers.process_watch_enabled);
        assert!(!config.triggers.serial_enabled);
        assert!(config.triggers.startup_enabled);
        assert!(config.triggers.webhooks_enabled);
        assert!(config.triggers.websockets_enabled);
        assert_eq!(config.webhooks.bind, "0.0.0.0");
        assert_eq!(config.webhooks.port, 9000);
        assert_eq!(config.webhooks.max_body_bytes, 2048);
        assert_eq!(config.websockets.bind, "127.0.0.1");
        assert_eq!(config.websockets.port, 9001);
        assert_eq!(config.websockets.max_message_bytes, 4096);
        assert_eq!(config.websockets.max_connections, 32);
        let device = config
            .serial
            .devices
            .get("main_controller")
            .expect("serial device should parse");
        assert_eq!(device.port, "COM3");
        assert_eq!(device.baud_rate, 115_200);
        assert!(device.auto_rebind_port);
        assert_eq!(device.vendor_id.as_deref(), Some("1A86"));
        assert_eq!(device.serial_number.as_deref(), Some("ABC123"));
    }

    #[test]
    fn rejects_zero_run_history_retention_limits() {
        for contents in [
            "[runner]\nrun_history_max_records = 0",
            "[runner]\nrun_history_max_age_days = 0",
        ] {
            let error = RunnerConfig::from_toml(contents, "runner.toml")
                .expect_err("run history retention must stay bounded");
            assert!(error.to_string().contains("must be greater than zero"));
        }
    }

    #[test]
    fn rejects_auto_rebind_without_usb_identity() {
        let error = RunnerConfig::from_toml(
            r#"
                [serial.devices.main_controller]
                port = "COM3"
                auto_rebind_port = true
                validate_usb_identity = false
                vendor_id = "1A86"
                product_id = "7523"
            "#,
            "runner.toml",
        )
        .expect_err("auto rebind without USB identity should fail");

        assert!(matches!(error, RunnerConfigError::Validate { .. }));
    }

    #[test]
    fn rejects_auto_rebind_without_vendor_or_product_id() {
        let error = RunnerConfig::from_toml(
            r#"
                [serial.devices.main_controller]
                port = "COM3"
                auto_rebind_port = true
                validate_usb_identity = true
                vendor_id = "1A86"
            "#,
            "runner.toml",
        )
        .expect_err("auto rebind without product id should fail");

        assert!(matches!(error, RunnerConfigError::Validate { .. }));
    }

    #[test]
    fn rejects_invalid_toml() {
        let error = RunnerConfig::from_toml("[webhooks", "runner.toml")
            .expect_err("invalid TOML should fail");

        assert!(matches!(error, RunnerConfigError::Parse { .. }));
    }

    #[test]
    fn rejects_zero_websocket_resource_limits() {
        for (field, config) in [
            (
                "max_connections",
                "[websockets]\nmax_connections = 0\nmax_message_bytes = 1024",
            ),
            (
                "max_message_bytes",
                "[websockets]\nmax_connections = 4\nmax_message_bytes = 0",
            ),
        ] {
            let error = RunnerConfig::from_toml(config, "runner.toml")
                .expect_err("zero WebSocket resource limit should fail");
            assert!(error.to_string().contains(field), "{error}");
        }
    }

    #[test]
    fn template_toml_parses_as_valid_config() {
        let config = RunnerConfig::from_toml(RunnerConfig::template_toml(), "template.toml")
            .expect("template should parse");

        assert_eq!(config.runner_name(), "BaudBound Runner");
        assert_eq!(config.webhooks.port, DEFAULT_WEBHOOK_PORT);
        assert_eq!(config.websockets.port, DEFAULT_WEBSOCKET_PORT);
        assert_eq!(
            config.websockets.max_connections,
            DEFAULT_WEBSOCKET_MAX_CONNECTIONS
        );
        assert!(!config.triggers.webhooks_enabled);
        assert!(!config.triggers.websockets_enabled);
    }
}
