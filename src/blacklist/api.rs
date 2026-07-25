use std::collections::HashSet;

use baudbound_security::{
    BlacklistEntry, BlacklistScope, BlacklistSeverity, normalize_blacklist_entry,
};
use serde::Deserialize;

use super::error::BlacklistError;

const API_FIELDS: &str =
    "id,scope,target,subdomains,title,reason,severity,advisory_url,published_at,active,updated";
pub(super) const MAX_ENTRIES: usize = 5_000;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

pub(super) fn fetch_entries(
    backend_api: &crate::backend_api::BackendApiClient,
) -> Result<Vec<BlacklistEntry>, BlacklistError> {
    let records = backend_api.list_collection::<BlacklistApiRecord>(
        crate::backend_api::CollectionRequest {
            collection: "blacklist",
            fields: API_FIELDS,
            filter: Some("active = true"),
            maximum_records: MAX_ENTRIES,
            maximum_response_bytes: MAX_RESPONSE_BYTES,
            sort: Some("scope,target"),
        },
    )?;
    validate_records(records)
}

fn validate_records(
    records: Vec<BlacklistApiRecord>,
) -> Result<Vec<BlacklistEntry>, BlacklistError> {
    let mut entries = Vec::with_capacity(records.len());
    let mut seen_ids = HashSet::new();
    let mut seen_targets = HashSet::new();
    for record in records {
        if !record.active {
            return Err(BlacklistError::InvalidResponse(
                "the API returned an inactive blacklist entry".to_owned(),
            ));
        }
        let entry = normalize_blacklist_entry(record.into_entry()?)
            .map_err(|error| BlacklistError::InvalidResponse(error.to_string()))?;
        if !seen_ids.insert(entry.id.clone()) {
            return Err(BlacklistError::InvalidResponse(
                "the API returned a duplicate entry ID".to_owned(),
            ));
        }
        if !seen_targets.insert((entry.scope, entry.target.clone())) {
            return Err(BlacklistError::InvalidResponse(
                "the API returned a duplicate scope and target".to_owned(),
            ));
        }
        entries.push(entry);
    }
    Ok(entries)
}

#[derive(Debug, Deserialize)]
struct BlacklistApiRecord {
    active: bool,
    advisory_url: String,
    id: String,
    published_at: String,
    reason: String,
    scope: String,
    severity: String,
    subdomains: bool,
    target: String,
    title: String,
    updated: String,
}

impl BlacklistApiRecord {
    fn into_entry(self) -> Result<BlacklistEntry, BlacklistError> {
        let scope = match self.scope.as_str() {
            "repository" => BlacklistScope::Repository,
            "publisher" => BlacklistScope::Publisher,
            "domain" => BlacklistScope::Domain,
            "script" => BlacklistScope::Script,
            "package" => BlacklistScope::Package,
            _ => {
                return Err(BlacklistError::InvalidResponse(format!(
                    "unknown blacklist scope {:?}",
                    self.scope
                )));
            }
        };
        let severity = match self.severity.as_str() {
            "low" => BlacklistSeverity::Low,
            "medium" => BlacklistSeverity::Medium,
            "high" => BlacklistSeverity::High,
            "critical" => BlacklistSeverity::Critical,
            _ => {
                return Err(BlacklistError::InvalidResponse(format!(
                    "unknown blacklist severity {:?}",
                    self.severity
                )));
            }
        };
        Ok(BlacklistEntry {
            advisory_url: self.advisory_url,
            id: self.id,
            published_at: self.published_at,
            reason: self.reason,
            scope,
            severity,
            subdomains: self.subdomains,
            target: self.target,
            title: self.title,
            updated: self.updated,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api_record() -> BlacklistApiRecord {
        BlacklistApiRecord {
            active: true,
            advisory_url: "https://baudbound.app/advisories/test".to_owned(),
            id: "record123456789".to_owned(),
            published_at: "2026-07-25 12:00:00.000Z".to_owned(),
            reason: "A reviewed security concern".to_owned(),
            scope: "domain".to_owned(),
            severity: "high".to_owned(),
            subdomains: true,
            target: "malicious.example".to_owned(),
            title: "Test advisory".to_owned(),
            updated: "2026-07-25 12:05:00.000Z".to_owned(),
        }
    }

    #[test]
    fn public_field_request_contains_every_required_field_and_excludes_private_notes() {
        let fields = API_FIELDS.split(',').collect::<HashSet<_>>();
        for required in [
            "id",
            "scope",
            "target",
            "subdomains",
            "title",
            "reason",
            "severity",
            "advisory_url",
            "published_at",
            "active",
            "updated",
        ] {
            assert!(fields.contains(required), "missing API field {required}");
        }
        assert!(!fields.contains("private_notes"));
    }

    #[test]
    fn api_record_is_converted_and_normalized() {
        let entries = validate_records(vec![api_record()]).expect("record should convert");

        assert_eq!(entries[0].scope, BlacklistScope::Domain);
        assert_eq!(entries[0].severity, BlacklistSeverity::High);
        assert_eq!(entries[0].target, "malicious.example");
        assert!(entries[0].subdomains);
    }

    #[test]
    fn duplicate_targets_are_rejected() {
        let mut duplicate = api_record();
        duplicate.id = "another-record".to_owned();

        assert!(validate_records(vec![api_record(), duplicate]).is_err());
    }
}
