use baudbound_script::{load_script_package, parse_script_repository};
use baudbound_storage::{ScriptStore, ScriptUpdateState, SqliteRunnerStore};
use chrono::Utc;
use semver::Version;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Mutex, OnceLock},
};
use thiserror::Error;

use super::{RemoteFetchError, RemoteFetchService};

#[derive(Debug, Error)]
pub(crate) enum ScriptUpdateCheckError {
    #[error("the installed package cannot be inspected: {0}")]
    InstalledPackage(String),
    #[error("this script does not provide a repository URL")]
    MissingRepositoryUrl,
    #[error("this script is already being checked for updates")]
    AlreadyChecking,
    #[error(transparent)]
    Remote(#[from] RemoteFetchError),
    #[error("the script repository is invalid: {0}")]
    Repository(String),
    #[error("the script repository no longer contains this script")]
    ScriptMissing,
    #[error("the repository package exceeds the configured {limit} byte limit")]
    PackageTooLarge { limit: u64 },
    #[error("the repository reuses version {version} with different package bytes")]
    ReusedVersion { version: String },
    #[error("the repository points to older version {available}")]
    Downgrade { available: String },
    #[error("failed to store the update check result: {0}")]
    Storage(String),
}

pub(crate) fn check_script_update(
    store: &SqliteRunnerStore,
    package_limit: u64,
    script_reference: &str,
) -> Result<ScriptUpdateState, ScriptUpdateCheckError> {
    let installed = store
        .verify_script_package_hash(script_reference)
        .map_err(|error| ScriptUpdateCheckError::InstalledPackage(error.to_string()))?;
    let package = load_script_package(&installed.package_path)
        .map_err(|error| ScriptUpdateCheckError::InstalledPackage(error.to_string()))?;
    let _check_guard = ScriptCheckGuard::acquire(&installed.id)?;
    let repository_url = package.manifest.repository_url.trim().to_owned();
    if repository_url.is_empty() {
        return Err(ScriptUpdateCheckError::MissingRepositoryUrl);
    }
    let checked_at_unix = Utc::now().timestamp().max(0) as u64;
    let result = check_repository(
        store,
        package_limit,
        &installed.id,
        &installed.package_hash,
        &package.manifest.version,
        &repository_url,
        checked_at_unix,
    );
    if let Err(error) = &result {
        store
            .record_script_update_failure(
                &installed.id,
                &repository_url,
                checked_at_unix,
                &error.to_string(),
            )
            .map_err(|storage| ScriptUpdateCheckError::Storage(storage.to_string()))?;
    }
    result
}

pub(crate) fn check_script_updates(
    store: &SqliteRunnerStore,
    package_limit: u64,
    script_references: &[String],
) -> BTreeMap<String, Result<ScriptUpdateState, String>> {
    let mut results = BTreeMap::new();
    let mut repositories = BTreeMap::<String, Vec<InstalledUpdateCandidate>>::new();

    for reference in script_references {
        match installed_update_candidate(store, reference) {
            Ok(candidate) => repositories
                .entry(candidate.repository_url.clone())
                .or_default()
                .push(candidate),
            Err(error) => {
                results.insert(reference.clone(), Err(error.to_string()));
            }
        }
    }

    let fetcher = RemoteFetchService::new(package_limit);
    for (repository_url, candidates) in repositories {
        let repository_result = fetcher
            .fetch_repository(&repository_url)
            .map_err(ScriptUpdateCheckError::from)
            .and_then(|(bytes, _)| {
                parse_script_repository(&bytes)
                    .map_err(|error| ScriptUpdateCheckError::Repository(error.to_string()))
            });
        let checked_at_unix = Utc::now().timestamp().max(0) as u64;

        match repository_result {
            Ok(repository) => {
                for candidate in candidates {
                    let result = check_repository_entry(
                        store,
                        package_limit,
                        &candidate.id,
                        &candidate.package_hash,
                        &candidate.version,
                        &repository_url,
                        &repository,
                        checked_at_unix,
                    );
                    results.insert(
                        candidate.id.clone(),
                        record_check_result(
                            store,
                            &candidate.id,
                            &repository_url,
                            checked_at_unix,
                            result,
                        ),
                    );
                }
            }
            Err(error) => {
                let message = error.to_string();
                for candidate in candidates {
                    let result = record_script_update_failure(
                        store,
                        &candidate.id,
                        &repository_url,
                        checked_at_unix,
                        &message,
                    )
                    .map(|_| Err(message.clone()))
                    .unwrap_or_else(|storage| Err(storage.to_string()));
                    results.insert(candidate.id, result);
                }
            }
        }
    }

    results
}

pub(crate) fn reconcile_script_update_state_after_install(
    store: &SqliteRunnerStore,
    script_reference: &str,
    has_repository_url: bool,
) -> Result<(), ScriptUpdateCheckError> {
    let state = store
        .script_update_state(script_reference)
        .map_err(|error| ScriptUpdateCheckError::Storage(error.to_string()))?;
    if !has_repository_url && state.automatic_checks_enabled {
        store
            .set_script_automatic_update_checks(script_reference, false)
            .map_err(|error| ScriptUpdateCheckError::Storage(error.to_string()))?;
    }
    store
        .clear_script_update_discovery(script_reference)
        .map_err(|error| ScriptUpdateCheckError::Storage(error.to_string()))
}

static ACTIVE_SCRIPT_CHECKS: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();

struct ScriptCheckGuard {
    script_id: String,
}

impl ScriptCheckGuard {
    fn acquire(script_id: &str) -> Result<Self, ScriptUpdateCheckError> {
        let checks = ACTIVE_SCRIPT_CHECKS.get_or_init(|| Mutex::new(BTreeSet::new()));
        let mut checks = checks.lock().unwrap_or_else(|error| error.into_inner());
        if !checks.insert(script_id.to_owned()) {
            return Err(ScriptUpdateCheckError::AlreadyChecking);
        }
        Ok(Self {
            script_id: script_id.to_owned(),
        })
    }
}

impl Drop for ScriptCheckGuard {
    fn drop(&mut self) {
        let Some(checks) = ACTIVE_SCRIPT_CHECKS.get() else {
            return;
        };
        checks
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&self.script_id);
    }
}

fn check_repository(
    store: &SqliteRunnerStore,
    package_limit: u64,
    script_id: &str,
    installed_hash: &str,
    installed_version: &str,
    repository_url: &str,
    checked_at_unix: u64,
) -> Result<ScriptUpdateState, ScriptUpdateCheckError> {
    let fetcher = RemoteFetchService::new(package_limit);
    let (bytes, _) = fetcher.fetch_repository(repository_url)?;
    let repository = parse_script_repository(&bytes)
        .map_err(|error| ScriptUpdateCheckError::Repository(error.to_string()))?;
    check_repository_entry(
        store,
        package_limit,
        script_id,
        installed_hash,
        installed_version,
        repository_url,
        &repository,
        checked_at_unix,
    )
}

#[allow(clippy::too_many_arguments)]
fn check_repository_entry(
    store: &SqliteRunnerStore,
    package_limit: u64,
    script_id: &str,
    installed_hash: &str,
    installed_version: &str,
    repository_url: &str,
    repository: &baudbound_script::ScriptRepository,
    checked_at_unix: u64,
) -> Result<ScriptUpdateState, ScriptUpdateCheckError> {
    let script = repository
        .script(script_id)
        .ok_or(ScriptUpdateCheckError::ScriptMissing)?;
    if script.latest.size > package_limit {
        return Err(ScriptUpdateCheckError::PackageTooLarge {
            limit: package_limit,
        });
    }

    let current = Version::parse(installed_version)
        .map_err(|error| ScriptUpdateCheckError::InstalledPackage(error.to_string()))?;
    let available = Version::parse(&script.latest.version)
        .map_err(|error| ScriptUpdateCheckError::Repository(error.to_string()))?;
    if available < current {
        return Err(ScriptUpdateCheckError::Downgrade {
            available: script.latest.version.clone(),
        });
    }
    if available == current && script.latest.sha256 != installed_hash {
        return Err(ScriptUpdateCheckError::ReusedVersion {
            version: script.latest.version.clone(),
        });
    }

    let existing = store
        .script_update_state(script_id)
        .map_err(|error| ScriptUpdateCheckError::Storage(error.to_string()))?;
    let state = ScriptUpdateState {
        automatic_checks_enabled: existing.automatic_checks_enabled,
        checked_repository_url: Some(repository_url.to_owned()),
        last_checked_at_unix: Some(checked_at_unix),
        last_error: None,
        last_success_at_unix: Some(checked_at_unix),
        latest_version: Some(script.latest.version.clone()),
        package_sha256: Some(script.latest.sha256.clone()),
        package_size: Some(script.latest.size),
        package_url: Some(script.latest.package_url.clone()),
        published_at: Some(script.latest.published_at.clone()),
        release_notes: Some(script.latest.release_notes.clone()),
        script_id: script_id.to_owned(),
    };
    store
        .record_script_update_success(&state)
        .map_err(|error| ScriptUpdateCheckError::Storage(error.to_string()))?;
    Ok(state)
}

struct InstalledUpdateCandidate {
    _check_guard: ScriptCheckGuard,
    id: String,
    package_hash: String,
    repository_url: String,
    version: String,
}

fn installed_update_candidate(
    store: &SqliteRunnerStore,
    script_reference: &str,
) -> Result<InstalledUpdateCandidate, ScriptUpdateCheckError> {
    let installed = store
        .verify_script_package_hash(script_reference)
        .map_err(|error| ScriptUpdateCheckError::InstalledPackage(error.to_string()))?;
    let package = load_script_package(&installed.package_path)
        .map_err(|error| ScriptUpdateCheckError::InstalledPackage(error.to_string()))?;
    let check_guard = ScriptCheckGuard::acquire(&installed.id)?;
    let repository_url = package.manifest.repository_url.trim().to_owned();
    if repository_url.is_empty() {
        return Err(ScriptUpdateCheckError::MissingRepositoryUrl);
    }
    Ok(InstalledUpdateCandidate {
        _check_guard: check_guard,
        id: installed.id,
        package_hash: installed.package_hash,
        repository_url,
        version: package.manifest.version,
    })
}

fn record_check_result(
    store: &SqliteRunnerStore,
    script_id: &str,
    repository_url: &str,
    checked_at_unix: u64,
    result: Result<ScriptUpdateState, ScriptUpdateCheckError>,
) -> Result<ScriptUpdateState, String> {
    match result {
        Ok(state) => Ok(state),
        Err(error) => {
            record_script_update_failure(
                store,
                script_id,
                repository_url,
                checked_at_unix,
                &error.to_string(),
            )
            .map_err(|storage| storage.to_string())?;
            Err(error.to_string())
        }
    }
}

fn record_script_update_failure(
    store: &SqliteRunnerStore,
    script_id: &str,
    repository_url: &str,
    checked_at_unix: u64,
    message: &str,
) -> Result<(), ScriptUpdateCheckError> {
    store
        .record_script_update_failure(script_id, repository_url, checked_at_unix, message)
        .map_err(|error| ScriptUpdateCheckError::Storage(error.to_string()))
}
