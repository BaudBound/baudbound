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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RunnerConfig {
    pub runner: RunnerSettings,
    pub serial: SerialSettings,
    pub triggers: TriggerSettings,
    pub webhooks: WebhookSettings,
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

    pub fn from_toml(contents: &str, path: impl AsRef<Path>) -> Result<Self, RunnerConfigError> {
        toml::from_str(contents).map_err(|source| RunnerConfigError::Parse {
            path: path.as_ref().to_path_buf(),
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
    pub baud_rate: u32,
    pub data_bits: u8,
    pub flow_control: String,
    pub parity: String,
    pub port: String,
    pub product_id: Option<String>,
    pub read_mode: String,
    pub stop_bits: String,
    pub validate_usb_identity: bool,
    pub vendor_id: Option<String>,
}

impl Default for SerialDeviceSettings {
    fn default() -> Self {
        Self {
            auto_reconnect: true,
            baud_rate: 115_200,
            data_bits: 8,
            flow_control: "none".to_owned(),
            parity: "none".to_owned(),
            port: String::new(),
            product_id: None,
            read_mode: "line".to_owned(),
            stop_bits: "1".to_owned(),
            validate_usb_identity: false,
            vendor_id: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RunnerSettings {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct TriggerSettings {
    pub file_watch_enabled: bool,
    pub schedules_enabled: bool,
    pub serial_enabled: bool,
    pub webhooks_enabled: bool,
}

impl Default for TriggerSettings {
    fn default() -> Self {
        Self {
            file_watch_enabled: true,
            schedules_enabled: true,
            serial_enabled: true,
            webhooks_enabled: false,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_uses_safe_defaults() {
        let temporary_directory = tempfile::tempdir().expect("temp dir");
        let config_path = temporary_directory.path().join("runner.toml");

        let config = RunnerConfig::load_or_default(config_path).expect("config should load");

        assert_eq!(config.runner_name(), "BaudBound Runner");
        assert!(config.triggers.schedules_enabled);
        assert!(config.triggers.file_watch_enabled);
        assert!(config.triggers.serial_enabled);
        assert!(!config.triggers.webhooks_enabled);
        assert_eq!(config.webhooks.bind, DEFAULT_WEBHOOK_BIND);
        assert_eq!(config.webhooks.port, DEFAULT_WEBHOOK_PORT);
    }

    #[test]
    fn parses_configured_trigger_services() {
        let config = RunnerConfig::from_toml(
            r#"
                [runner]
                name = "Server Runner"

                [triggers]
                schedules_enabled = false
                file_watch_enabled = true
                serial_enabled = false
                webhooks_enabled = true

                [serial.devices.main_controller]
                port = "COM3"
                baud_rate = 115200
                data_bits = 8
                parity = "none"
                stop_bits = "1"
                flow_control = "none"
                read_mode = "line"
                auto_reconnect = true
                validate_usb_identity = true
                vendor_id = "1A86"
                product_id = "7523"

                [webhooks]
                bind = "0.0.0.0"
                port = 9000
                max_body_bytes = 2048
            "#,
            "runner.toml",
        )
        .expect("config should parse");

        assert_eq!(config.runner_name(), "Server Runner");
        assert!(!config.triggers.schedules_enabled);
        assert!(config.triggers.file_watch_enabled);
        assert!(!config.triggers.serial_enabled);
        assert!(config.triggers.webhooks_enabled);
        assert_eq!(config.webhooks.bind, "0.0.0.0");
        assert_eq!(config.webhooks.port, 9000);
        assert_eq!(config.webhooks.max_body_bytes, 2048);
        let device = config
            .serial
            .devices
            .get("main_controller")
            .expect("serial device should parse");
        assert_eq!(device.port, "COM3");
        assert_eq!(device.baud_rate, 115_200);
        assert_eq!(device.vendor_id.as_deref(), Some("1A86"));
    }

    #[test]
    fn rejects_invalid_toml() {
        let error = RunnerConfig::from_toml("[webhooks", "runner.toml")
            .expect_err("invalid TOML should fail");

        assert!(matches!(error, RunnerConfigError::Parse { .. }));
    }
}
