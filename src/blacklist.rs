use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::Duration,
};

use baudbound_runtime::{
    RunIdentity, RuntimeCancellationToken, RuntimeLogEntry, RuntimeRunObserver,
};
use baudbound_security::{
    BlacklistDecision, BlacklistMatchSubject, BlacklistPolicy, BlacklistScope, matching_entries,
    normalize_repository_url,
};
use baudbound_storage::SqliteRunnerStore;
use url::Url;

mod api;
mod cache;
mod enforcement;
mod error;
mod models;

use api::fetch_entries;
use cache::{load_state, save_state};
use enforcement::apply_quarantine;
pub(crate) use error::BlacklistError;
use models::{ActiveRun, PersistedState};
pub(crate) use models::{BlacklistStatus, CatalogExclusions, TrustedProvenance};

pub(crate) const REFRESH_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
pub(crate) const STALE_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

static GLOBAL: OnceLock<Arc<BlacklistService>> = OnceLock::new();

pub(crate) fn install_global(service: Arc<BlacklistService>) {
    let _ = GLOBAL.set(service);
}

pub(crate) fn global() -> Option<&'static Arc<BlacklistService>> {
    GLOBAL.get()
}

pub(crate) struct BlacklistService {
    active_runs: Mutex<BTreeMap<String, ActiveRun>>,
    backend_api: Arc<crate::backend_api::BackendApiClient>,
    change_callback: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    path: PathBuf,
    state: RwLock<PersistedState>,
}

impl BlacklistService {
    pub(crate) fn open(
        path: PathBuf,
        backend_api: Arc<crate::backend_api::BackendApiClient>,
    ) -> Result<Arc<Self>, BlacklistError> {
        let state = load_state(&path)?;
        Ok(Arc::new(Self {
            active_runs: Mutex::new(BTreeMap::new()),
            backend_api,
            change_callback: Mutex::new(None),
            path,
            state: RwLock::new(state),
        }))
    }

    pub(crate) fn status(&self) -> BlacklistStatus {
        let state = self.read_state();
        let now = crate::paths::current_unix_timestamp();
        let stale = state
            .fetched_at_unix
            .is_none_or(|then| now.saturating_sub(then) > STALE_AFTER.as_secs());
        BlacklistStatus {
            active_entry_count: state.entries.len(),
            api_available: state.last_error.is_none() && state.fetched_at_unix.is_some(),
            entries: state.entries.clone(),
            fetched_at_unix: state.fetched_at_unix,
            incidents: state.incidents.clone(),
            last_error: state.last_error.clone(),
            personal_repository_blocks: state.personal_repository_blocks.iter().cloned().collect(),
            stale,
        }
    }

    pub(crate) fn refresh_now(
        &self,
        store: Option<&SqliteRunnerStore>,
    ) -> Result<BlacklistStatus, BlacklistError> {
        let entries = match fetch_entries(&self.backend_api) {
            Ok(entries) => entries,
            Err(error) => {
                let mut state = self.write_state();
                state.last_error = Some(error.to_string());
                save_state(&self.path, &state)?;
                drop(state);
                self.notify_changed();
                return Err(error);
            }
        };
        {
            let mut state = self.write_state();
            state.entries = entries;
            state.fetched_at_unix = Some(crate::paths::current_unix_timestamp());
            state.last_error = None;
            let critical_scripts = if let Some(store) = store {
                apply_quarantine(&mut state, store)?
            } else {
                BTreeSet::new()
            };
            self.cancel_critical_runs(&critical_scripts);
            save_state(&self.path, &state)?;
        }
        self.notify_changed();
        Ok(self.status())
    }

    pub(crate) fn enforce_cached(&self, store: &SqliteRunnerStore) -> Result<(), BlacklistError> {
        let mut state = self.write_state();
        let critical_scripts = apply_quarantine(&mut state, store)?;
        self.cancel_critical_runs(&critical_scripts);
        save_state(&self.path, &state)
    }

    pub(crate) fn set_change_callback(&self, callback: Arc<dyn Fn() + Send + Sync>) {
        *self
            .change_callback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(callback);
    }

    pub(crate) fn start_background_refresh(self: &Arc<Self>, store: SqliteRunnerStore) {
        let service = Arc::clone(self);
        std::thread::Builder::new()
            .name("baudbound-blacklist".to_owned())
            .spawn(move || loop {
                if let Err(error) = service.refresh_now(Some(&store)) {
                    tracing::warn!(%error, "Official blacklist refresh failed; keeping the last valid cache");
                }
                std::thread::sleep(REFRESH_INTERVAL);
            })
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "failed to start the Official blacklist refresh worker");
                std::thread::spawn(|| {})
            });
    }

    pub(crate) fn record_provenance(
        &self,
        script_id: &str,
        provenance: TrustedProvenance,
    ) -> Result<(), BlacklistError> {
        let mut state = self.write_state();
        state.provenance.insert(script_id.to_owned(), provenance);
        save_state(&self.path, &state)
    }

    pub(crate) fn record_repository_provenance(
        &self,
        repository_url: &str,
        urls: Vec<String>,
    ) -> Result<(), BlacklistError> {
        let repository_url = normalize_repository_url(repository_url)
            .map_err(|_| BlacklistError::InvalidRepositoryUrl)?;
        let mut normalized_urls = urls
            .into_iter()
            .filter_map(|value| Url::parse(&value).ok())
            .filter(|url| url.scheme() == "https")
            .map(|url| url.to_string())
            .collect::<Vec<_>>();
        normalized_urls.sort();
        normalized_urls.dedup();
        let mut state = self.write_state();
        state
            .repository_provenance
            .insert(repository_url, normalized_urls);
        save_state(&self.path, &state)
    }

    pub(crate) fn remove_script_state(&self, script_id: &str) -> Result<(), BlacklistError> {
        let mut state = self.write_state();
        state.provenance.remove(script_id);
        state.quarantined_scripts.remove(script_id);
        save_state(&self.path, &state)
    }

    pub(crate) fn repository_decision(&self, repository_url: &str) -> BlacklistDecision {
        self.decide(&BlacklistMatchSubject::url(repository_url))
    }

    pub(crate) fn catalog_exclusions(
        &self,
        repository_urls: impl IntoIterator<Item = String>,
    ) -> CatalogExclusions {
        let state = self.read_state();
        let mut exclusions = CatalogExclusions::default();
        for url in repository_urls {
            let decision = BlacklistDecision::from_entries(matching_entries(
                &state.entries,
                &BlacklistMatchSubject::url(&url),
            ));
            if decision.blocks_distribution() || state.personal_repository_blocks.contains(&url) {
                exclusions.repository_urls.push(url);
            }
        }
        for entry in state
            .entries
            .iter()
            .filter(|entry| entry.severity.blocks_distribution())
        {
            match entry.scope {
                BlacklistScope::Script => exclusions.script_ids.push(entry.target.clone()),
                BlacklistScope::Package => exclusions.package_hashes.push(entry.target.clone()),
                _ => {}
            }
        }
        exclusions.repository_urls.sort();
        exclusions.repository_urls.dedup();
        exclusions.script_ids.sort();
        exclusions.script_ids.dedup();
        exclusions.package_hashes.sort();
        exclusions.package_hashes.dedup();
        exclusions
    }

    pub(crate) fn script_decision(&self, script_id: &str, package_hash: &str) -> BlacklistDecision {
        let state = self.read_state();
        let mut subject = BlacklistMatchSubject::installed(script_id, package_hash);
        if let Some(provenance) = state.provenance.get(script_id) {
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
        BlacklistDecision::from_entries(matching_entries(&state.entries, &subject))
    }

    pub(crate) fn ensure_url_distribution_allowed(
        &self,
        url: &Url,
    ) -> Result<BlacklistDecision, BlacklistError> {
        let decision = self.decide(&BlacklistMatchSubject::url(url.to_string()));
        if decision.blocks_distribution() {
            return Err(BlacklistError::Restricted {
                severity: decision
                    .severity
                    .expect("a restricted decision has a severity"),
                titles: decision
                    .entries
                    .iter()
                    .map(|entry| entry.title.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            });
        }
        Ok(decision)
    }

    pub(crate) fn is_personally_blocked(&self, repository_url: &str) -> bool {
        normalize_repository_url(repository_url)
            .is_ok_and(|url| self.read_state().personal_repository_blocks.contains(&url))
    }

    pub(crate) fn set_personal_repository_block(
        &self,
        repository_url: &str,
        blocked: bool,
    ) -> Result<BlacklistStatus, BlacklistError> {
        let normalized = normalize_repository_url(repository_url)
            .map_err(|_| BlacklistError::InvalidRepositoryUrl)?;
        let mut state = self.write_state();
        if blocked {
            state.personal_repository_blocks.insert(normalized);
        } else {
            state.personal_repository_blocks.remove(&normalized);
        }
        save_state(&self.path, &state)?;
        drop(state);
        self.notify_changed();
        Ok(self.status())
    }

    fn read_state(&self) -> std::sync::RwLockReadGuard<'_, PersistedState> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_state(&self) -> std::sync::RwLockWriteGuard<'_, PersistedState> {
        self.state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn cancel_critical_runs(&self, critical_scripts: &BTreeSet<String>) {
        let active_runs = self
            .active_runs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for run in active_runs.values() {
            if critical_scripts.contains(&run.script_id) {
                run.cancellation.cancel();
            }
        }
    }

    fn notify_changed(&self) {
        if let Some(callback) = self
            .change_callback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
        {
            callback();
        }
    }
}

impl BlacklistPolicy for BlacklistService {
    fn decide(&self, subject: &BlacklistMatchSubject) -> BlacklistDecision {
        let state = self.read_state();
        let mut expanded = subject.clone();
        if let Some(script_id) = &subject.script_id
            && let Some(provenance) = state.provenance.get(script_id)
        {
            if let Some(repository_url) = &provenance.repository_url {
                expanded.trusted_urls.push(repository_url.clone());
                if let Some(urls) = state.repository_provenance.get(repository_url) {
                    expanded.trusted_urls.extend(urls.iter().cloned());
                }
            }
            expanded
                .trusted_urls
                .extend(provenance.package_urls.iter().cloned());
            if let Some(final_url) = &provenance.final_package_url {
                expanded.trusted_urls.push(final_url.clone());
            }
        }
        BlacklistDecision::from_entries(matching_entries(&state.entries, &expanded))
    }
}

impl RuntimeRunObserver for BlacklistService {
    fn run_started(&self, identity: &RunIdentity, cancellation: RuntimeCancellationToken) {
        self.active_runs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                identity.run_id.clone(),
                ActiveRun {
                    cancellation,
                    script_id: identity.script_id.clone(),
                },
            );
    }

    fn log_emitted(&self, _identity: &RunIdentity, _entry: &RuntimeLogEntry) {}

    fn run_finished(&self, identity: &RunIdentity) {
        self.active_runs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&identity.run_id);
    }
}
