//! Renders elapsed durations with a pattern such as `D HH:mm:ss`.
//!
//! The editor implements the same language in `data/project/duration-format.ts`.
//! Shared fixtures ensure authoring-time simulation and the runner agree.

use serde_json::Value;

const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
const TOKENS: [&str; 11] = ["SSS", "DD", "HH", "mm", "ss", "SS", "D", "H", "m", "s", "S"];

#[derive(Clone, Copy)]
struct DurationFields {
    days: u64,
    hours: u64,
    milliseconds: u64,
    minutes: u64,
    seconds: u64,
}

fn milliseconds_per_unit(unit: &str) -> Option<f64> {
    match unit {
        "milliseconds" => Some(1.0),
        "seconds" => Some(1_000.0),
        "minutes" => Some(60_000.0),
        "hours" => Some(3_600_000.0),
        "days" => Some(86_400_000.0),
        _ => None,
    }
}

fn token_at(pattern: &str, index: usize) -> Option<&'static str> {
    TOKENS
        .into_iter()
        .find(|token| pattern[index..].starts_with(token))
}

/// Refuses units outside the stable format-duration vocabulary.
pub fn validate_duration_unit(unit: &str) -> Result<(), String> {
    if milliseconds_per_unit(unit).is_some() {
        Ok(())
    } else {
        Err(format!("unsupported duration unit {unit:?}"))
    }
}

/// Reports the first problem with a duration pattern, or `Ok(())` when valid.
pub fn validate_duration_pattern(pattern: &str) -> Result<(), String> {
    if pattern.is_empty() {
        return Err("a format pattern is required".to_owned());
    }

    let bytes = pattern.as_bytes();
    let mut index = 0;
    while index < pattern.len() {
        if bytes[index] == b'\'' {
            let Some(offset) = pattern[index + 1..].find('\'') else {
                return Err("a quoted section is missing its closing '".to_owned());
            };
            index += offset + 2;
            continue;
        }
        if !bytes[index].is_ascii_alphabetic() {
            index += 1;
            continue;
        }
        let Some(token) = token_at(pattern, index) else {
            let run: String = pattern[index..]
                .chars()
                .take_while(char::is_ascii_alphabetic)
                .collect();
            return Err(format!(
                "\"{run}\" is not a duration format token. Quote it as '{run}' to use it as text."
            ));
        };
        index += token.len();
    }
    Ok(())
}

/// Renders a finite non-negative numeric value, or `None` for an invalid input.
#[must_use]
pub fn format_duration(value: &Value, unit: &str, pattern: &str) -> Option<String> {
    let amount = match value {
        Value::Number(number) => number.as_f64()?,
        Value::String(text) if !text.trim().is_empty() => text.trim().parse::<f64>().ok()?,
        _ => return None,
    };
    let total_milliseconds = amount * milliseconds_per_unit(unit)?;
    if !total_milliseconds.is_finite() || total_milliseconds < 0.0 {
        return None;
    }
    let total_milliseconds = total_milliseconds.round();
    if total_milliseconds > MAX_SAFE_INTEGER {
        return None;
    }
    let fields = duration_fields(total_milliseconds as u64);

    let bytes = pattern.as_bytes();
    let mut output = String::new();
    let mut index = 0;
    while index < pattern.len() {
        if bytes[index] == b'\'' {
            let Some(offset) = pattern[index + 1..].find('\'') else {
                output.push_str(&pattern[index + 1..]);
                break;
            };
            if offset == 0 {
                output.push('\'');
            } else {
                output.push_str(&pattern[index + 1..index + 1 + offset]);
            }
            index += offset + 2;
            continue;
        }
        if bytes[index].is_ascii_alphabetic()
            && let Some(token) = token_at(pattern, index)
        {
            output.push_str(&render_token(token, fields));
            index += token.len();
            continue;
        }
        let character = pattern[index..].chars().next().unwrap_or_default();
        output.push(character);
        index += character.len_utf8();
    }
    Some(output)
}

fn duration_fields(total_milliseconds: u64) -> DurationFields {
    const DAY: u64 = 86_400_000;
    const HOUR: u64 = 3_600_000;
    const MINUTE: u64 = 60_000;
    const SECOND: u64 = 1_000;

    let days = total_milliseconds / DAY;
    let after_days = total_milliseconds % DAY;
    DurationFields {
        days,
        hours: after_days / HOUR,
        minutes: (after_days % HOUR) / MINUTE,
        seconds: (after_days % MINUTE) / SECOND,
        milliseconds: after_days % SECOND,
    }
}

fn render_token(token: &str, fields: DurationFields) -> String {
    match token {
        "DD" => format!("{:02}", fields.days),
        "D" => fields.days.to_string(),
        "HH" => format!("{:02}", fields.hours),
        "H" => fields.hours.to_string(),
        "mm" => format!("{:02}", fields.minutes),
        "m" => fields.minutes.to_string(),
        "ss" => format!("{:02}", fields.seconds),
        "s" => fields.seconds.to_string(),
        "SSS" => format!("{:03}", fields.milliseconds),
        "SS" => format!("{:02}", fields.milliseconds / 10),
        "S" => (fields.milliseconds / 100).to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use super::{TOKENS, format_duration, validate_duration_pattern, validate_duration_unit};

    #[test]
    fn renders_elapsed_time_components() {
        assert_eq!(
            format_duration(&json!(90_061), "seconds", "D HH:mm:ss").as_deref(),
            Some("1 01:01:01")
        );
        assert_eq!(
            format_duration(&json!(3_723_004), "milliseconds", "DD HH:mm:ss.SSS").as_deref(),
            Some("00 01:02:03.004")
        );
    }

    #[test]
    fn accepts_integer_float_and_numeric_text_inputs() {
        assert_eq!(
            format_duration(&json!(1.2345), "seconds", "ss.SSS").as_deref(),
            Some("01.235")
        );
        assert_eq!(
            format_duration(&json!("90"), "minutes", "D HH:mm:ss").as_deref(),
            Some("0 01:30:00")
        );
    }

    #[test]
    fn rejects_invalid_values_units_and_patterns() {
        for value in [json!(""), json!("not a number"), json!(-1), json!([])] {
            assert!(format_duration(&value, "seconds", "HH:mm:ss").is_none());
        }
        assert!(format_duration(&json!(1), "weeks", "HH:mm:ss").is_none());
        assert!(validate_duration_unit("seconds").is_ok());
        assert!(validate_duration_unit("second").is_err());
        assert!(validate_duration_pattern("D HH:mm:ss.SSS").is_ok());
        assert!(validate_duration_pattern("YYYY").is_err());
        assert!(validate_duration_pattern("").is_err());
    }

    #[derive(Deserialize)]
    struct Conformance {
        cases: Vec<Case>,
        tokens: Vec<String>,
        units: Vec<String>,
        version: u32,
    }

    #[derive(Deserialize)]
    struct Case {
        amount: f64,
        #[serde(default)]
        error: bool,
        pattern: String,
        reason: String,
        #[serde(default)]
        result: Option<String>,
        unit: String,
    }

    #[test]
    fn shared_format_fixtures_conform() {
        let conformance: Conformance = serde_json::from_str(include_str!(
            "../../../../contracts/duration-format-conformance.json"
        ))
        .expect("shared duration fixtures should parse");
        assert_eq!(conformance.version, 1);
        let mut shared = conformance.tokens;
        let mut ours = TOKENS.map(str::to_owned).to_vec();
        shared.sort();
        ours.sort();
        assert_eq!(shared, ours);
        assert_eq!(
            conformance.units,
            ["milliseconds", "seconds", "minutes", "hours", "days"].map(str::to_owned)
        );

        for case in conformance.cases {
            let validity = validate_duration_pattern(&case.pattern);
            if case.error {
                assert!(
                    validity.is_err(),
                    "{:?} should be refused: {}",
                    case.pattern,
                    case.reason
                );
                continue;
            }
            validity.unwrap_or_else(|error| {
                panic!("{:?} should be valid but reported {error}", case.pattern)
            });
            assert_eq!(
                format_duration(&json!(case.amount), &case.unit, &case.pattern).as_deref(),
                case.result.as_deref(),
                "{:?} using {}: {}",
                case.pattern,
                case.unit,
                case.reason
            );
        }
    }
}
