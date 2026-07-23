use baudbound_core::RunnerCore;
use baudbound_script::{
    RiskLevel, ScriptUpdateDescriptor, load_script_package, parse_script_update_descriptor,
};
use baudbound_storage::{ScriptStore, ScriptUpdateState, SqliteRunnerStore};
use chrono::Utc;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
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
    Descriptor,
    Package,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemotePreparationStage {
    DownloadingDescriptor,
    DownloadingPackage,
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
    pub(crate) update_url: String,
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
    #[error("the update descriptor is invalid: {0}")]
    Descriptor(String),
    #[error("the downloaded package is invalid: {0}")]
    Package(String),
    #[error("the update descriptor does not match the downloaded package")]
    DescriptorPackageMismatch,
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
    if matches!(operation, RemotePackageOperation::Update)
        && matches!(source, RemotePackageSource::Descriptor)
    {
        return Err(RemotePackagePrepareError::Descriptor(
            "the explicit Update action accepts direct .bbs package URLs only".to_owned(),
        ));
    }
    let fetcher = RemoteFetchService::new(package_limit);
    let (descriptor, download) = match source {
        RemotePackageSource::Package => (
            None,
            fetcher.fetch_package_with_progress(url, &mut |transferred, total| {
                progress(RemotePreparationProgress {
                    stage: RemotePreparationStage::DownloadingPackage,
                    transferred_bytes: transferred,
                    total_bytes: total,
                })
            })?,
        ),
        RemotePackageSource::Descriptor => {
            let (bytes, _) =
                fetcher.fetch_descriptor_with_progress(url, &mut |transferred, total| {
                    progress(RemotePreparationProgress {
                        stage: RemotePreparationStage::DownloadingDescriptor,
                        transferred_bytes: transferred,
                        total_bytes: total,
                    })
                })?;
            let descriptor = parse_script_update_descriptor(&bytes)
                .map_err(|error| RemotePackagePrepareError::Descriptor(error.to_string()))?;
            if descriptor.latest.size > package_limit {
                return Err(RemotePackagePrepareError::Descriptor(format!(
                    "the declared package size exceeds the {package_limit} byte limit"
                )));
            }
            let download = fetcher.fetch_package_with_progress(
                &descriptor.latest.package_url,
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
            verify_descriptor_download(&descriptor, &download)?;
            (Some(descriptor), download)
        }
    };

    prepare_downloaded_package(
        core,
        store,
        operation,
        source,
        download,
        descriptor.as_ref(),
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
    let checked_update_url = state.checked_update_url.as_deref().ok_or_else(|| {
        RemotePackagePrepareError::Descriptor(
            "check this script for updates before reviewing an update".to_owned(),
        )
    })?;
    if installed_package.manifest.update_url != checked_update_url {
        return Err(RemotePackagePrepareError::Descriptor(
            "the script update URL changed after the last successful check".to_owned(),
        ));
    }
    let package_url = state.package_url.as_deref().ok_or_else(|| {
        RemotePackagePrepareError::Descriptor("no discovered update is available".to_owned())
    })?;
    let expected_sha256 = state.package_sha256.as_deref().ok_or_else(|| {
        RemotePackagePrepareError::Descriptor("the discovered package hash is missing".to_owned())
    })?;
    let expected_size = state.package_size.ok_or_else(|| {
        RemotePackagePrepareError::Descriptor("the discovered package size is missing".to_owned())
    })?;
    let expected_version = state.latest_version.as_deref().ok_or_else(|| {
        RemotePackagePrepareError::Descriptor(
            "the discovered package version is missing".to_owned(),
        )
    })?;
    if expected_size > package_limit {
        return Err(RemotePackagePrepareError::Descriptor(format!(
            "the declared package size exceeds the {package_limit} byte limit"
        )));
    }

    let fetcher = RemoteFetchService::new(package_limit);
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
        return Err(RemotePackagePrepareError::DescriptorPackageMismatch);
    }
    let prepared = prepare_downloaded_package(
        core,
        store,
        RemotePackageOperation::Update,
        RemotePackageSource::Descriptor,
        download,
        None,
        progress,
    )?;
    if prepared.review.script_id != installed.id || prepared.review.version != expected_version {
        return Err(RemotePackagePrepareError::DescriptorPackageMismatch);
    }
    Ok(prepared)
}

fn prepare_downloaded_package(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    operation: RemotePackageOperation,
    source: RemotePackageSource,
    download: super::RemoteDownload,
    descriptor: Option<&ScriptUpdateDescriptor>,
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
    if descriptor.is_some_and(|descriptor| {
        descriptor.script_id != package.manifest.id
            || descriptor.latest.version != package.manifest.version
    }) {
        return Err(RemotePackagePrepareError::DescriptorPackageMismatch);
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
        update_url: package.manifest.update_url,
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

fn verify_descriptor_download(
    descriptor: &ScriptUpdateDescriptor,
    download: &super::RemoteDownload,
) -> Result<(), RemotePackagePrepareError> {
    if descriptor.latest.size != download.size || descriptor.latest.sha256 != download.sha256 {
        return Err(RemotePackagePrepareError::DescriptorPackageMismatch);
    }
    Ok(())
}

fn risk_level_name(risk: &RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
        RiskLevel::Dangerous => "dangerous",
    }
}

#[derive(Debug, Error)]
pub(crate) enum ScriptUpdateCheckError {
    #[error("the installed package cannot be inspected: {0}")]
    InstalledPackage(String),
    #[error("this script does not provide an update URL")]
    MissingUpdateUrl,
    #[error("this script is already being checked for updates")]
    AlreadyChecking,
    #[error(transparent)]
    Remote(#[from] RemoteFetchError),
    #[error("the update descriptor is invalid: {0}")]
    Descriptor(String),
    #[error("the update descriptor belongs to a different script")]
    ScriptIdentityMismatch,
    #[error("the update descriptor package exceeds the configured {limit} byte limit")]
    PackageTooLarge { limit: u64 },
    #[error("the update descriptor reuses version {version} with different package bytes")]
    ReusedVersion { version: String },
    #[error("the update descriptor points to older version {available}")]
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
    let update_url = package.manifest.update_url.trim().to_owned();
    if update_url.is_empty() {
        return Err(ScriptUpdateCheckError::MissingUpdateUrl);
    }
    let checked_at_unix = Utc::now().timestamp().max(0) as u64;
    let result = check_descriptor(
        store,
        package_limit,
        &installed.id,
        &installed.package_hash,
        &package.manifest.version,
        &update_url,
        checked_at_unix,
    );
    if let Err(error) = &result {
        store
            .record_script_update_failure(
                &installed.id,
                &update_url,
                checked_at_unix,
                &error.to_string(),
            )
            .map_err(|storage| ScriptUpdateCheckError::Storage(storage.to_string()))?;
    }
    result
}

pub(crate) fn reconcile_script_update_state_after_install(
    store: &SqliteRunnerStore,
    script_reference: &str,
    has_update_url: bool,
) -> Result<(), ScriptUpdateCheckError> {
    let state = store
        .script_update_state(script_reference)
        .map_err(|error| ScriptUpdateCheckError::Storage(error.to_string()))?;
    if !has_update_url && state.automatic_checks_enabled {
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

fn check_descriptor(
    store: &SqliteRunnerStore,
    package_limit: u64,
    script_id: &str,
    installed_hash: &str,
    installed_version: &str,
    update_url: &str,
    checked_at_unix: u64,
) -> Result<ScriptUpdateState, ScriptUpdateCheckError> {
    let fetcher = RemoteFetchService::new(package_limit);
    let (bytes, _) = fetcher.fetch_descriptor(update_url)?;
    let descriptor = parse_script_update_descriptor(&bytes)
        .map_err(|error| ScriptUpdateCheckError::Descriptor(error.to_string()))?;
    if descriptor.script_id != script_id {
        return Err(ScriptUpdateCheckError::ScriptIdentityMismatch);
    }
    if descriptor.latest.size > package_limit {
        return Err(ScriptUpdateCheckError::PackageTooLarge {
            limit: package_limit,
        });
    }

    let current = Version::parse(installed_version)
        .map_err(|error| ScriptUpdateCheckError::InstalledPackage(error.to_string()))?;
    let available = Version::parse(&descriptor.latest.version)
        .map_err(|error| ScriptUpdateCheckError::Descriptor(error.to_string()))?;
    if available < current {
        return Err(ScriptUpdateCheckError::Downgrade {
            available: descriptor.latest.version,
        });
    }
    if available == current && descriptor.latest.sha256 != installed_hash {
        return Err(ScriptUpdateCheckError::ReusedVersion {
            version: descriptor.latest.version,
        });
    }

    let existing = store
        .script_update_state(script_id)
        .map_err(|error| ScriptUpdateCheckError::Storage(error.to_string()))?;
    let state = ScriptUpdateState {
        automatic_checks_enabled: existing.automatic_checks_enabled,
        checked_update_url: Some(update_url.to_owned()),
        last_checked_at_unix: Some(checked_at_unix),
        last_error: None,
        last_success_at_unix: Some(checked_at_unix),
        latest_version: Some(descriptor.latest.version),
        package_sha256: Some(descriptor.latest.sha256),
        package_size: Some(descriptor.latest.size),
        package_url: Some(descriptor.latest.package_url),
        published_at: Some(descriptor.latest.published_at),
        release_notes: Some(descriptor.latest.release_notes),
        script_id: script_id.to_owned(),
    };
    store
        .record_script_update_success(&state)
        .map_err(|error| ScriptUpdateCheckError::Storage(error.to_string()))?;
    Ok(state)
}
