//! Renders a datetime with a pattern such as `yyyy-MM-dd HH:mm`.
//!
//! The editor implements the same language in
//! `data/project/datetime-format.ts`; the two are held together by
//! `contracts/datetime-format-conformance.json`.
//!
//! A pattern is read in the offset the value carries, matching the `.$` parts,
//! so an author sees the wall clock the value was written in.
//!
//! Month and weekday names are English. Nothing else in the variable system
//! depends on locale, and making one field of one node depend on it would be a
//! surprising place to introduce it.

use chrono::{DateTime, Datelike as _, FixedOffset, Timelike as _};
use serde_json::Value;

/// Longest first, so `yyyy` is never read as two `yy`.
const TOKENS: [&str; 19] = [
    "yyyy", "MMMM", "EEEE", "MMM", "EEE", "yy", "MM", "dd", "HH", "hh", "mm", "ss", "M", "d", "H",
    "h", "a", "m", "s",
];

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Index 1 is Monday, matching the ISO weekday the `.$weekday` part reports.
const WEEKDAY_NAMES: [&str; 8] = [
    "",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// The datetime a value carries, or `None` when it is not a datetime.
fn parse_datetime(value: &Value) -> Option<DateTime<FixedOffset>> {
    if value.get("type").and_then(Value::as_str) != Some("datetime") {
        return None;
    }
    let text = value.get("value").and_then(Value::as_str)?;
    DateTime::parse_from_rfc3339(text).ok()
}

fn render_token(token: &str, moment: &DateTime<FixedOffset>) -> String {
    let month = moment.month() as usize;
    let weekday = moment.weekday().number_from_monday() as usize;
    let hour = moment.hour();
    let hour12 = if hour.is_multiple_of(12) {
        12
    } else {
        hour % 12
    };
    match token {
        "yyyy" => format!("{:04}", moment.year()),
        "yy" => format!("{:02}", moment.year().rem_euclid(100)),
        "MMMM" => MONTH_NAMES[month - 1].to_owned(),
        "MMM" => MONTH_NAMES[month - 1][..3].to_owned(),
        "MM" => format!("{month:02}"),
        "M" => month.to_string(),
        "dd" => format!("{:02}", moment.day()),
        "d" => moment.day().to_string(),
        "EEEE" => WEEKDAY_NAMES[weekday].to_owned(),
        "EEE" => WEEKDAY_NAMES[weekday][..3].to_owned(),
        "HH" => format!("{hour:02}"),
        "H" => hour.to_string(),
        "hh" => format!("{hour12:02}"),
        "h" => hour12.to_string(),
        "a" => if hour < 12 { "AM" } else { "PM" }.to_owned(),
        "mm" => format!("{:02}", moment.minute()),
        "m" => moment.minute().to_string(),
        "ss" => format!("{:02}", moment.second()),
        "s" => moment.second().to_string(),
        _ => String::new(),
    }
}

/// The token starting at `index`, if the pattern has one there.
fn token_at(pattern: &str, index: usize) -> Option<&'static str> {
    TOKENS
        .into_iter()
        .find(|token| pattern[index..].starts_with(token))
}

/// Reports the first problem with a pattern, or `Ok(())` when it is fine.
///
/// A run of letters that is not a token is refused rather than emitted as
/// itself, so a mistyped `YYYY` is an error rather than literal text in the
/// output.
pub fn validate_datetime_pattern(pattern: &str) -> Result<(), String> {
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
            // Stepping by one byte is safe: a non-ASCII byte is never
            // alphabetic here, and the loop only ever indexes by bytes.
            index += 1;
            continue;
        }
        let Some(token) = token_at(pattern, index) else {
            let run: String = pattern[index..]
                .chars()
                .take_while(char::is_ascii_alphabetic)
                .collect();
            return Err(format!(
                "\"{run}\" is not a format token. Quote it as '{run}' to use it as text."
            ));
        };
        index += token.len();
    }

    Ok(())
}

/// Renders the pattern, or `None` when the value is not a datetime.
#[must_use]
pub fn format_datetime(value: &Value, pattern: &str) -> Option<String> {
    let moment = parse_datetime(value)?;

    let bytes = pattern.as_bytes();
    let mut output = String::new();
    let mut index = 0;
    while index < pattern.len() {
        if bytes[index] == b'\'' {
            let Some(offset) = pattern[index + 1..].find('\'') else {
                output.push_str(&pattern[index + 1..]);
                break;
            };
            // Two quotes in a row are a literal quote, the usual escape.
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
            output.push_str(&render_token(token, &moment));
            index += token.len();
            continue;
        }
        let character = pattern[index..].chars().next().unwrap_or_default();
        output.push(character);
        index += character.len_utf8();
    }

    Some(output)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn datetime(text: &str) -> Value {
        json!({ "type": "datetime", "value": text })
    }

    #[test]
    fn a_pattern_renders_each_token() {
        let value = datetime("2026-07-03T14:30:45+03:00");

        assert_eq!(
            format_datetime(&value, "yyyy-MM-dd").as_deref(),
            Some("2026-07-03")
        );
        assert_eq!(format_datetime(&value, "HH:mm").as_deref(), Some("14:30"));
        assert_eq!(
            format_datetime(&value, "h:mm a").as_deref(),
            Some("2:30 PM")
        );
        assert_eq!(
            format_datetime(&value, "EEEE d MMMM yyyy").as_deref(),
            Some("Friday 3 July 2026")
        );
        assert_eq!(
            format_datetime(&value, "EEE MMM d").as_deref(),
            Some("Fri Jul 3")
        );
        assert_eq!(
            format_datetime(&value, "yy/M/d H:m:s").as_deref(),
            Some("26/7/3 14:30:45")
        );
    }

    #[test]
    fn a_pattern_is_read_in_the_offset_the_value_carries() {
        // Converting to the machine's zone would report a different hour than
        // the editor does for the same value.
        assert_eq!(
            format_datetime(&datetime("2026-07-03T14:30:45-05:00"), "HH").as_deref(),
            Some("14")
        );
    }

    #[test]
    fn text_is_kept_by_quoting_it() {
        let value = datetime("2026-07-03T14:30:45+03:00");

        assert_eq!(
            format_datetime(&value, "'on' EEEE").as_deref(),
            Some("on Friday")
        );
        assert_eq!(format_datetime(&value, "HH'h'mm").as_deref(), Some("14h30"));
        // Two quotes in a row are one literal quote.
        assert_eq!(format_datetime(&value, "''yyyy").as_deref(), Some("'2026"));
    }

    #[test]
    fn midnight_and_noon_read_as_twelve_in_a_twelve_hour_pattern() {
        assert_eq!(
            format_datetime(&datetime("2026-07-03T00:15:00+00:00"), "hh:mm a").as_deref(),
            Some("12:15 AM")
        );
        assert_eq!(
            format_datetime(&datetime("2026-07-03T12:15:00+00:00"), "hh:mm a").as_deref(),
            Some("12:15 PM")
        );
        assert_eq!(
            format_datetime(&datetime("2026-07-03T00:15:00+00:00"), "HH").as_deref(),
            Some("00")
        );
    }

    #[test]
    fn a_value_that_is_not_a_datetime_renders_nothing() {
        assert_eq!(format_datetime(&json!("2026-07-03"), "yyyy"), None);
        assert_eq!(
            format_datetime(&json!({ "type": "duration" }), "yyyy"),
            None
        );
        assert_eq!(format_datetime(&datetime("not a moment"), "yyyy"), None);
    }

    #[test]
    fn a_pattern_is_validated_before_it_runs() {
        assert!(validate_datetime_pattern("yyyy-MM-dd HH:mm").is_ok());
        assert!(validate_datetime_pattern("'YYYY' yyyy").is_ok());

        let mistyped = validate_datetime_pattern("YYYY").expect_err("YYYY is not a token");
        assert!(
            mistyped.contains("not a format token"),
            "unexpected message: {mistyped}"
        );
        assert!(validate_datetime_pattern("yyyy 'unclosed").is_err());
        assert!(validate_datetime_pattern("").is_err());
    }

    #[test]
    fn a_pattern_may_contain_text_that_is_not_ascii() {
        // Indexing walks bytes, so a multi-byte character between tokens has
        // to be stepped over whole rather than split.
        assert_eq!(
            format_datetime(&datetime("2026-07-03T14:30:45+00:00"), "HH→mm").as_deref(),
            Some("14→30")
        );
    }
}
