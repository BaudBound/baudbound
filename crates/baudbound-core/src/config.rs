use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    net::Ipv4Addr,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

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
pub const DEFAULT_UPDATE_CHECK_INTERVAL_HOURS: u64 = 24;
pub const DEFAULT_SERIAL_BAUD_RATE: u32 = 9_600;
pub const DEFAULT_SERIAL_DTR_ON_OPEN: &str = "deasserted";
pub const DEFAULT_SERIAL_MESSAGE_GAP_MS: u64 = 100;
pub const DEFAULT_SERIAL_MAX_MESSAGE_BYTES: usize = 1024 * 1024;
pub const DEFAULT_SERIAL_OPEN_STABILIZATION_MS: u64 = 500;
pub const DEFAULT_SERIAL_READ_MODE: &str = "idle_gap";
pub const MAX_SERIAL_OPEN_STABILIZATION_MS: u64 = 60_000;
pub const MAX_SERIAL_MESSAGE_GAP_MS: u64 = 60_000;
pub const MAX_SERIAL_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_RUNNER_CONFIG_BYTES: usize = 1024 * 1024;
pub const MAX_BROWSER_ORIGINS: usize = 128;
pub const MAX_BROWSER_ORIGIN_LENGTH: usize = 2048;
pub const MAX_SERIAL_DEVICES: usize = 128;
pub const MAX_SERIAL_DEVICE_ID_LENGTH: usize = 64;
pub const MAX_SERIAL_PORT_LENGTH: usize = 1024;
pub const MAX_SERIAL_METADATA_LENGTH: usize = 512;
const MAX_RUN_HISTORY_RECORDS: usize = 10_000_000;
const MAX_RUN_HISTORY_AGE_DAYS: u64 = 36_500;
const MAX_TRIGGER_RELOAD_SECONDS: u64 = 86_400;
const MAX_UPDATE_CHECK_INTERVAL_HOURS: u64 = 8_760;
const MAX_NETWORK_CONNECTIONS: usize = 10_000;
const MAX_EXTERNAL_DATA_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub use baudbound_actions::{
    DEFAULT_MAX_FILE_DOWNLOAD_BYTES, DEFAULT_MAX_FILE_READ_BYTES, DEFAULT_MAX_HTTP_RESPONSE_BYTES,
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RunnerConfig {
    pub desktop: DesktopSettings,
    pub display: DisplaySettings,
    pub limits: LimitSettings,
    pub runner: RunnerSettings,
    pub serial: SerialSettings,
    pub triggers: TriggerSettings,
    pub updates: UpdateSettings,
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
        if contents.len() > MAX_RUNNER_CONFIG_BYTES {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: format!(
                    "configuration exceeds the maximum size of {MAX_RUNNER_CONFIG_BYTES} bytes"
                ),
            });
        }
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
        if !(1..=MAX_RUN_HISTORY_RECORDS).contains(&self.runner.run_history_max_records) {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: format!(
                    "runner.run_history_max_records must be between 1 and {MAX_RUN_HISTORY_RECORDS}"
                ),
            });
        }
        if !(1..=MAX_RUN_HISTORY_AGE_DAYS).contains(&self.runner.run_history_max_age_days) {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: format!(
                    "runner.run_history_max_age_days must be between 1 and {MAX_RUN_HISTORY_AGE_DAYS}"
                ),
            });
        }
        if !(1..=MAX_TRIGGER_RELOAD_SECONDS).contains(&self.runner.trigger_reload_seconds) {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: format!(
                    "runner.trigger_reload_seconds must be between 1 and {MAX_TRIGGER_RELOAD_SECONDS}"
                ),
            });
        }
        validate_target_runtimes(path, &self.runner.target_runtimes)?;
        if !(1..=MAX_UPDATE_CHECK_INTERVAL_HOURS).contains(&self.updates.check_interval_hours) {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: format!(
                    "updates.check_interval_hours must be between 1 and {MAX_UPDATE_CHECK_INTERVAL_HOURS}"
                ),
            });
        }
        for (setting, value) in [
            (
                "limits.max_http_response_bytes",
                self.limits.max_http_response_bytes,
            ),
            (
                "limits.max_file_download_bytes",
                self.limits.max_file_download_bytes,
            ),
            (
                "limits.max_file_read_bytes",
                self.limits.max_file_read_bytes,
            ),
        ] {
            if value == 0 || value > MAX_EXTERNAL_DATA_BYTES {
                return Err(RunnerConfigError::Validate {
                    path: path.to_path_buf(),
                    message: format!("{setting} must be between 1 and {MAX_EXTERNAL_DATA_BYTES}"),
                });
            }
        }
        validate_bind_address(path, "webhooks.bind", &self.webhooks.bind)?;
        if self.webhooks.max_body_bytes == 0
            || u64::try_from(self.webhooks.max_body_bytes)
                .map_or(true, |value| value > MAX_EXTERNAL_DATA_BYTES)
        {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: format!(
                    "webhooks.max_body_bytes must be between 1 and {MAX_EXTERNAL_DATA_BYTES}"
                ),
            });
        }
        validate_browser_origins(
            path,
            "webhooks.allow_browser_origins",
            &self.webhooks.allow_browser_origins,
        )?;
        validate_bind_address(path, "websockets.bind", &self.websockets.bind)?;
        if !(1..=MAX_NETWORK_CONNECTIONS).contains(&self.websockets.max_connections) {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: format!(
                    "websockets.max_connections must be between 1 and {MAX_NETWORK_CONNECTIONS}"
                ),
            });
        }
        if self.websockets.max_message_bytes == 0
            || u64::try_from(self.websockets.max_message_bytes)
                .map_or(true, |value| value > MAX_EXTERNAL_DATA_BYTES)
        {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: format!(
                    "websockets.max_message_bytes must be between 1 and {MAX_EXTERNAL_DATA_BYTES}"
                ),
            });
        }
        validate_browser_origins(
            path,
            "websockets.allow_browser_origins",
            &self.websockets.allow_browser_origins,
        )?;
        if self.serial.devices.len() > MAX_SERIAL_DEVICES {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: format!(
                    "serial.devices contains more than {MAX_SERIAL_DEVICES} configured devices"
                ),
            });
        }
        for (device_id, device) in &self.serial.devices {
            validate_serial_device(path, device_id, device)?;
        }
        Ok(())
    }

    pub fn write_template(path: impl AsRef<Path>) -> Result<(), RunnerConfigError> {
        Self::write_atomic(path, Self::template_toml())
    }

    pub fn write_atomic(path: impl AsRef<Path>, contents: &str) -> Result<(), RunnerConfigError> {
        let path = path.as_ref();
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|source| RunnerConfigError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|source| RunnerConfigError::Write {
                path: path.to_path_buf(),
                source,
            })?;
        temporary
            .write_all(contents.as_bytes())
            .and_then(|()| temporary.as_file_mut().sync_all())
            .map_err(|source| RunnerConfigError::Write {
                path: path.to_path_buf(),
                source,
            })?;
        temporary
            .persist(path)
            .map_err(|error| RunnerConfigError::Write {
                path: path.to_path_buf(),
                source: error.error,
            })?;
        Ok(())
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), RunnerConfigError> {
        let path = path.as_ref();
        self.validate(path)?;
        let contents = self.to_pretty_toml()?;
        Self::write_atomic(path, &contents)
    }

    #[must_use]
    pub fn template_toml() -> &'static str {
        r#"# BaudBound runner configuration.
# The runner creates this file automatically on first start.
# Webhooks and WebSockets are disabled by default.

[runner]
trigger_reload_seconds = 2
run_history_max_records = 10000
run_history_max_age_days = 30
# Empty or omitted target_runtimes allows the runner mode currently active on this operating system.
# To restrict the allowed modes, list explicit operating system targets:
# target_runtimes = ["Linux Headless", "Linux Desktop"]
target_runtimes = []

[display]
time_format = "24-hour"

[limits]
max_http_response_bytes = 10485760
max_file_download_bytes = 104857600
max_file_read_bytes = 10485760

[updates]
automatic_checks = true
check_interval_hours = 24

[desktop]
launch_at_login = false
start_background_runner_on_launch = false
start_minimized_to_tray = false
keep_running_on_close = true

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
# baud_rate = 9600
# data_bits = 8
# dtr_on_open = "deasserted"
# parity = "none"
# stop_bits = "1"
# flow_control = "none"
# read_mode = "idle_gap"
# message_gap_ms = 100
# max_message_bytes = 1048576
# open_stabilization_ms = 500
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
allow_browser_origins = []
allow_unauthenticated_public_bind = false

[websockets]
bind = "127.0.0.1"
port = 43892
max_message_bytes = 1048576
max_connections = 128
allow_browser_origins = []
allow_unauthenticated_public_bind = false
"#
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DisplaySettings {
    pub time_format: TimeFormat,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            time_format: TimeFormat::TwentyFourHour,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum TimeFormat {
    #[serde(rename = "12-hour")]
    TwelveHour,
    #[default]
    #[serde(rename = "24-hour")]
    TwentyFourHour,
}

impl TimeFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TwelveHour => "12-hour",
            Self::TwentyFourHour => "24-hour",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct LimitSettings {
    pub max_file_download_bytes: u64,
    pub max_file_read_bytes: u64,
    pub max_http_response_bytes: u64,
}

impl Default for LimitSettings {
    fn default() -> Self {
        Self {
            max_file_download_bytes: DEFAULT_MAX_FILE_DOWNLOAD_BYTES,
            max_file_read_bytes: DEFAULT_MAX_FILE_READ_BYTES,
            max_http_response_bytes: DEFAULT_MAX_HTTP_RESPONSE_BYTES,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct UpdateSettings {
    pub automatic_checks: bool,
    pub check_interval_hours: u64,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            automatic_checks: true,
            check_interval_hours: DEFAULT_UPDATE_CHECK_INTERVAL_HOURS,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DesktopSettings {
    pub keep_running_on_close: bool,
    pub launch_at_login: bool,
    pub start_background_runner_on_launch: bool,
    pub start_minimized_to_tray: bool,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            keep_running_on_close: true,
            launch_at_login: false,
            start_background_runner_on_launch: false,
            start_minimized_to_tray: false,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct SerialSettings {
    pub devices: BTreeMap<String, SerialDeviceSettings>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct SerialDeviceSettings {
    pub auto_reconnect: bool,
    pub auto_rebind_port: bool,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub dtr_on_open: String,
    pub flow_control: String,
    pub manufacturer: Option<String>,
    pub max_message_bytes: usize,
    pub message_gap_ms: u64,
    pub open_stabilization_ms: u64,
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
            baud_rate: DEFAULT_SERIAL_BAUD_RATE,
            data_bits: 8,
            dtr_on_open: DEFAULT_SERIAL_DTR_ON_OPEN.to_owned(),
            flow_control: "none".to_owned(),
            manufacturer: None,
            max_message_bytes: DEFAULT_SERIAL_MAX_MESSAGE_BYTES,
            message_gap_ms: DEFAULT_SERIAL_MESSAGE_GAP_MS,
            open_stabilization_ms: DEFAULT_SERIAL_OPEN_STABILIZATION_MS,
            parity: "none".to_owned(),
            port: String::new(),
            product_id: None,
            product: None,
            read_mode: DEFAULT_SERIAL_READ_MODE.to_owned(),
            serial_number: None,
            stop_bits: "1".to_owned(),
            validate_usb_identity: false,
            vendor_id: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct RunnerSettings {
    pub run_history_max_age_days: u64,
    pub run_history_max_records: usize,
    pub target_runtimes: Vec<String>,
    pub trigger_reload_seconds: u64,
}

impl Default for RunnerSettings {
    fn default() -> Self {
        Self {
            run_history_max_age_days: DEFAULT_RUN_HISTORY_MAX_AGE_DAYS,
            run_history_max_records: DEFAULT_RUN_HISTORY_MAX_RECORDS,
            target_runtimes: Vec::new(),
            trigger_reload_seconds: DEFAULT_TRIGGER_RELOAD_SECONDS,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct WebhookSettings {
    pub allow_browser_origins: Vec<String>,
    pub allow_unauthenticated_public_bind: bool,
    pub bind: String,
    pub max_body_bytes: usize,
    pub port: u16,
}

impl Default for WebhookSettings {
    fn default() -> Self {
        Self {
            allow_browser_origins: Vec::new(),
            allow_unauthenticated_public_bind: false,
            bind: DEFAULT_WEBHOOK_BIND.to_owned(),
            max_body_bytes: DEFAULT_WEBHOOK_MAX_BODY_BYTES,
            port: DEFAULT_WEBHOOK_PORT,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct WebSocketSettings {
    pub allow_browser_origins: Vec<String>,
    pub allow_unauthenticated_public_bind: bool,
    pub bind: String,
    pub max_connections: usize,
    pub max_message_bytes: usize,
    pub port: u16,
}

impl Default for WebSocketSettings {
    fn default() -> Self {
        Self {
            allow_browser_origins: Vec::new(),
            allow_unauthenticated_public_bind: false,
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

fn validate_serial_device(
    path: &Path,
    device_id: &str,
    device: &SerialDeviceSettings,
) -> Result<(), RunnerConfigError> {
    let invalid = |message: String| RunnerConfigError::Validate {
        path: path.to_path_buf(),
        message: format!("serial device {device_id:?} {message}"),
    };

    if device_id.is_empty()
        || device_id.len() > MAX_SERIAL_DEVICE_ID_LENGTH
        || !device_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
    {
        return Err(invalid(format!(
            "ID must contain 1 to {MAX_SERIAL_DEVICE_ID_LENGTH} lowercase letters, numbers, underscores, or hyphens"
        )));
    }
    validate_bounded_text(
        path,
        &format!("serial.devices.{device_id}.port"),
        &device.port,
        MAX_SERIAL_PORT_LENGTH,
        false,
        true,
    )?;
    for (field, value) in [
        ("manufacturer", &device.manufacturer),
        ("product", &device.product),
        ("serial_number", &device.serial_number),
    ] {
        if let Some(value) = value {
            validate_bounded_text(
                path,
                &format!("serial.devices.{device_id}.{field}"),
                value,
                MAX_SERIAL_METADATA_LENGTH,
                true,
                false,
            )?;
        }
    }
    validate_optional_usb_id(path, device_id, "vendor_id", &device.vendor_id)?;
    validate_optional_usb_id(path, device_id, "product_id", &device.product_id)?;
    if device.baud_rate == 0 {
        return Err(invalid("baud_rate must be greater than zero".to_owned()));
    }
    if !matches!(device.data_bits, 5..=8) {
        return Err(invalid("data_bits must be 5, 6, 7, or 8".to_owned()));
    }
    if !matches!(
        device.dtr_on_open.as_str(),
        "deasserted" | "asserted" | "preserve"
    ) {
        return Err(invalid(
            "dtr_on_open must be one of deasserted, asserted, or preserve".to_owned(),
        ));
    }
    if !matches!(device.parity.as_str(), "none" | "even" | "odd") {
        return Err(invalid(
            "parity must be one of none, even, or odd".to_owned(),
        ));
    }
    if !matches!(device.stop_bits.as_str(), "1" | "2") {
        return Err(invalid("stop_bits must be 1 or 2".to_owned()));
    }
    if !matches!(
        device.flow_control.as_str(),
        "none" | "software" | "hardware"
    ) {
        return Err(invalid(
            "flow_control must be one of none, software, or hardware".to_owned(),
        ));
    }
    if !matches!(device.read_mode.as_str(), "line" | "idle_gap" | "raw") {
        return Err(invalid(
            "read_mode must be one of line, idle_gap, or raw".to_owned(),
        ));
    }
    if device.message_gap_ms == 0 || device.message_gap_ms > MAX_SERIAL_MESSAGE_GAP_MS {
        return Err(invalid(format!(
            "message_gap_ms must be between 1 and {MAX_SERIAL_MESSAGE_GAP_MS}"
        )));
    }
    if device.max_message_bytes == 0 || device.max_message_bytes > MAX_SERIAL_MESSAGE_BYTES {
        return Err(invalid(format!(
            "max_message_bytes must be between 1 and {MAX_SERIAL_MESSAGE_BYTES}"
        )));
    }
    if device.open_stabilization_ms > MAX_SERIAL_OPEN_STABILIZATION_MS {
        return Err(invalid(format!(
            "open_stabilization_ms must be between 0 and {MAX_SERIAL_OPEN_STABILIZATION_MS}"
        )));
    }
    if device.validate_usb_identity {
        validate_usb_id(path, device_id, "vendor_id", &device.vendor_id)?;
        validate_usb_id(path, device_id, "product_id", &device.product_id)?;
    }
    if device.auto_rebind_port && !device.validate_usb_identity {
        return Err(invalid(
            "enables auto_rebind_port but validate_usb_identity is false".to_owned(),
        ));
    }
    Ok(())
}

fn validate_usb_id(
    path: &Path,
    device_id: &str,
    field: &str,
    value: &Option<String>,
) -> Result<(), RunnerConfigError> {
    let Some(value) = value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err(RunnerConfigError::Validate {
            path: path.to_path_buf(),
            message: format!(
                "serial device {device_id:?} enables validate_usb_identity but {field} is not set"
            ),
        });
    };
    let digits = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if digits.is_empty() || digits.len() > 4 || !digits.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(RunnerConfigError::Validate {
            path: path.to_path_buf(),
            message: format!(
                "serial device {device_id:?} {field} must be a 1 to 4 digit hexadecimal value"
            ),
        });
    }
    Ok(())
}

fn validate_optional_usb_id(
    path: &Path,
    device_id: &str,
    field: &str,
    value: &Option<String>,
) -> Result<(), RunnerConfigError> {
    let Some(value) = value.as_deref() else {
        return Ok(());
    };
    if value.is_empty() {
        return Ok(());
    }
    let digits = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if digits.is_empty() || digits.len() > 4 || !digits.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(RunnerConfigError::Validate {
            path: path.to_path_buf(),
            message: format!(
                "serial device {device_id:?} {field} must be a 1 to 4 digit hexadecimal value"
            ),
        });
    }
    Ok(())
}

fn validate_browser_origins(
    path: &Path,
    setting: &str,
    origins: &[String],
) -> Result<(), RunnerConfigError> {
    if origins.len() > MAX_BROWSER_ORIGINS {
        return Err(RunnerConfigError::Validate {
            path: path.to_path_buf(),
            message: format!("{setting} contains more than {MAX_BROWSER_ORIGINS} origins"),
        });
    }
    let mut unique = BTreeSet::new();
    for origin in origins {
        let parsed = Url::parse(origin).ok();
        let valid = origin.len() <= MAX_BROWSER_ORIGIN_LENGTH
            && origin == origin.trim()
            && !contains_unsafe_text(origin)
            && parsed.is_some_and(|parsed| {
                matches!(parsed.scheme(), "http" | "https")
                    && parsed.origin().ascii_serialization() == *origin
            });
        if !valid {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: format!(
                    "{setting} contains invalid origin {origin:?}; use an exact origin such as https://dashboard.example.com"
                ),
            });
        }
        if !unique.insert(origin) {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: format!("{setting} contains duplicate origin {origin:?}"),
            });
        }
    }
    Ok(())
}

fn validate_bind_address(path: &Path, setting: &str, value: &str) -> Result<(), RunnerConfigError> {
    if value != value.trim() || value.parse::<Ipv4Addr>().is_err() {
        return Err(RunnerConfigError::Validate {
            path: path.to_path_buf(),
            message: format!("{setting} must be an IPv4 address"),
        });
    }
    Ok(())
}

fn validate_target_runtimes(path: &Path, values: &[String]) -> Result<(), RunnerConfigError> {
    const ALLOWED: [&str; 4] = [
        "Linux Headless",
        "Windows Headless",
        "Windows Desktop",
        "Linux Desktop",
    ];
    let mut unique = BTreeSet::new();
    for value in values {
        if !ALLOWED.contains(&value.as_str()) {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: format!("runner.target_runtimes contains unknown value {value:?}"),
            });
        }
        if !unique.insert(value) {
            return Err(RunnerConfigError::Validate {
                path: path.to_path_buf(),
                message: format!("runner.target_runtimes contains duplicate value {value:?}"),
            });
        }
    }
    Ok(())
}

fn validate_bounded_text(
    path: &Path,
    setting: &str,
    value: &str,
    max_length: usize,
    allow_empty: bool,
    require_trimmed: bool,
) -> Result<(), RunnerConfigError> {
    if (!allow_empty && value.is_empty())
        || value.len() > max_length
        || (require_trimmed && value != value.trim())
        || contains_unsafe_text(value)
    {
        return Err(RunnerConfigError::Validate {
            path: path.to_path_buf(),
            message: format!(
                "{setting} must {}contain at most {max_length} bytes without unsafe control characters{}",
                if allow_empty { "" } else { "not be empty and " },
                if require_trimmed {
                    " or surrounding whitespace"
                } else {
                    ""
                }
            ),
        });
    }
    Ok(())
}

fn contains_unsafe_text(value: &str) -> bool {
    value.chars().any(|character| {
        character.is_control()
            || matches!(
                character,
                '\u{061c}'
                    | '\u{200e}'
                    | '\u{200f}'
                    | '\u{202a}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_uses_safe_defaults() {
        let temporary_directory = tempfile::tempdir().expect("temp dir");
        let config_path = temporary_directory.path().join("runner.toml");

        let config = RunnerConfig::load_or_default(&config_path).expect("config should load");

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
        assert_eq!(
            config.limits.max_http_response_bytes,
            DEFAULT_MAX_HTTP_RESPONSE_BYTES
        );
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

        RunnerConfig::load_or_init(&config_path).expect("config should load");

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
                trigger_reload_seconds = 5
                run_history_max_records = 2500
                run_history_max_age_days = 14
                target_runtimes = ["Windows Headless", "Linux Headless"]

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
                dtr_on_open = "asserted"
                parity = "none"
                stop_bits = "1"
                flow_control = "none"
                read_mode = "line"
                open_stabilization_ms = 750
                auto_reconnect = true
                auto_rebind_port = true
                validate_usb_identity = true
                vendor_id = "1A86"
                product_id = "7523"
                manufacturer = "Prolific Technology Inc. "
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

        assert_eq!(config.runner.trigger_reload_seconds, 5);
        assert_eq!(config.runner.run_history_max_records, 2500);
        assert_eq!(config.runner.run_history_max_age_days, 14);
        assert_eq!(
            config.runner.target_runtimes,
            ["Windows Headless", "Linux Headless"]
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
        assert_eq!(device.dtr_on_open, "asserted");
        assert_eq!(device.open_stabilization_ms, 750);
        assert!(device.auto_rebind_port);
        assert_eq!(device.vendor_id.as_deref(), Some("1A86"));
        assert_eq!(
            device.manufacturer.as_deref(),
            Some("Prolific Technology Inc. ")
        );
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
            assert!(error.to_string().contains("must be between 1"));
        }
    }

    #[test]
    fn rejects_oversized_config_before_parsing() {
        let contents = "#".repeat(MAX_RUNNER_CONFIG_BYTES + 1);
        let error = RunnerConfig::from_toml(&contents, "runner.toml")
            .expect_err("oversized configuration must be rejected");

        assert!(error.to_string().contains("maximum size"));
    }

    #[test]
    fn rejects_invalid_bind_addresses_and_target_runtimes() {
        for contents in [
            "[webhooks]\nbind = \"localhost\"",
            "[webhooks]\nbind = \"::1\"",
            "[websockets]\nbind = \"127.0.0.1\\n\"",
            "[runner]\ntarget_runtimes = [\"Unknown Desktop\"]",
            "[runner]\ntarget_runtimes = [\"Linux Desktop\", \"Linux Desktop\"]",
        ] {
            RunnerConfig::from_toml(contents, "runner.toml")
                .expect_err("invalid bounded config text must be rejected");
        }
    }

    #[test]
    fn rejects_zero_external_data_limits() {
        for field in [
            "max_http_response_bytes",
            "max_file_download_bytes",
            "max_file_read_bytes",
        ] {
            let contents = format!("[limits]\n{field} = 0");
            let error = RunnerConfig::from_toml(&contents, "runner.toml")
                .expect_err("external data limits must be positive");
            assert!(error.to_string().contains(field), "{error}");
        }
    }

    #[test]
    fn rejects_zero_webhook_body_limit() {
        let error = RunnerConfig::from_toml("[webhooks]\nmax_body_bytes = 0", "runner.toml")
            .expect_err("webhook request bodies must stay bounded");

        assert!(error.to_string().contains("webhooks.max_body_bytes"));
    }

    #[test]
    fn validates_exact_browser_origins() {
        let config = RunnerConfig::from_toml(
            r#"
                [webhooks]
                allow_browser_origins = ["https://dashboard.example.com"]

                [websockets]
                allow_browser_origins = ["http://localhost:3000"]
            "#,
            "runner.toml",
        )
        .expect("exact HTTP and HTTPS origins should be accepted");

        assert_eq!(
            config.webhooks.allow_browser_origins,
            ["https://dashboard.example.com"]
        );
        assert_eq!(
            config.websockets.allow_browser_origins,
            ["http://localhost:3000"]
        );

        for origin in [
            "*",
            "dashboard.example.com",
            "https://dashboard.example.com/path",
            "https://user@dashboard.example.com",
            "https://dashboard.example.com:not-a-port",
            "https://dashboard.example.com/",
            " https://dashboard.example.com",
        ] {
            let contents = format!(
                "[webhooks]\nallow_browser_origins = [{}]",
                toml::Value::String(origin.to_owned())
            );
            let error = RunnerConfig::from_toml(&contents, "runner.toml")
                .expect_err("non-origin values must be rejected");
            assert!(error.to_string().contains("invalid origin"), "{error}");
        }
    }

    #[test]
    fn rejects_duplicate_browser_origins() {
        let error = RunnerConfig::from_toml(
            r#"
                [websockets]
                allow_browser_origins = [
                    "https://dashboard.example.com",
                    "https://dashboard.example.com",
                ]
            "#,
            "runner.toml",
        )
        .expect_err("duplicate browser origins should be rejected");

        assert!(error.to_string().contains("duplicate origin"));
    }

    #[test]
    fn rejects_excessive_browser_origins() {
        let mut config = RunnerConfig::default();
        config.webhooks.allow_browser_origins = (0..=MAX_BROWSER_ORIGINS)
            .map(|index| format!("https://{index}.example.com"))
            .collect();
        let contents = config.to_pretty_toml().expect("config should serialize");
        let error = RunnerConfig::from_toml(&contents, "runner.toml")
            .expect_err("excessive origin lists must be rejected");

        assert!(error.to_string().contains("more than"));
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
    fn serial_device_defaults_use_idle_gap_at_9600_baud() {
        let config = RunnerConfig::from_toml(
            "[serial.devices.controller]\nport = \"COM3\"",
            "runner.toml",
        )
        .expect("default serial device should parse");
        let device = &config.serial.devices["controller"];

        assert_eq!(device.baud_rate, DEFAULT_SERIAL_BAUD_RATE);
        assert_eq!(device.dtr_on_open, DEFAULT_SERIAL_DTR_ON_OPEN);
        assert_eq!(device.read_mode, DEFAULT_SERIAL_READ_MODE);
        assert_eq!(device.message_gap_ms, DEFAULT_SERIAL_MESSAGE_GAP_MS);
        assert_eq!(device.max_message_bytes, DEFAULT_SERIAL_MAX_MESSAGE_BYTES);
        assert_eq!(
            device.open_stabilization_ms,
            DEFAULT_SERIAL_OPEN_STABILIZATION_MS
        );
    }

    #[test]
    fn serial_framing_settings_round_trip_through_toml() {
        let mut config = RunnerConfig::default();
        config.serial.devices.insert(
            "controller".to_owned(),
            SerialDeviceSettings {
                max_message_bytes: 32_768,
                message_gap_ms: 275,
                port: "COM7".to_owned(),
                read_mode: "line".to_owned(),
                ..SerialDeviceSettings::default()
            },
        );

        let contents = config
            .to_pretty_toml()
            .expect("serial config should serialize");
        let restored = RunnerConfig::from_toml(&contents, "runner.toml")
            .expect("serialized serial config should parse");
        let device = &restored.serial.devices["controller"];

        assert_eq!(device.baud_rate, DEFAULT_SERIAL_BAUD_RATE);
        assert_eq!(device.read_mode, "line");
        assert_eq!(device.message_gap_ms, 275);
        assert_eq!(device.max_message_bytes, 32_768);
    }

    #[test]
    fn rejects_unknown_serial_framing_and_out_of_range_limits() {
        for contents in [
            "[serial.devices.controller]\nport = \"COM3\"\nread_mode = \"packet\"",
            "[serial.devices.controller]\nport = \"COM3\"\nmessage_gap_ms = 0",
            "[serial.devices.controller]\nport = \"COM3\"\nmax_message_bytes = 0",
            "[serial.devices.controller]\nport = \"COM3\"\ndtr_on_open = \"pulse\"",
            "[serial.devices.controller]\nport = \"COM3\"\nopen_stabilization_ms = 60001",
        ] {
            let error = RunnerConfig::from_toml(contents, "runner.toml")
                .expect_err("invalid serial framing must be rejected");
            assert!(matches!(error, RunnerConfigError::Validate { .. }));
        }
    }

    #[test]
    fn rejects_invalid_usb_identity_hex_values() {
        let error = RunnerConfig::from_toml(
            r#"
                [serial.devices.controller]
                port = "COM3"
                validate_usb_identity = true
                vendor_id = "not-hex"
                product_id = "7523"
            "#,
            "runner.toml",
        )
        .expect_err("invalid USB identity should fail");

        assert!(error.to_string().contains("vendor_id"));
    }

    #[test]
    fn rejects_unsafe_serial_device_text() {
        for contents in [
            "[serial.devices.\"Bad ID\"]\nport = \"COM3\"",
            "[serial.devices.controller]\nport = \" COM3\"",
            "[serial.devices.controller]\nport = \"COM3\"\nmanufacturer = \"trusted\\nspoofed\"",
            "[serial.devices.controller]\nport = \"COM3\"\nvendor_id = \"not-hex\"",
        ] {
            RunnerConfig::from_toml(contents, "runner.toml")
                .expect_err("unsafe serial configuration text must be rejected");
        }
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
    fn rejects_zero_update_check_interval() {
        let error = RunnerConfig::from_toml("[updates]\ncheck_interval_hours = 0", "runner.toml")
            .expect_err("update checks need a positive interval");

        assert!(error.to_string().contains("updates.check_interval_hours"));
    }

    #[test]
    fn saves_config_atomically_over_an_existing_file() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let path = directory.path().join("config.toml");
        fs::write(&path, "old contents").expect("initial config should be written");

        let mut config = RunnerConfig::default();
        config.display.time_format = TimeFormat::TwelveHour;
        config.desktop.launch_at_login = true;
        config.updates.check_interval_hours = 6;
        config.save(&path).expect("config should be replaced");

        let saved = RunnerConfig::load_or_init(&path).expect("saved config should load");
        assert_eq!(saved.display.time_format, TimeFormat::TwelveHour);
        assert!(saved.desktop.launch_at_login);
        assert_eq!(saved.updates.check_interval_hours, 6);
    }

    #[test]
    fn template_toml_parses_as_valid_config() {
        let config = RunnerConfig::from_toml(RunnerConfig::template_toml(), "template.toml")
            .expect("template should parse");

        assert_eq!(config.display.time_format, TimeFormat::TwentyFourHour);
        assert!(config.updates.automatic_checks);
        assert_eq!(
            config.updates.check_interval_hours,
            DEFAULT_UPDATE_CHECK_INTERVAL_HOURS
        );
        assert!(!config.desktop.launch_at_login);
        assert!(config.desktop.keep_running_on_close);
        assert_eq!(config.webhooks.port, DEFAULT_WEBHOOK_PORT);
        assert_eq!(
            config.limits.max_file_download_bytes,
            DEFAULT_MAX_FILE_DOWNLOAD_BYTES
        );
        assert_eq!(config.websockets.port, DEFAULT_WEBSOCKET_PORT);
        assert_eq!(
            config.websockets.max_connections,
            DEFAULT_WEBSOCKET_MAX_CONNECTIONS
        );
        assert!(!config.triggers.webhooks_enabled);
        assert!(!config.triggers.websockets_enabled);
    }
}
