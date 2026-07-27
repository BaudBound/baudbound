use baudbound_core::RunnerCore;
use baudbound_script::{
    RiskLevel, ScriptRepositoryEntry, load_script_package, parse_script_repository,
};
use baudbound_storage::{ScriptStore, SqliteRunnerStore};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
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
    pub(crate) release_notes: Option<String>,
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
    pub(crate) trusted_repository_url: Option<String>,
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

    prepare_downloaded_package(
        core,
        store,
        download,
        DownloadedPackageContext {
            operation,
            source,
            repository_script: None,
            trusted_repository_url: None,
        },
        progress,
    )
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
        super::RepositoryFetchResult::Modified(result) => {
            let super::RepositoryFetchModified {
                bytes,
                final_url,
                original_url,
                redirect_urls,
                ..
            } = *result;
            if let Some(blacklist) = crate::blacklist::global() {
                blacklist
                    .record_repository_provenance(
                        checked_repository_url,
                        std::iter::once(original_url.to_string())
                            .chain(redirect_urls.iter().map(ToString::to_string))
                            .chain(std::iter::once(final_url.to_string()))
                            .collect(),
                    )
                    .map_err(|error| RemotePackagePrepareError::Repository(error.to_string()))?;
            }
            bytes
        }
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
        download,
        DownloadedPackageContext {
            operation: RemotePackageOperation::Update,
            source: RemotePackageSource::Repository,
            repository_script: Some(&repository_script),
            trusted_repository_url: Some(checked_repository_url),
        },
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
        download,
        DownloadedPackageContext {
            operation,
            source: RemotePackageSource::Repository,
            repository_script: Some(repository_script),
            trusted_repository_url: Some(repository_url),
        },
        progress,
    )?;
    if prepared.review.repository_url != repository_url {
        return Err(RemotePackagePrepareError::RepositoryPackageMismatch(
            "the package repository URL differs from the repository used to download it".to_owned(),
        ));
    }
    Ok(prepared)
}

struct DownloadedPackageContext<'a> {
    operation: RemotePackageOperation,
    source: RemotePackageSource,
    repository_script: Option<&'a ScriptRepositoryEntry>,
    trusted_repository_url: Option<&'a str>,
}

fn prepare_downloaded_package(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    download: super::RemoteDownload,
    context: DownloadedPackageContext<'_>,
    progress: &mut dyn FnMut(RemotePreparationProgress) -> bool,
) -> Result<PreparedRemotePackage, RemotePackagePrepareError> {
    let DownloadedPackageContext {
        operation,
        source,
        repository_script,
        trusted_repository_url,
    } = context;
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
        release_notes: repository_script
            .map(|script| script.latest.release_notes.trim().to_owned())
            .filter(|notes| !notes.is_empty()),
        risk_level: risk_level_name(&package.permissions.risk_level).to_owned(),
        script_id: package.manifest.id,
        script_name: package.manifest.name,
        sha256: download.sha256.clone(),
        size: download.size,
        source,
        target_runtime: package.capabilities.target_runtimes.join(", "),
        repository_url: package.manifest.repository_url,
        version: package.manifest.version,
    };
    report_progress(
        progress,
        RemotePreparationStage::AwaitingReview,
        download.size,
        Some(download.size),
    )?;
    Ok(PreparedRemotePackage {
        download,
        review,
        trusted_repository_url: trusted_repository_url.map(ToOwned::to_owned),
    })
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
    if repository.target_runtimes != package.capabilities.target_runtimes {
        let repository_targets = repository.target_runtimes.join(", ");
        let package_targets = package.capabilities.target_runtimes.join(", ");
        mismatches.push(text_claim_mismatch(
            "target runtimes",
            &repository_targets,
            &package_targets,
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
