use anyhow::{Result, anyhow};
use baudbound_core::{RunnerConfig, TimeFormat};
use chrono::{DateTime, Local};

#[derive(Debug, Clone, Copy)]
pub struct CliTimeFormatter {
    time_format: TimeFormat,
}

impl CliTimeFormatter {
    pub const fn from_config(config: &RunnerConfig) -> Self {
        Self {
            time_format: config.display.time_format,
        }
    }

    pub fn format_unix_seconds(self, value: u64) -> Result<String> {
        let seconds = i64::try_from(value).map_err(|_| anyhow!("timestamp is out of range"))?;
        let timestamp = DateTime::from_timestamp(seconds, 0)
            .ok_or_else(|| anyhow!("timestamp is out of range"))?
            .with_timezone(&Local);
        Ok(self.format(timestamp))
    }

    pub fn format_unix_milliseconds(self, value: u64) -> Result<String> {
        let milliseconds =
            i64::try_from(value).map_err(|_| anyhow!("timestamp is out of range"))?;
        let timestamp = DateTime::from_timestamp_millis(milliseconds)
            .ok_or_else(|| anyhow!("timestamp is out of range"))?
            .with_timezone(&Local);
        Ok(self.format(timestamp))
    }

    fn format(self, timestamp: DateTime<Local>) -> String {
        match self.time_format {
            TimeFormat::TwelveHour => timestamp.format("%Y-%m-%d %I:%M:%S %p").to_string(),
            TimeFormat::TwentyFourHour => timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_twelve_and_twenty_four_hour_clocks() {
        let timestamp = DateTime::parse_from_rfc3339("2026-07-17T13:05:09+00:00")
            .expect("sample timestamp should parse")
            .with_timezone(&Local);

        let twelve = CliTimeFormatter {
            time_format: TimeFormat::TwelveHour,
        }
        .format(timestamp);
        let twenty_four = CliTimeFormatter {
            time_format: TimeFormat::TwentyFourHour,
        }
        .format(timestamp);

        assert!(twelve.ends_with("PM"));
        assert!(!twenty_four.contains("PM"));
    }
}
