use std::collections::BTreeSet;

use baudbound_security::{BlacklistDecision, BlacklistMatchSubject, matching_entries};
use baudbound_storage::{ScriptStore, SqliteRunnerStore};

use super::{
    error::BlacklistError,
    models::{BlacklistIncident, PersistedState},
};

pub(super) fn apply_quarantine(
    state: &mut PersistedState,
    store: &SqliteRunnerStore,
) -> Result<BTreeSet<String>, BlacklistError> {
    let mut critical_scripts = BTreeSet::new();
    for repository in store
        .list_repository_sources()
        .map_err(BlacklistError::Storage)?
    {
        let decision = BlacklistDecision::from_entries(matching_entries(
            &state.entries,
            &BlacklistMatchSubject::url(&repository.url),
        ));
        if decision.blocks_distribution() {
            let now = crate::paths::current_unix_timestamp();
            for entry in decision.entries {
                if !state.incidents.iter().any(|incident| {
                    incident.entry_id == entry.id
                        && incident.repository_url.as_deref() == Some(repository.url.as_str())
                }) {
                    tracing::warn!(
                        blacklist_entry_id = %entry.id,
                        blacklist_scope = ?entry.scope,
                        blacklist_severity = ?entry.severity,
                        blacklist_reason = %entry.reason,
                        repository_url = %repository.url,
                        "Official blacklist entry matched a configured repository"
                    );
                    state.incidents.push(BlacklistIncident {
                        advisory_url: entry.advisory_url,
                        entry_id: entry.id,
                        published_at: entry.published_at,
                        reason: entry.reason,
                        recorded_at_unix: now,
                        scope: entry.scope,
                        repository_url: Some(repository.url.clone()),
                        script_id: None,
                        severity: entry.severity,
                        title: entry.title,
                    });
                }
            }
        }
    }
    for script in store.list_scripts().map_err(BlacklistError::Storage)? {
        let mut subject = BlacklistMatchSubject::installed(&script.id, &script.package_hash);
        if let Some(provenance) = state.provenance.get(&script.id) {
            if let Some(repository_url) = &provenance.repository_url {
                subject.trusted_urls.push(repository_url.clone());
                if let Some(urls) = state.repository_provenance.get(repository_url) {
                    subject.trusted_urls.extend(urls.iter().cloned());
                }
            }
            subject
                .trusted_urls
                .extend(provenance.package_urls.iter().cloned());
            if let Some(final_url) = &provenance.final_package_url {
                subject.trusted_urls.push(final_url.clone());
            }
        }
        let decision = BlacklistDecision::from_entries(matching_entries(&state.entries, &subject));
        if decision.blocks_distribution() {
            let now = crate::paths::current_unix_timestamp();
            for entry in &decision.entries {
                if !state.incidents.iter().any(|incident| {
                    incident.entry_id == entry.id
                        && incident.script_id.as_deref() == Some(script.id.as_str())
                }) {
                    tracing::warn!(
                        blacklist_entry_id = %entry.id,
                        blacklist_scope = ?entry.scope,
                        blacklist_severity = ?entry.severity,
                        blacklist_reason = %entry.reason,
                        script_id = %script.id,
                        "Official blacklist entry matched an installed script"
                    );
                    state.incidents.push(BlacklistIncident {
                        advisory_url: entry.advisory_url.clone(),
                        entry_id: entry.id.clone(),
                        published_at: entry.published_at.clone(),
                        reason: entry.reason.clone(),
                        recorded_at_unix: now,
                        scope: entry.scope,
                        repository_url: None,
                        script_id: Some(script.id.clone()),
                        severity: entry.severity,
                        title: entry.title.clone(),
                    });
                }
            }
        }
        if decision.blocks_execution() {
            if decision.requests_cancellation() {
                critical_scripts.insert(script.id.clone());
            }
            if script.enabled {
                store
                    .set_script_enabled(&script.id, false)
                    .map_err(BlacklistError::Storage)?;
            }
            state.quarantined_scripts.insert(script.id.clone());
        }
    }
    state.incidents.sort_by_key(|incident| {
        std::cmp::Reverse((incident.recorded_at_unix, incident.entry_id.clone()))
    });
    state.incidents.truncate(1_000);
    Ok(critical_scripts)
}
