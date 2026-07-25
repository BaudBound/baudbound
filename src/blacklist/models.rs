use std::collections::{BTreeMap, BTreeSet};

use baudbound_runtime::RuntimeCancellationToken;
use baudbound_security::{BlacklistEntry, BlacklistScope, BlacklistSeverity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct TrustedProvenance {
    pub final_package_url: Option<String>,
    pub package_urls: Vec<String>,
    pub publishers: Vec<String>,
    pub repository_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct BlacklistIncident {
    #[serde(default)]
    pub advisory_url: String,
    pub entry_id: String,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub reason: String,
    pub recorded_at_unix: u64,
    pub scope: BlacklistScope,
    #[serde(default)]
    pub repository_url: Option<String>,
    pub script_id: Option<String>,
    pub severity: BlacklistSeverity,
    pub title: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(super) struct PersistedState {
    #[serde(default)]
    pub entries: Vec<BlacklistEntry>,
    pub fetched_at_unix: Option<u64>,
    pub last_error: Option<String>,
    #[serde(default)]
    pub incidents: Vec<BlacklistIncident>,
    #[serde(default)]
    pub personal_repository_blocks: BTreeSet<String>,
    #[serde(default)]
    pub repository_provenance: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub provenance: BTreeMap<String, TrustedProvenance>,
    #[serde(default)]
    pub quarantined_scripts: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BlacklistStatus {
    pub active_entry_count: usize,
    pub api_available: bool,
    pub entries: Vec<BlacklistEntry>,
    pub fetched_at_unix: Option<u64>,
    pub incidents: Vec<BlacklistIncident>,
    pub last_error: Option<String>,
    pub personal_repository_blocks: Vec<String>,
    pub stale: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CatalogExclusions {
    pub package_hashes: Vec<String>,
    pub repository_urls: Vec<String>,
    pub script_ids: Vec<String>,
}

#[derive(Debug)]
pub(super) struct ActiveRun {
    pub cancellation: RuntimeCancellationToken,
    pub script_id: String,
}
