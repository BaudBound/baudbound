use std::collections::BTreeSet;

use chrono::DateTime;
use jsonschema::Validator;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use url::Url;

use crate::package::schema::REPOSITORY_SCHEMA_JSON;
use crate::package::schema::{REPOSITORY_CAPABILITY_NAMES, REPOSITORY_PERMISSION_NAMES};

pub const SCRIPT_REPOSITORY_FORMAT: &str = "baudbound.repository";
pub const SCRIPT_REPOSITORY_FORMAT_VERSION: u32 = 1;
pub const MAX_REPOSITORY_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_REPOSITORY_SCRIPTS: usize = 1_000;
pub const MAX_RELEASE_NOTES_BYTES: usize = 8_000;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScriptRepository {
    pub format: String,
    pub format_version: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub homepage: String,
    pub scripts: Vec<ScriptRepositoryEntry>,
}

pub fn repository_capability_names() -> &'static [&'static str] {
    REPOSITORY_CAPABILITY_NAMES
}

pub fn repository_permission_names() -> &'static [&'static str] {
    REPOSITORY_PERMISSION_NAMES
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScriptRepositoryEntry {
    pub script_id: String,
    pub name: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub website: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license: String,
    pub target_runtime: String,
    pub minimum_runner_version: String,
    pub risk_level: String,
    pub tags: Vec<String>,
    pub permissions: Vec<String>,
    pub capabilities: Vec<String>,
    pub latest: ScriptRepositoryRelease,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScriptRepositoryRelease {
    pub version: String,
    pub package_url: String,
    pub sha256: String,
    pub size: u64,
    pub published_at: String,
    pub release_notes: String,
}

#[derive(Debug, Error)]
pub enum ScriptRepositoryError {
    #[error("repository exceeds the {MAX_REPOSITORY_BYTES} byte limit")]
    TooLarge,
    #[error("repository contains invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("repository schema is invalid: {0}")]
    SchemaContract(String),
    #[error("repository does not match the schema: {0}")]
    Schema(String),
    #[error("repository validation failed: {0}")]
    Validation(String),
}

pub fn parse_script_repository(bytes: &[u8]) -> Result<ScriptRepository, ScriptRepositoryError> {
    if bytes.len() > MAX_REPOSITORY_BYTES {
        return Err(ScriptRepositoryError::TooLarge);
    }
    let value: Value = serde_json::from_slice(bytes)?;
    validate_schema(&value)?;
    let repository: ScriptRepository = serde_json::from_value(value)?;
    validate_script_repository(&repository)?;
    Ok(repository)
}

pub fn validate_script_repository(
    repository: &ScriptRepository,
) -> Result<(), ScriptRepositoryError> {
    let mut errors = Vec::new();
    if repository.format != SCRIPT_REPOSITORY_FORMAT {
        errors.push("format is unsupported".to_owned());
    }
    if repository.format_version != SCRIPT_REPOSITORY_FORMAT_VERSION {
        errors.push("format_version is unsupported".to_owned());
    }
    validate_single_line_text("name", &repository.name, 160, false, &mut errors);
    validate_text(
        "description",
        &repository.description,
        4_000,
        true,
        &mut errors,
    );
    validate_optional_public_https_url("homepage", &repository.homepage, &mut errors);
    if repository.scripts.is_empty() || repository.scripts.len() > MAX_REPOSITORY_SCRIPTS {
        errors.push(format!(
            "scripts must contain between 1 and {MAX_REPOSITORY_SCRIPTS} entries"
        ));
    }

    let mut script_ids = BTreeSet::new();
    for (index, script) in repository.scripts.iter().enumerate() {
        let prefix = format!("scripts[{index}]");
        if !script_ids.insert(script.script_id.clone()) {
            errors.push(format!(
                "{prefix}.script_id duplicates {}",
                script.script_id
            ));
        }
        validate_single_line_text(
            &format!("{prefix}.name"),
            &script.name,
            128,
            false,
            &mut errors,
        );
        validate_single_line_text(
            &format!("{prefix}.summary"),
            &script.summary,
            500,
            false,
            &mut errors,
        );
        validate_text(
            &format!("{prefix}.description"),
            &script.description,
            4_000,
            true,
            &mut errors,
        );
        validate_single_line_text(
            &format!("{prefix}.author"),
            &script.author,
            128,
            true,
            &mut errors,
        );
        validate_single_line_text(
            &format!("{prefix}.license"),
            &script.license,
            128,
            true,
            &mut errors,
        );
        validate_optional_public_https_url(
            &format!("{prefix}.website"),
            &script.website,
            &mut errors,
        );
        validate_optional_public_https_url(
            &format!("{prefix}.source"),
            &script.source,
            &mut errors,
        );
        if Version::parse(&script.minimum_runner_version).is_err() {
            errors.push(format!(
                "{prefix}.minimum_runner_version is not a valid semantic version"
            ));
        }
        for (tag_index, tag) in script.tags.iter().enumerate() {
            validate_single_line_text(
                &format!("{prefix}.tags[{tag_index}]"),
                tag,
                64,
                false,
                &mut errors,
            );
        }
        validate_release(&prefix, &script.latest, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ScriptRepositoryError::Validation(errors.join("; ")))
    }
}

impl ScriptRepository {
    pub fn script(&self, script_id: &str) -> Option<&ScriptRepositoryEntry> {
        self.scripts
            .iter()
            .find(|script| script.script_id == script_id)
    }
}

pub fn validate_public_https_package_url(value: &str) -> Result<Url, ScriptRepositoryError> {
    validate_public_https_url(value, Some(".bbs"))
}

pub fn validate_public_https_repository_url(value: &str) -> Result<Url, ScriptRepositoryError> {
    validate_public_https_url(value, Some("repository.json"))
}

fn validate_release(
    script_prefix: &str,
    release: &ScriptRepositoryRelease,
    errors: &mut Vec<String>,
) {
    let prefix = format!("{script_prefix}.latest");
    if Version::parse(&release.version).is_err() {
        errors.push(format!("{prefix}.version is not a valid semantic version"));
    }
    if release.size == 0 {
        errors.push(format!("{prefix}.size must be greater than zero"));
    }
    if release.sha256.len() != 64
        || !release
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        errors.push(format!(
            "{prefix}.sha256 must contain 64 lowercase hexadecimal characters"
        ));
    }
    if release.release_notes.len() > MAX_RELEASE_NOTES_BYTES {
        errors.push(format!(
            "{prefix}.release_notes exceeds {MAX_RELEASE_NOTES_BYTES} UTF-8 bytes"
        ));
    }
    if contains_unsafe_text(&release.release_notes) {
        errors.push(format!(
            "{prefix}.release_notes contains unsafe control characters"
        ));
    }
    if !DateTime::parse_from_rfc3339(&release.published_at)
        .map(|value| value.offset().local_minus_utc() == 0)
        .unwrap_or(false)
    {
        errors.push(format!(
            "{prefix}.published_at is not a valid UTC timestamp"
        ));
    }
    if validate_public_https_package_url(&release.package_url).is_err() {
        errors.push(format!(
            "{prefix}.package_url must be a public HTTPS URL without credentials or a fragment and must end in .bbs"
        ));
    }
}

fn validate_text(
    field: &str,
    value: &str,
    maximum_bytes: usize,
    allow_empty: bool,
    errors: &mut Vec<String>,
) {
    if !allow_empty && value.trim().is_empty() {
        errors.push(format!("{field} is required"));
    }
    if value.len() > maximum_bytes {
        errors.push(format!("{field} exceeds {maximum_bytes} UTF-8 bytes"));
    }
    if contains_unsafe_text(value) {
        errors.push(format!("{field} contains unsafe control characters"));
    }
}

fn validate_single_line_text(
    field: &str,
    value: &str,
    maximum_bytes: usize,
    allow_empty: bool,
    errors: &mut Vec<String>,
) {
    validate_text(field, value, maximum_bytes, allow_empty, errors);
    if value
        .chars()
        .any(|character| matches!(character, '\n' | '\r' | '\t'))
    {
        errors.push(format!("{field} must use a single line"));
    }
}

fn validate_optional_public_https_url(field: &str, value: &str, errors: &mut Vec<String>) {
    if value.is_empty() {
        return;
    }
    if value.len() > 2_048 || validate_public_https_url(value, None).is_err() {
        errors.push(format!(
            "{field} must be a public HTTPS URL without credentials or a fragment"
        ));
    }
}

fn validate_public_https_url(
    value: &str,
    required_filename: Option<&str>,
) -> Result<Url, ScriptRepositoryError> {
    let url = Url::parse(value)
        .map_err(|_| ScriptRepositoryError::Validation("URL is invalid".to_owned()))?;
    let filename_matches = required_filename.is_none_or(|required| {
        url.path_segments()
            .and_then(Iterator::last)
            .is_some_and(|name| {
                if required.starts_with('.') {
                    name.to_ascii_lowercase().ends_with(required)
                } else {
                    name == required
                }
            })
    });
    let valid = value.len() <= 2_048
        && value == value.trim()
        && url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && filename_matches;
    if valid {
        Ok(url)
    } else {
        Err(ScriptRepositoryError::Validation(
            "URL violates the public remote resource policy".to_owned(),
        ))
    }
}

fn contains_unsafe_text(value: &str) -> bool {
    value.chars().any(|character| {
        (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
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

fn validate_schema(value: &Value) -> Result<(), ScriptRepositoryError> {
    let schema: Value = serde_json::from_str(REPOSITORY_SCHEMA_JSON)
        .map_err(|error| ScriptRepositoryError::SchemaContract(error.to_string()))?;
    let validator = Validator::new(&schema)
        .map_err(|error| ScriptRepositoryError::SchemaContract(error.to_string()))?;
    let errors = validator
        .iter_errors(value)
        .take(20)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ScriptRepositoryError::Schema(errors.join("; ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_repository() -> ScriptRepository {
        ScriptRepository {
            format: SCRIPT_REPOSITORY_FORMAT.to_owned(),
            format_version: SCRIPT_REPOSITORY_FORMAT_VERSION,
            name: "Test scripts".to_owned(),
            description: String::new(),
            homepage: String::new(),
            scripts: vec![ScriptRepositoryEntry {
                script_id: "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7".to_owned(),
                name: "Example".to_owned(),
                summary: "An example script.".to_owned(),
                description: String::new(),
                author: String::new(),
                website: String::new(),
                source: String::new(),
                license: String::new(),
                target_runtime: "Generic Desktop".to_owned(),
                minimum_runner_version: "2.0.0".to_owned(),
                risk_level: "low".to_owned(),
                tags: Vec::new(),
                permissions: vec!["log".to_owned()],
                capabilities: vec!["action.log".to_owned()],
                latest: ScriptRepositoryRelease {
                    version: "1.2.0".to_owned(),
                    package_url: "https://example.com/packages/example-1.2.0.bbs".to_owned(),
                    sha256: "a".repeat(64),
                    size: 123_456,
                    published_at: "2026-07-22T12:00:00Z".to_owned(),
                    release_notes: "A tested release.".to_owned(),
                },
            }],
        }
    }

    #[test]
    fn parses_and_finds_a_valid_repository_entry() {
        let repository = valid_repository();
        let bytes = serde_json::to_vec(&repository).expect("repository should serialize");
        let parsed = parse_script_repository(&bytes).expect("repository should parse");
        assert_eq!(
            parsed
                .script("6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7")
                .expect("script should exist")
                .latest
                .version,
            "1.2.0"
        );
    }

    #[test]
    fn exposes_filter_values_from_the_canonical_runner_contracts() {
        assert!(repository_permission_names().contains(&"log"));
        assert!(repository_capability_names().contains(&"action.log"));
    }

    #[test]
    fn rejects_the_legacy_descriptor_format() {
        let legacy = serde_json::json!({
            "format": "baudbound.script-update",
            "format_version": 1,
            "script_id": "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7",
            "latest": valid_repository().scripts[0].latest,
        });
        assert!(parse_script_repository(&serde_json::to_vec(&legacy).unwrap()).is_err());
    }

    #[test]
    fn rejects_duplicate_script_ids() {
        let mut repository = valid_repository();
        repository.scripts.push(repository.scripts[0].clone());
        assert!(validate_script_repository(&repository).is_err());
    }

    #[test]
    fn rejects_a_repository_without_a_name() {
        let mut repository = valid_repository();
        repository.name = "  ".to_owned();
        assert!(validate_script_repository(&repository).is_err());

        let mut value = serde_json::to_value(valid_repository()).unwrap();
        value
            .as_object_mut()
            .expect("repository should be an object")
            .remove("name");
        assert!(parse_script_repository(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn rejects_multiline_display_labels() {
        let mut repository = valid_repository();
        repository.name = "Trusted\nFake status".to_owned();
        assert!(validate_script_repository(&repository).is_err());

        let mut repository = valid_repository();
        repository.scripts[0].summary = "Summary\r\nForged log entry".to_owned();
        assert!(validate_script_repository(&repository).is_err());
    }

    #[test]
    fn accepts_a_repository_with_many_distinct_scripts() {
        let mut repository = valid_repository();
        for index in 1..100 {
            let mut script = repository.scripts[0].clone();
            script.script_id = format!("00000000-0000-4000-8000-{index:012x}");
            script.name = format!("Example {index}");
            repository.scripts.push(script);
        }
        let bytes = serde_json::to_vec(&repository).expect("repository should serialize");
        assert_eq!(
            parse_script_repository(&bytes)
                .expect("repository should parse")
                .scripts
                .len(),
            100
        );
    }

    #[test]
    fn rejects_unknown_fields_and_unsupported_versions() {
        let mut value = serde_json::to_value(valid_repository()).unwrap();
        value["unexpected"] = Value::Bool(true);
        assert!(parse_script_repository(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut value = serde_json::to_value(valid_repository()).unwrap();
        value["format_version"] = Value::from(2);
        assert!(parse_script_repository(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn rejects_invalid_release_claims_and_too_many_scripts() {
        let mut repository = valid_repository();
        repository.scripts[0].latest.version = "latest".to_owned();
        assert!(validate_script_repository(&repository).is_err());

        let mut repository = valid_repository();
        repository.scripts[0].latest.package_url =
            "https://example.com/packages/example.json".to_owned();
        assert!(validate_script_repository(&repository).is_err());

        let mut repository = valid_repository();
        repository.scripts = vec![repository.scripts[0].clone(); MAX_REPOSITORY_SCRIPTS + 1];
        assert!(validate_script_repository(&repository).is_err());
    }

    #[test]
    fn rejects_unsafe_release_values_and_oversized_documents() {
        let mut repository = valid_repository();
        repository.scripts[0].latest.release_notes = "trusted\u{202e}spoofed".to_owned();
        assert!(validate_script_repository(&repository).is_err());
        assert!(matches!(
            parse_script_repository(&vec![b' '; MAX_REPOSITORY_BYTES + 1]),
            Err(ScriptRepositoryError::TooLarge)
        ));
    }
}
