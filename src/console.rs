use std::{
    fmt::{self, Write as _},
    io::{self, Write},
    sync::Mutex,
};

use chrono::{DateTime, Utc};
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
    registry::LookupSpan,
};

static CONSOLE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
    Debug,
    Error,
    Info,
    Warn,
}

impl ConsoleLevel {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Error => "error",
            Self::Info => "info",
            Self::Warn => "warn",
        }
    }

    pub fn from_runtime(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "debug" => Self::Debug,
            "error" => Self::Error,
            "warn" | "warning" => Self::Warn,
            _ => Self::Info,
        }
    }
}

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .event_format(BracketedEventFormatter)
        .init();
}

pub fn error(arguments: fmt::Arguments<'_>) {
    write_now(ConsoleLevel::Error, arguments);
}

pub fn info(arguments: fmt::Arguments<'_>) {
    write_now(ConsoleLevel::Info, arguments);
}

pub fn write_at(level: ConsoleLevel, timestamp_unix_ms: u64, arguments: fmt::Arguments<'_>) {
    let timestamp =
        timestamp_from_unix_milliseconds(timestamp_unix_ms).unwrap_or_else(timestamp_now);
    write_formatted(level, &timestamp, &arguments.to_string());
}

fn write_now(level: ConsoleLevel, arguments: fmt::Arguments<'_>) {
    write_formatted(level, &timestamp_now(), &arguments.to_string());
}

fn write_formatted(level: ConsoleLevel, timestamp: &str, message: &str) {
    let Ok(_guard) = CONSOLE_LOCK.lock() else {
        return;
    };
    match level {
        ConsoleLevel::Error | ConsoleLevel::Warn => {
            let mut output = io::stderr().lock();
            write_lines(&mut output, timestamp, level, message);
        }
        ConsoleLevel::Debug | ConsoleLevel::Info => {
            let mut output = io::stdout().lock();
            write_lines(&mut output, timestamp, level, message);
        }
    }
}

fn write_lines(output: &mut impl Write, timestamp: &str, level: ConsoleLevel, message: &str) {
    let message = visible_text(message);
    let _ = writeln!(output, "{}", format_line(timestamp, level, &message));
}

pub fn visible_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\0' => output.push_str("\\0"),
            character if character.is_control() => {
                let _ = write!(output, "\\u{{{:x}}}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output
}

fn format_line(timestamp: &str, level: ConsoleLevel, message: &str) -> String {
    if message.is_empty() {
        format!("[{timestamp}] [{}]", level.as_str())
    } else {
        format!("[{timestamp}] [{}] {message}", level.as_str())
    }
}

fn timestamp_now() -> String {
    format_timestamp(Utc::now())
}

fn timestamp_from_unix_milliseconds(value: u64) -> Option<String> {
    let value = i64::try_from(value).ok()?;
    DateTime::from_timestamp_millis(value).map(format_timestamp)
}

fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

struct BracketedEventFormatter;

impl<S, N> FormatEvent<S, N> for BracketedEventFormatter
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        context: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let level = event.metadata().level().as_str().to_ascii_lowercase();
        write!(writer, "[{}] [{level}] ", timestamp_now())?;
        context
            .field_format()
            .format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_timestamps_with_milliseconds_and_brackets() {
        assert_eq!(
            timestamp_from_unix_milliseconds(1_784_628_110_120),
            Some("2026-07-21T10:01:50.120Z".to_owned())
        );
    }

    #[test]
    fn formats_every_console_line_with_bracketed_timestamp_and_level() {
        assert_eq!(
            format_line(
                "2026-07-21T00:41:50.120Z",
                ConsoleLevel::Info,
                "Runner started."
            ),
            "[2026-07-21T00:41:50.120Z] [info] Runner started."
        );
    }

    #[test]
    fn maps_runtime_levels_conservatively() {
        assert_eq!(ConsoleLevel::from_runtime("error"), ConsoleLevel::Error);
        assert_eq!(ConsoleLevel::from_runtime("warning"), ConsoleLevel::Warn);
        assert_eq!(ConsoleLevel::from_runtime("unknown"), ConsoleLevel::Info);
    }

    #[test]
    fn exposes_control_characters_in_console_text() {
        assert_eq!(
            visible_text("scanner\r\nnext\tvalue"),
            "scanner\\r\\nnext\\tvalue"
        );
    }
}
