use std::{
    io::Read,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use baudbound_storage::{SqliteRunnerStore, UpdateCheckCache};
use semver::Version;
use serde::{Deserialize, Serialize};

mod worker;

pub use worker::AutomaticUpdateWorker;

pub const RELEASE_FEED_URL: &str =
    "https://github.com/BaudBound/baudbound/releases/latest/download/latest.json";
const MAX_RELEASE_FEED_BYTES: u64 = 1024 * 1024;
const MAX_RELEASE_NOTES_BYTES: usize = 256 * 1024;
const MAX_PUBLISHED_AT_BYTES: usize = 128;

#[derive(Debug, Clone, Serialize)]
pub struct UpdateCheckResult {
    pub checked_at_unix: u64,
    pub current_version: String,
    pub latest_version: String,
    pub published_at: Option<String>,
    pub release_notes: Option<String>,
    pub update_available: bool,
}

#[derive(Debug, Deserialize)]
struct ReleaseFeed {
    version: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    pub_date: Option<String>,
}

pub fn check_now(store: &SqliteRunnerStore) -> Result<UpdateCheckResult> {
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(format!("BaudBound/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to create update client")?
        .get(RELEASE_FEED_URL)
        .send()
        .context("failed to request the BaudBound release feed")?
        .error_for_status()
        .context("the BaudBound release feed returned an error")?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RELEASE_FEED_BYTES)
    {
        return Err(anyhow!(
            "the BaudBound release feed exceeds {MAX_RELEASE_FEED_BYTES} bytes"
        ));
    }
    let mut body = Vec::new();
    response
        .take(MAX_RELEASE_FEED_BYTES + 1)
        .read_to_end(&mut body)
        .context("failed to read the BaudBound release feed")?;
    if body.len() as u64 > MAX_RELEASE_FEED_BYTES {
        return Err(anyhow!(
            "the BaudBound release feed exceeds {MAX_RELEASE_FEED_BYTES} bytes"
        ));
    }
    let feed: ReleaseFeed =
        serde_json::from_slice(&body).context("the BaudBound release feed is not valid JSON")?;
    let result = result_from_feed(feed, current_unix_timestamp()?)?;
    store
        .write_update_check_cache(&UpdateCheckCache {
            checked_at_unix: result.checked_at_unix,
            latest_version: result.latest_version.clone(),
            published_at: result.published_at.clone(),
            release_notes: result.release_notes.clone(),
            update_available: result.update_available,
        })
        .context("failed to cache update check result")?;
    Ok(result)
}

pub fn check_is_due(store: &SqliteRunnerStore, interval_hours: u64) -> Result<bool> {
    let Some(cache) = store
        .read_update_check_cache()
        .context("failed to read update check cache")?
    else {
        return Ok(true);
    };
    let now = current_unix_timestamp()?;
    let interval_seconds = interval_hours.saturating_mul(60 * 60);
    Ok(now.saturating_sub(cache.checked_at_unix) >= interval_seconds)
}

pub fn record_desktop_check(
    store: &SqliteRunnerStore,
    latest_version: Option<&str>,
    release_notes: Option<String>,
) -> Result<()> {
    validate_optional_text(
        "release notes",
        release_notes.as_deref(),
        MAX_RELEASE_NOTES_BYTES,
        true,
    )?;
    let current = current_version()?;
    let latest = latest_version
        .map(parse_version)
        .transpose()?
        .unwrap_or_else(|| current.clone());
    store
        .write_update_check_cache(&UpdateCheckCache {
            checked_at_unix: current_unix_timestamp()?,
            latest_version: latest.to_string(),
            published_at: None,
            release_notes,
            update_available: latest > current,
        })
        .context("failed to cache desktop update check")
}

fn result_from_feed(feed: ReleaseFeed, checked_at_unix: u64) -> Result<UpdateCheckResult> {
    validate_optional_text(
        "release notes",
        feed.notes.as_deref(),
        MAX_RELEASE_NOTES_BYTES,
        true,
    )?;
    validate_optional_text(
        "release publication date",
        feed.pub_date.as_deref(),
        MAX_PUBLISHED_AT_BYTES,
        false,
    )?;
    let current = current_version()?;
    let latest = parse_version(&feed.version)?;
    Ok(UpdateCheckResult {
        checked_at_unix,
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        published_at: feed.pub_date,
        release_notes: feed.notes.filter(|notes| !notes.trim().is_empty()),
        update_available: latest > current,
    })
}

fn validate_optional_text(
    label: &str,
    value: Option<&str>,
    max_bytes: usize,
    allow_line_breaks: bool,
) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let invalid_character = value.chars().any(|character| {
        let invalid_control = character.is_control()
            && !(allow_line_breaks && matches!(character, '\n' | '\r' | '\t'));
        invalid_control
            || matches!(
                character,
                '\u{061c}'
                    | '\u{200e}'
                    | '\u{200f}'
                    | '\u{202a}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            )
    });
    if value.len() > max_bytes || invalid_character {
        return Err(anyhow!(
            "{label} must contain at most {max_bytes} bytes and no unsafe control characters"
        ));
    }
    Ok(())
}

fn current_version() -> Result<Version> {
    parse_version(env!("CARGO_PKG_VERSION"))
}

fn parse_version(value: &str) -> Result<Version> {
    Version::parse(value.trim().trim_start_matches('v'))
        .with_context(|| format!("release feed contains invalid version {value:?}"))
}

fn current_unix_timestamp() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| anyhow!("system clock is before the Unix epoch: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_release_versions_and_compares_semantically() {
        let result = result_from_feed(
            ReleaseFeed {
                version: "v999.2.10".to_owned(),
                notes: Some(" Release notes ".to_owned()),
                pub_date: Some("2026-07-17T12:00:00Z".to_owned()),
            },
            42,
        )
        .expect("release feed should parse");

        assert_eq!(result.latest_version, "999.2.10");
        assert!(result.update_available);
        assert_eq!(result.checked_at_unix, 42);
    }

    #[test]
    fn rejects_invalid_release_versions() {
        assert!(parse_version("newest").is_err());
    }

    #[test]
    fn rejects_oversized_or_unsafe_release_metadata() {
        assert!(
            validate_optional_text(
                "release notes",
                Some(&"x".repeat(MAX_RELEASE_NOTES_BYTES + 1)),
                MAX_RELEASE_NOTES_BYTES,
                true,
            )
            .is_err()
        );
        assert!(
            validate_optional_text(
                "release notes",
                Some("trusted\u{202e}spoofed"),
                MAX_RELEASE_NOTES_BYTES,
                true,
            )
            .is_err()
        );
        assert!(
            validate_optional_text(
                "release notes",
                Some("line one\nline two"),
                MAX_RELEASE_NOTES_BYTES,
                true,
            )
            .is_ok()
        );
    }

    #[test]
    fn cached_success_suppresses_checks_until_the_interval_expires() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let store = SqliteRunnerStore::open(directory.path().join("runner.sqlite3"))
            .expect("runner store should open");
        record_desktop_check(&store, None, None).expect("desktop check should be cached");

        assert!(!check_is_due(&store, 24).expect("cache schedule should be readable"));
    }
}
