use baudbound_core::RunnerCore;
use baudbound_script::{
    RiskLevel, ScriptRepositoryEntry, load_script_package, parse_script_repository,
};
use baudbound_storage::{ScriptStore, ScriptUpdateState, SqliteRunnerStore};
use chrono::Utc;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Mutex, OnceLock},
};
use thiserror::Error;

use super::{RemoteFetchError, RemoteFetchService};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemotePackageOperation {
    Import,
    Update,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemotePackageSource {
    Package,
    Repository,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemotePreparationStage {
    DownloadingPackage,
    DownloadingRepository,
    VerifyingHash,
    ValidatingPackage,
    AwaitingReview,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RemotePreparationProgress {
    pub(crate) stage: RemotePreparationStage,
    pub(crate) transferred_bytes: u64,
    pub(crate) total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RemotePackageReview {
    pub(crate) capabilities: Vec<String>,
    pub(crate) current_version: Option<String>,
    pub(crate) operation: RemotePackageOperation,
    pub(crate) permissions: Vec<String>,
    pub(crate) risk_level: String,
    pub(crate) script_id: String,
    pub(crate) script_name: String,
    pub(crate) sha256: String,
    pub(crate) size: u64,
    pub(crate) source: RemotePackageSource,
    pub(crate) target_runtime: String,
    pub(crate) repository_url: String,
    pub(crate) version: String,
}

#[derive(Debug)]
pub(crate) struct PreparedRemotePackage {
    pub(crate) download: super::RemoteDownload,
    pub(crate) review: RemotePackageReview,
}

#[derive(Debug, Error)]
pub(crate) enum RemotePackagePrepareError {
    #[error(transparent)]
    Remote(#[from] RemoteFetchError),
    #[error("the script repository is invalid: {0}")]
    Repository(String),
    #[error("the downloaded package is invalid: {0}")]
    Package(String),
    #[error("the script repository does not match the downloaded package: {0}")]
    RepositoryPackageMismatch(String),
    #[error("script {0} is already installed")]
    AlreadyInstalled(String),
    #[error("script {0} is not installed")]
    NotInstalled(String),
    #[error("the package must have a newer version than the installed script")]
    VersionNotNewer,
    #[error("the package reuses version {0} with different package bytes")]
    ReusedVersion(String),
    #[error("failed to inspect installed scripts: {0}")]
    Storage(String),
}

pub(crate) fn prepare_remote_package_with_progress(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    package_limit: u64,
    operation: RemotePackageOperation,
    source: RemotePackageSource,
    url: &str,
    progress: &mut dyn FnMut(RemotePreparationProgress) -> bool,
) -> Result<PreparedRemotePackage, RemotePackagePrepareError> {
    if matches!(source, RemotePackageSource::Repository) {
        return Err(RemotePackagePrepareError::Repository(
            "add repository.json through Browse Scripts instead of the package Import or Update dialog"
                .to_owned(),
        ));
    }
    let fetcher = RemoteFetchService::new(package_limit);
    let download = fetcher.fetch_package_with_progress(url, &mut |transferred, total| {
        progress(RemotePreparationProgress {
            stage: RemotePreparationStage::DownloadingPackage,
            transferred_bytes: transferred,
            total_bytes: total,
        })
    })?;

    prepare_downloaded_package(core, store, operation, source, download, None, progress)
}

pub(crate) fn prepare_discovered_update_with_progress(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    package_limit: u64,
    script_reference: &str,
    progress: &mut dyn FnMut(RemotePreparationProgress) -> bool,
) -> Result<PreparedRemotePackage, RemotePackagePrepareError> {
    let installed = store
        .verify_script_package_hash(script_reference)
        .map_err(|error| RemotePackagePrepareError::Storage(error.to_string()))?;
    let installed_package = load_script_package(&installed.package_path)
        .map_err(|error| RemotePackagePrepareError::Package(error.to_string()))?;
    let state = store
        .script_update_state(&installed.id)
        .map_err(|error| RemotePackagePrepareError::Storage(error.to_string()))?;
    let checked_repository_url = state.checked_repository_url.as_deref().ok_or_else(|| {
        RemotePackagePrepareError::Repository(
            "check this script for updates before reviewing an update".to_owned(),
        )
    })?;
    if installed_package.manifest.repository_url != checked_repository_url {
        return Err(RemotePackagePrepareError::Repository(
            "the script repository URL changed after the last successful check".to_owned(),
        ));
    }
    let package_url = state.package_url.as_deref().ok_or_else(|| {
        RemotePackagePrepareError::Repository("no discovered update is available".to_owned())
    })?;
    let expected_sha256 = state.package_sha256.as_deref().ok_or_else(|| {
        RemotePackagePrepareError::Repository("the discovered package hash is missing".to_owned())
    })?;
    let expected_size = state.package_size.ok_or_else(|| {
        RemotePackagePrepareError::Repository("the discovered package size is missing".to_owned())
    })?;
    let expected_version = state.latest_version.as_deref().ok_or_else(|| {
        RemotePackagePrepareError::Repository(
            "the discovered package version is missing".to_owned(),
        )
    })?;
    if expected_size > package_limit {
        return Err(RemotePackagePrepareError::Repository(format!(
            "the declared package size exceeds the {package_limit} byte limit"
        )));
    }

    let fetcher = RemoteFetchService::new(package_limit);
    let repository_bytes = match fetcher.fetch_repository_with_progress(
        checked_repository_url,
        &mut |transferred, total| {
            progress(RemotePreparationProgress {
                stage: RemotePreparationStage::DownloadingRepository,
                transferred_bytes: transferred,
                total_bytes: total,
            })
        },
    )? {
        super::RepositoryFetchResult::Modified { bytes, .. } => bytes,
        super::RepositoryFetchResult::NotModified => {
            return Err(RemotePackagePrepareError::Repository(
                "the repository returned an unexpected unchanged response".to_owned(),
            ));
        }
    };
    let repository = parse_script_repository(&repository_bytes)
        .map_err(|error| RemotePackagePrepareError::Repository(error.to_string()))?;
    let repository_script = repository
        .script(&installed.id)
        .ok_or_else(|| {
            RemotePackagePrepareError::Repository(
                "the repository no longer contains this script".to_owned(),
            )
        })?
        .clone();
    if repository_script.latest.package_url != package_url
        || repository_script.latest.sha256 != expected_sha256
        || repository_script.latest.size != expected_size
        || repository_script.latest.version != expected_version
    {
        return Err(RemotePackagePrepareError::Repository(
            "the repository changed after the last successful update check; check again before reviewing the update"
                .to_owned(),
        ));
    }
    let download =
        fetcher.fetch_package_with_progress(package_url, &mut |transferred, total| {
            progress(RemotePreparationProgress {
                stage: RemotePreparationStage::DownloadingPackage,
                transferred_bytes: transferred,
                total_bytes: total,
            })
        })?;
    report_progress(
        progress,
        RemotePreparationStage::VerifyingHash,
        download.size,
        Some(download.size),
    )?;
    if download.size != expected_size || download.sha256 != expected_sha256 {
        return Err(RemotePackagePrepareError::RepositoryPackageMismatch(
            package_integrity_mismatch(
                expected_size,
                expected_sha256,
                download.size,
                &download.sha256,
            ),
        ));
    }
    let prepared = prepare_downloaded_package(
        core,
        store,
        RemotePackageOperation::Update,
        RemotePackageSource::Repository,
        download,
        Some(&repository_script),
        progress,
    )?;
    if prepared.review.script_id != installed.id || prepared.review.version != expected_version {
        let mut mismatches = Vec::new();
        if prepared.review.script_id != installed.id {
            mismatches.push(text_claim_mismatch(
                "script ID",
                &installed.id,
                &prepared.review.script_id,
            ));
        }
        if prepared.review.version != expected_version {
            mismatches.push(text_claim_mismatch(
                "version",
                expected_version,
                &prepared.review.version,
            ));
        }
        return Err(RemotePackagePrepareError::RepositoryPackageMismatch(
            mismatches.join(", "),
        ));
    }
    Ok(prepared)
}

pub(crate) fn prepare_repository_package_with_progress(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    package_limit: u64,
    operation: RemotePackageOperation,
    repository_url: &str,
    repository_script: &ScriptRepositoryEntry,
    progress: &mut dyn FnMut(RemotePreparationProgress) -> bool,
) -> Result<PreparedRemotePackage, RemotePackagePrepareError> {
    if repository_script.latest.size > package_limit {
        return Err(RemotePackagePrepareError::Repository(format!(
            "the declared package size exceeds the {package_limit} byte limit"
        )));
    }
    let fetcher = RemoteFetchService::new(package_limit);
    let download = fetcher.fetch_package_with_progress(
        &repository_script.latest.package_url,
        &mut |transferred, total| {
            progress(RemotePreparationProgress {
                stage: RemotePreparationStage::DownloadingPackage,
                transferred_bytes: transferred,
                total_bytes: total,
            })
        },
    )?;
    report_progress(
        progress,
        RemotePreparationStage::VerifyingHash,
        download.size,
        Some(download.size),
    )?;
    if download.size != repository_script.latest.size
        || download.sha256 != repository_script.latest.sha256
    {
        return Err(RemotePackagePrepareError::RepositoryPackageMismatch(
            package_integrity_mismatch(
                repository_script.latest.size,
                &repository_script.latest.sha256,
                download.size,
                &download.sha256,
            ),
        ));
    }
    let prepared = prepare_downloaded_package(
        core,
        store,
        operation,
        RemotePackageSource::Repository,
        download,
        Some(repository_script),
        progress,
    )?;
    if prepared.review.repository_url != repository_url {
        return Err(RemotePackagePrepareError::RepositoryPackageMismatch(
            "the package repository URL differs from the repository used to download it".to_owned(),
        ));
    }
    Ok(prepared)
}

fn prepare_downloaded_package(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    operation: RemotePackageOperation,
    source: RemotePackageSource,
    download: super::RemoteDownload,
    repository_script: Option<&ScriptRepositoryEntry>,
    progress: &mut dyn FnMut(RemotePreparationProgress) -> bool,
) -> Result<PreparedRemotePackage, RemotePackagePrepareError> {
    report_progress(
        progress,
        RemotePreparationStage::ValidatingPackage,
        download.size,
        Some(download.size),
    )?;
    core.validate_package(download.file.path())
        .map_err(|error| RemotePackagePrepareError::Package(error.to_string()))?;
    let package = load_script_package(download.file.path())
        .map_err(|error| RemotePackagePrepareError::Package(error.to_string()))?;
    if let Some(repository_script) = repository_script {
        let mismatches = repository_package_mismatches(repository_script, &package);
        if !mismatches.is_empty() {
            return Err(RemotePackagePrepareError::RepositoryPackageMismatch(
                mismatches.join(", "),
            ));
        }
    }
    let installed = store
        .list_scripts()
        .map_err(|error| RemotePackagePrepareError::Storage(error.to_string()))?
        .into_iter()
        .find(|script| script.id == package.manifest.id);
    let current_version = match operation {
        RemotePackageOperation::Import => {
            if installed.is_some() {
                return Err(RemotePackagePrepareError::AlreadyInstalled(
                    package.manifest.id,
                ));
            }
            None
        }
        RemotePackageOperation::Update => {
            let installed = installed.ok_or_else(|| {
                RemotePackagePrepareError::NotInstalled(package.manifest.id.clone())
            })?;
            let installed_package = load_script_package(&installed.package_path)
                .map_err(|error| RemotePackagePrepareError::Package(error.to_string()))?;
            let current = Version::parse(&installed_package.manifest.version)
                .map_err(|error| RemotePackagePrepareError::Package(error.to_string()))?;
            let next = Version::parse(&package.manifest.version)
                .map_err(|error| RemotePackagePrepareError::Package(error.to_string()))?;
            if next < current {
                return Err(RemotePackagePrepareError::VersionNotNewer);
            }
            if next == current {
                if download.sha256 != installed.package_hash {
                    return Err(RemotePackagePrepareError::ReusedVersion(
                        package.manifest.version,
                    ));
                }
                return Err(RemotePackagePrepareError::VersionNotNewer);
            }
            Some(installed_package.manifest.version)
        }
    };

    let review = RemotePackageReview {
        capabilities: package.capabilities.required_capabilities,
        current_version,
        operation,
        permissions: package.permissions.declared_permissions,
        risk_level: risk_level_name(&package.permissions.risk_level).to_owned(),
        script_id: package.manifest.id,
        script_name: package.manifest.name,
        sha256: download.sha256.clone(),
        size: download.size,
        source,
        target_runtime: package.capabilities.target_runtime,
        repository_url: package.manifest.repository_url,
        version: package.manifest.version,
    };
    report_progress(
        progress,
        RemotePreparationStage::AwaitingReview,
        download.size,
        Some(download.size),
    )?;
    Ok(PreparedRemotePackage { download, review })
}

fn report_progress(
    progress: &mut dyn FnMut(RemotePreparationProgress) -> bool,
    stage: RemotePreparationStage,
    transferred_bytes: u64,
    total_bytes: Option<u64>,
) -> Result<(), RemotePackagePrepareError> {
    if progress(RemotePreparationProgress {
        stage,
        transferred_bytes,
        total_bytes,
    }) {
        Ok(())
    } else {
        Err(RemoteFetchError::Cancelled.into())
    }
}

fn risk_level_name(risk: &RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
        RiskLevel::Dangerous => "dangerous",
    }
}

fn repository_package_mismatches(
    repository: &ScriptRepositoryEntry,
    package: &baudbound_script::ScriptPackage,
) -> Vec<String> {
    let mut mismatches = Vec::new();
    if repository.script_id != package.manifest.id {
        mismatches.push(text_claim_mismatch(
            "script ID",
            &repository.script_id,
            &package.manifest.id,
        ));
    }
    if repository.name != package.manifest.name {
        mismatches.push(text_claim_mismatch(
            "name",
            &repository.name,
            &package.manifest.name,
        ));
    }
    if repository.latest.version != package.manifest.version {
        mismatches.push(text_claim_mismatch(
            "version",
            &repository.latest.version,
            &package.manifest.version,
        ));
    }
    if repository.target_runtime != package.capabilities.target_runtime {
        mismatches.push(text_claim_mismatch(
            "target runtime",
            &repository.target_runtime,
            &package.capabilities.target_runtime,
        ));
    }
    if repository.minimum_runner_version != package.manifest.minimum_runner_version {
        mismatches.push(text_claim_mismatch(
            "minimum runner version",
            &repository.minimum_runner_version,
            &package.manifest.minimum_runner_version,
        ));
    }
    let package_risk = risk_level_name(&package.permissions.risk_level);
    if repository.risk_level != package_risk {
        mismatches.push(text_claim_mismatch(
            "risk level",
            &repository.risk_level,
            package_risk,
        ));
    }
    let repository_permissions = normalized_claims(&repository.permissions);
    let package_permissions = normalized_claims(&package.permissions.declared_permissions);
    if repository_permissions != package_permissions {
        mismatches.push(list_claim_mismatch(
            "permissions",
            &repository_permissions,
            &package_permissions,
        ));
    }
    let repository_capabilities = normalized_claims(&repository.capabilities);
    let package_capabilities = normalized_claims(&package.capabilities.required_capabilities);
    if repository_capabilities != package_capabilities {
        mismatches.push(list_claim_mismatch(
            "capabilities",
            &repository_capabilities,
            &package_capabilities,
        ));
    }
    mismatches
}

fn package_integrity_mismatch(
    repository_size: u64,
    repository_sha256: &str,
    package_size: u64,
    package_sha256: &str,
) -> String {
    let mut mismatches = Vec::new();
    if repository_size != package_size {
        mismatches.push(format!(
            "package size (repository {repository_size}, package {package_size})"
        ));
    }
    if repository_sha256 != package_sha256 {
        mismatches.push(text_claim_mismatch(
            "SHA-256",
            repository_sha256,
            package_sha256,
        ));
    }
    mismatches.join(", ")
}

fn text_claim_mismatch(field: &str, repository: &str, package: &str) -> String {
    format!(
        "{field} (repository {}, package {})",
        quoted_claim(repository),
        quoted_claim(package)
    )
}

fn list_claim_mismatch(
    field: &str,
    repository: &BTreeSet<&str>,
    package: &BTreeSet<&str>,
) -> String {
    let repository = repository.iter().copied().collect::<Vec<_>>();
    let package = package.iter().copied().collect::<Vec<_>>();
    format!(
        "{field} (repository {}, package {})",
        serde_json::to_string(&repository).unwrap_or_else(|_| "[]".to_owned()),
        serde_json::to_string(&package).unwrap_or_else(|_| "[]".to_owned())
    )
}

fn quoted_claim(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"invalid value\"".to_owned())
}

fn normalized_claims(values: &[String]) -> BTreeSet<&str> {
    values.iter().map(String::as_str).collect()
}

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
