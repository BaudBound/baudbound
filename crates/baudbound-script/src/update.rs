use chrono::DateTime;
use jsonschema::Validator;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use url::Url;

use crate::package::schema::SCRIPT_UPDATE_SCHEMA_JSON;

pub const SCRIPT_UPDATE_FORMAT: &str = "baudbound.script-update";
pub const SCRIPT_UPDATE_FORMAT_VERSION: u32 = 1;
pub const MAX_RELEASE_NOTES_CHARS: usize = 65_536;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScriptUpdateDescriptor {
    pub format: String,
    pub format_version: u32,
    pub script_id: String,
    pub latest: ScriptUpdateRelease,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScriptUpdateRelease {
    pub version: String,
    pub package_url: String,
    pub sha256: String,
    pub size: u64,
    pub published_at: String,
    pub release_notes: String,
}

#[derive(Debug, Error)]
pub enum ScriptUpdateError {
    #[error("update descriptor contains invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("update descriptor schema is invalid: {0}")]
    SchemaContract(String),
    #[error("update descriptor does not match the schema: {0}")]
    Schema(String),
    #[error("update descriptor validation failed: {0}")]
    Validation(String),
}

pub fn parse_script_update_descriptor(
    bytes: &[u8],
) -> Result<ScriptUpdateDescriptor, ScriptUpdateError> {
    let value: Value = serde_json::from_slice(bytes)?;
    validate_schema(&value)?;
    let descriptor: ScriptUpdateDescriptor = serde_json::from_value(value)?;
    validate_script_update_descriptor(&descriptor)?;
    Ok(descriptor)
}

pub fn validate_script_update_descriptor(
    descriptor: &ScriptUpdateDescriptor,
) -> Result<(), ScriptUpdateError> {
    let mut errors = Vec::new();
    if descriptor.format != SCRIPT_UPDATE_FORMAT {
        errors.push("format is unsupported".to_owned());
    }
    if descriptor.format_version != SCRIPT_UPDATE_FORMAT_VERSION {
        errors.push("format_version is unsupported".to_owned());
    }
    if !is_supported_uuid(&descriptor.script_id) {
        errors.push("script_id is not a valid UUID".to_owned());
    }
    if Version::parse(&descriptor.latest.version).is_err() {
        errors.push("latest.version is not a valid semantic version".to_owned());
    }
    if descriptor.latest.size == 0 {
        errors.push("latest.size must be greater than zero".to_owned());
    }
    if descriptor.latest.release_notes.chars().count() > MAX_RELEASE_NOTES_CHARS {
        errors.push(format!(
            "latest.release_notes exceeds {MAX_RELEASE_NOTES_CHARS} characters"
        ));
    }
    if !DateTime::parse_from_rfc3339(&descriptor.latest.published_at)
        .map(|value| value.offset().local_minus_utc() == 0)
        .unwrap_or(false)
    {
        errors.push("latest.published_at is not a valid UTC timestamp".to_owned());
    }
    if descriptor.latest.release_notes.chars().any(|character| {
        (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
            || matches!(
                character,
                '\u{061c}'
                    | '\u{200e}'
                    | '\u{200f}'
                    | '\u{202a}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            )
    }) {
        errors.push("latest.release_notes contains unsafe control characters".to_owned());
    }
    if validate_public_https_package_url(&descriptor.latest.package_url).is_err() {
        errors.push(
            "latest.package_url must be an HTTPS URL without credentials or a fragment and must end in .bbs"
                .to_owned(),
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ScriptUpdateError::Validation(errors.join("; ")))
    }
}

fn is_supported_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23]
            .into_iter()
            .all(|index| bytes[index] == b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| [8, 13, 18, 23].contains(&index) || byte.is_ascii_hexdigit())
        && matches!(bytes[14], b'1'..=b'8')
        && matches!(bytes[19].to_ascii_lowercase(), b'8' | b'9' | b'a' | b'b')
}

pub fn validate_public_https_package_url(value: &str) -> Result<Url, ScriptUpdateError> {
    let url = Url::parse(value)
        .map_err(|_| ScriptUpdateError::Validation("package URL is invalid".to_owned()))?;
    let valid = url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url
            .path_segments()
            .and_then(Iterator::last)
            .is_some_and(|name| name.to_ascii_lowercase().ends_with(".bbs"));
    if valid {
        Ok(url)
    } else {
        Err(ScriptUpdateError::Validation(
            "package URL violates the public remote package policy".to_owned(),
        ))
    }
}

fn validate_schema(value: &Value) -> Result<(), ScriptUpdateError> {
    let schema: Value = serde_json::from_str(SCRIPT_UPDATE_SCHEMA_JSON)
        .map_err(|error| ScriptUpdateError::SchemaContract(error.to_string()))?;
    let validator = Validator::new(&schema)
        .map_err(|error| ScriptUpdateError::SchemaContract(error.to_string()))?;
    let errors = validator
        .iter_errors(value)
        .take(20)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ScriptUpdateError::Schema(errors.join("; ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_descriptor() -> ScriptUpdateDescriptor {
        ScriptUpdateDescriptor {
            format: SCRIPT_UPDATE_FORMAT.to_owned(),
            format_version: SCRIPT_UPDATE_FORMAT_VERSION,
            script_id: "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7".to_owned(),
            latest: ScriptUpdateRelease {
                version: "1.2.0".to_owned(),
                package_url: "https://example.com/releases/example-1.2.0.bbs".to_owned(),
                sha256: "a".repeat(64),
                size: 123_456,
                published_at: "2026-07-22T12:00:00Z".to_owned(),
                release_notes: "A tested release.".to_owned(),
            },
        }
    }

    #[test]
    fn parses_a_valid_descriptor() {
        let bytes = serde_json::to_vec(&valid_descriptor()).expect("descriptor should serialize");
        assert_eq!(
            parse_script_update_descriptor(&bytes)
                .expect("descriptor should parse")
                .latest
                .version,
            "1.2.0"
        );
    }

    #[test]
    fn rejects_unknown_fields_and_unsafe_urls() {
        let mut value =
            serde_json::to_value(valid_descriptor()).expect("descriptor should serialize");
        value["unexpected"] = serde_json::json!(true);
        assert!(matches!(
            parse_script_update_descriptor(&serde_json::to_vec(&value).unwrap()),
            Err(ScriptUpdateError::Schema(_))
        ));

        let mut descriptor = valid_descriptor();
        descriptor.latest.package_url = "http://127.0.0.1/package.bbs".to_owned();
        assert!(validate_script_update_descriptor(&descriptor).is_err());
    }

    #[test]
    fn rejects_invalid_release_contract_values() {
        let updates: [fn(&mut ScriptUpdateDescriptor); 5] = [
            |descriptor: &mut ScriptUpdateDescriptor| {
                descriptor.latest.version = "latest".to_owned()
            },
            |descriptor: &mut ScriptUpdateDescriptor| descriptor.latest.sha256 = "A".repeat(64),
            |descriptor: &mut ScriptUpdateDescriptor| descriptor.latest.size = 0,
            |descriptor: &mut ScriptUpdateDescriptor| {
                descriptor.latest.published_at = "2026-07-22T12:00:00+03:00".to_owned()
            },
            |descriptor: &mut ScriptUpdateDescriptor| {
                descriptor.latest.release_notes = "trusted\u{202e}spoofed".to_owned()
            },
        ];
        for update in updates {
            let mut descriptor = valid_descriptor();
            update(&mut descriptor);
            let bytes = serde_json::to_vec(&descriptor).expect("descriptor should serialize");
            assert!(parse_script_update_descriptor(&bytes).is_err());
        }
    }

    #[test]
    fn rejects_missing_fields_and_invalid_identity() {
        let mut value =
            serde_json::to_value(valid_descriptor()).expect("descriptor should serialize");
        value.as_object_mut().unwrap().remove("latest");
        assert!(parse_script_update_descriptor(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut descriptor = valid_descriptor();
        descriptor.script_id = "not-a-uuid".to_owned();
        assert!(parse_script_update_descriptor(&serde_json::to_vec(&descriptor).unwrap()).is_err());
    }
}
