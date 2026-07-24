use baudbound_core::RunnerCore;
use baudbound_script::{
    ScriptRepository, ScriptRepositoryEntry, parse_script_repository, repository_capability_names,
    repository_permission_names, validate_public_https_repository_url,
};
use baudbound_storage::{
    PaginatedRecords, RepositoryCacheEntry, RepositoryCacheReplacement,
    RepositoryScriptFilterOptions, RepositoryScriptQuery, RepositoryScriptRecord,
    RepositoryScriptSummary, RepositorySource, SqliteRunnerStore,
};
use chrono::Utc;
use serde::Serialize;
use std::sync::Mutex;
use thiserror::Error;

use crate::script_updates::{
    PreparedRemotePackage, RemoteFetchService, RemotePackageOperation, RemotePackagePrepareError,
    RemotePreparationProgress, RepositoryFetchResult, prepare_repository_package_with_progress,
};

pub(crate) const OFFICIAL_REPOSITORY_URL: &str =
    "https://raw.githubusercontent.com/BaudBound/repository/master/repository.json";
const LEGACY_OFFICIAL_REPOSITORY_URL: &str =
    "https://raw.githubusercontent.com/BaudBound/repository/main/repository.json";
static REPOSITORY_CACHE_MUTATION_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Error)]
pub(crate) enum ScriptRepositoryServiceError {
    #[error("the repository URL is invalid: {0}")]
    Url(String),
    #[error("the repository could not be downloaded: {0}")]
    Download(String),
    #[error("the repository is invalid: {0}")]
    Repository(String),
    #[error("repository storage failed: {0}")]
    Storage(String),
    #[error("the repository script was not found")]
    ScriptNotFound,
    #[error(transparent)]
    Package(#[from] RemotePackagePrepareError),
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RepositoryRefreshStage {
    Downloading,
    Validating,
    ReplacingCache,
    Complete,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RepositoryRefreshProgress {
    pub(crate) repository_url: String,
    pub(crate) stage: RepositoryRefreshStage,
    pub(crate) transferred_bytes: u64,
    pub(crate) total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RepositoryPreview {
    pub(crate) description: String,
    pub(crate) homepage: String,
    pub(crate) name: String,
    pub(crate) script_count: usize,
    pub(crate) url: String,
}

pub(crate) fn ensure_official_repository(
    store: &SqliteRunnerStore,
) -> Result<RepositorySource, ScriptRepositoryServiceError> {
    store
        .migrate_repository_source_url(
            LEGACY_OFFICIAL_REPOSITORY_URL,
            OFFICIAL_REPOSITORY_URL,
            true,
        )
        .map_err(storage_error)?;
    store
        .ensure_repository_source(OFFICIAL_REPOSITORY_URL, true)
        .map_err(storage_error)
}

pub(crate) fn add_repository(
    store: &SqliteRunnerStore,
    package_limit: u64,
    url: &str,
    progress: &mut dyn FnMut(RepositoryRefreshProgress) -> bool,
) -> Result<RepositorySource, ScriptRepositoryServiceError> {
    let normalized = normalize_repository_url(url)?;
    let _mutation_guard = REPOSITORY_CACHE_MUTATION_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if store
        .repository_source(&normalized)
        .map_err(storage_error)?
        .is_some()
    {
        return Err(ScriptRepositoryServiceError::Storage(
            "this repository has already been added".to_owned(),
        ));
    }

    let repository = download_repository(package_limit, &normalized, None, None, progress)?;
    store
        .ensure_repository_source(&normalized, false)
        .map_err(storage_error)?;
    match repository {
        DownloadedRepository::Modified {
            etag,
            last_modified,
            repository,
        } => replace_repository(
            store,
            &normalized,
            repository,
            etag,
            last_modified,
            progress,
        )
        .inspect_err(|_| {
            let _ = store.remove_repository_source(&normalized);
        }),
        DownloadedRepository::NotModified => {
            let _ = store.remove_repository_source(&normalized);
            Err(ScriptRepositoryServiceError::Download(
                "a new repository unexpectedly returned an unchanged response".to_owned(),
            ))
        }
    }
}

pub(crate) fn preview_repository(
    package_limit: u64,
    url: &str,
    progress: &mut dyn FnMut(RepositoryRefreshProgress) -> bool,
) -> Result<RepositoryPreview, ScriptRepositoryServiceError> {
    let normalized = normalize_repository_url(url)?;
    let repository = download_repository(package_limit, &normalized, None, None, progress)?;
    let DownloadedRepository::Modified { repository, .. } = repository else {
        return Err(ScriptRepositoryServiceError::Download(
            "the repository unexpectedly returned an unchanged response".to_owned(),
        ));
    };
    Ok(RepositoryPreview {
        description: repository.description,
        homepage: repository.homepage,
        name: repository.name,
        script_count: repository.scripts.len(),
        url: normalized,
    })
}

pub(crate) fn refresh_repository(
    store: &SqliteRunnerStore,
    package_limit: u64,
    url: &str,
    progress: &mut dyn FnMut(RepositoryRefreshProgress) -> bool,
) -> Result<RepositorySource, ScriptRepositoryServiceError> {
    let normalized = normalize_repository_url(url)?;
    let _mutation_guard = REPOSITORY_CACHE_MUTATION_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let source = store
        .repository_source(&normalized)
        .map_err(storage_error)?
        .ok_or_else(|| {
            ScriptRepositoryServiceError::Storage("repository source was not found".to_owned())
        })?;

    let result = download_repository(
        package_limit,
        &normalized,
        source.etag.as_deref(),
        source.last_modified.as_deref(),
        progress,
    )
    .and_then(|download| match download {
        DownloadedRepository::Modified {
            etag,
            last_modified,
            repository,
        } => replace_repository(
            store,
            &normalized,
            repository,
            etag,
            last_modified,
            progress,
        ),
        DownloadedRepository::NotModified => store
            .record_repository_not_modified(&normalized, current_unix_timestamp())
            .map_err(storage_error),
    });
    if let Err(error) = &result {
        let _ = store.record_repository_refresh_failure(
            &normalized,
            current_unix_timestamp(),
            &error.to_string(),
        );
    }
    result
}

pub(crate) fn prepare_repository_script_with_progress(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    package_limit: u64,
    repository_url: &str,
    script_id: &str,
    progress: &mut dyn FnMut(RemotePreparationProgress) -> bool,
) -> Result<PreparedRemotePackage, ScriptRepositoryServiceError> {
    let record = store
        .repository_script(repository_url, script_id)
        .map_err(storage_error)?
        .ok_or(ScriptRepositoryServiceError::ScriptNotFound)?;
    if let Some(message) = record
        .information_mismatch
        .as_ref()
        .filter(|_| record.information_mismatch_refresh_required)
    {
        return Err(ScriptRepositoryServiceError::Repository(format!(
            "this repository entry is blocked because its information did not match the downloaded package: {message}. Refresh the repository before trying again"
        )));
    }
    let entry: ScriptRepositoryEntry = serde_json::from_value(record.entry)
        .map_err(|error| ScriptRepositoryServiceError::Repository(error.to_string()))?;
    let operation = if record.installed {
        RemotePackageOperation::Update
    } else {
        RemotePackageOperation::Import
    };
    let result = prepare_repository_package_with_progress(
        core,
        store,
        package_limit,
        operation,
        repository_url,
        &entry,
        progress,
    );
    if let Err(RemotePackagePrepareError::RepositoryPackageMismatch(message)) = &result {
        store
            .record_repository_information_mismatch(repository_url, script_id, message)
            .map_err(storage_error)?;
    } else if result.is_ok() && record.information_mismatch.is_some() {
        store
            .clear_repository_information_mismatch(repository_url, script_id)
            .map_err(storage_error)?;
    }
    result.map_err(Into::into)
}

pub(crate) fn list_repositories(
    store: &SqliteRunnerStore,
) -> Result<Vec<RepositorySource>, ScriptRepositoryServiceError> {
    store.list_repository_sources().map_err(storage_error)
}

pub(crate) fn query_scripts(
    store: &SqliteRunnerStore,
    query: &RepositoryScriptQuery,
) -> Result<PaginatedRecords<RepositoryScriptSummary>, ScriptRepositoryServiceError> {
    store.query_repository_scripts(query).map_err(storage_error)
}

pub(crate) fn script_filter_options() -> RepositoryScriptFilterOptions {
    RepositoryScriptFilterOptions {
        capabilities: repository_capability_names()
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        permissions: repository_permission_names()
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    }
}

pub(crate) fn script_details(
    store: &SqliteRunnerStore,
    repository_url: &str,
    script_id: &str,
) -> Result<RepositoryScriptRecord, ScriptRepositoryServiceError> {
    store
        .repository_script(repository_url, script_id)
        .map_err(storage_error)?
        .ok_or(ScriptRepositoryServiceError::ScriptNotFound)
}

fn download_repository(
    package_limit: u64,
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
    progress: &mut dyn FnMut(RepositoryRefreshProgress) -> bool,
) -> Result<DownloadedRepository, ScriptRepositoryServiceError> {
    let fetcher = RemoteFetchService::new(package_limit);
    let result = fetcher
        .fetch_repository_conditional(url, etag, last_modified, &mut |transferred, total| {
            progress(RepositoryRefreshProgress {
                repository_url: url.to_owned(),
                stage: RepositoryRefreshStage::Downloading,
                transferred_bytes: transferred,
                total_bytes: total,
            })
        })
        .map_err(|error| ScriptRepositoryServiceError::Download(error.to_string()))?;
    let RepositoryFetchResult::Modified {
        bytes,
        etag,
        last_modified,
        ..
    } = result
    else {
        return Ok(DownloadedRepository::NotModified);
    };
    if !progress(RepositoryRefreshProgress {
        repository_url: url.to_owned(),
        stage: RepositoryRefreshStage::Validating,
        transferred_bytes: bytes.len() as u64,
        total_bytes: Some(bytes.len() as u64),
    }) {
        return Err(ScriptRepositoryServiceError::Download(
            "repository refresh was cancelled".to_owned(),
        ));
    }
    let repository = parse_script_repository(&bytes)
        .map_err(|error| ScriptRepositoryServiceError::Repository(error.to_string()))?;
    Ok(DownloadedRepository::Modified {
        etag,
        last_modified,
        repository,
    })
}

fn replace_repository(
    store: &SqliteRunnerStore,
    url: &str,
    repository: ScriptRepository,
    etag: Option<String>,
    last_modified: Option<String>,
    progress: &mut dyn FnMut(RepositoryRefreshProgress) -> bool,
) -> Result<RepositorySource, ScriptRepositoryServiceError> {
    if !progress(RepositoryRefreshProgress {
        repository_url: url.to_owned(),
        stage: RepositoryRefreshStage::ReplacingCache,
        transferred_bytes: 0,
        total_bytes: None,
    }) {
        return Err(ScriptRepositoryServiceError::Download(
            "repository refresh was cancelled".to_owned(),
        ));
    }
    let entries = repository
        .scripts
        .iter()
        .map(|entry| {
            Ok(RepositoryCacheEntry {
                author: entry.author.clone(),
                entry_json: serde_json::to_string(entry)
                    .map_err(|error| ScriptRepositoryServiceError::Repository(error.to_string()))?,
                name: entry.name.clone(),
                published_at: entry.latest.published_at.clone(),
                risk_level: entry.risk_level.clone(),
                script_id: entry.script_id.clone(),
                summary: entry.summary.clone(),
                target_runtime: entry.target_runtimes.join(", "),
                version: entry.latest.version.clone(),
            })
        })
        .collect::<Result<Vec<_>, ScriptRepositoryServiceError>>()?;
    let source = store
        .replace_repository_cache(&RepositoryCacheReplacement {
            description: repository.description,
            etag,
            entries,
            homepage: repository.homepage,
            last_modified,
            name: repository.name,
            refreshed_at_unix: current_unix_timestamp(),
            url: url.to_owned(),
        })
        .map_err(storage_error)?;
    progress(RepositoryRefreshProgress {
        repository_url: url.to_owned(),
        stage: RepositoryRefreshStage::Complete,
        transferred_bytes: source.script_count as u64,
        total_bytes: Some(source.script_count as u64),
    });
    Ok(source)
}

enum DownloadedRepository {
    Modified {
        etag: Option<String>,
        last_modified: Option<String>,
        repository: ScriptRepository,
    },
    NotModified,
}

fn normalize_repository_url(url: &str) -> Result<String, ScriptRepositoryServiceError> {
    let mut url = validate_public_https_repository_url(url.trim())
        .map_err(|error| ScriptRepositoryServiceError::Url(error.to_string()))?;
    url.set_fragment(None);
    Ok(url.to_string())
}

fn storage_error(error: impl ToString) -> ScriptRepositoryServiceError {
    ScriptRepositoryServiceError::Storage(error.to_string())
}

fn current_unix_timestamp() -> u64 {
    Utc::now().timestamp().max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::{
        LEGACY_OFFICIAL_REPOSITORY_URL, OFFICIAL_REPOSITORY_URL, ensure_official_repository,
    };
    use baudbound_storage::SqliteRunnerStore;

    #[test]
    fn ensuring_official_repository_migrates_legacy_branch_url() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let store = SqliteRunnerStore::open(temporary_directory.path().join("runner.sqlite3"))
            .expect("runner store should open");
        store
            .ensure_repository_source(LEGACY_OFFICIAL_REPOSITORY_URL, true)
            .expect("legacy official repository should be stored");

        let repository =
            ensure_official_repository(&store).expect("official repository should be ensured");

        assert_eq!(repository.url, OFFICIAL_REPOSITORY_URL);
        assert!(repository.official);
        assert!(
            store
                .repository_source(LEGACY_OFFICIAL_REPOSITORY_URL)
                .expect("legacy repository lookup should succeed")
                .is_none()
        );
        assert_eq!(
            store
                .list_repository_sources()
                .expect("repository sources should list")
                .len(),
            1
        );
    }
}
