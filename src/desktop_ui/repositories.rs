use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

use baudbound_core::RunnerConfig;
use baudbound_storage::{
    PaginatedRecords, RepositoryScriptFilterOptions, RepositoryScriptQuery, RepositoryScriptRecord,
    RepositoryScriptSummary, RepositorySource, SqliteRunnerStore,
};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use super::{
    DesktopUiState, REMOTE_PACKAGE_PROGRESS_EVENT, RemotePackageProgressPayload,
    RemotePackageReviewPayload, current_core, current_runner_config,
};

pub(super) const REPOSITORY_CHANGED_EVENT: &str = "runner-repository-changed";
pub(super) const REPOSITORY_PROGRESS_EVENT: &str = "runner-repository-progress";
const MAX_POLL_INTERVAL: Duration = Duration::from_secs(60);
const MIN_POLL_INTERVAL: Duration = Duration::from_secs(10);
type WorkerControl = Arc<(Mutex<bool>, Condvar)>;

#[derive(Default)]
pub(super) struct RepositoryRefreshWorker {
    control: Mutex<Option<WorkerControl>>,
}

impl RepositoryRefreshWorker {
    pub(super) fn start<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        config_path: PathBuf,
        store: SqliteRunnerStore,
    ) {
        let mut current = self
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if current.is_some() {
            return;
        }
        let control = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_control = Arc::clone(&control);
        match thread::Builder::new()
            .name("baudbound-repository-refresh".to_owned())
            .spawn(move || run_refresh_worker(app, config_path, store, worker_control))
        {
            Ok(_) => *current = Some(control),
            Err(error) => tracing::warn!(%error, "failed to start repository refresh worker"),
        }
    }

    pub(super) fn wake(&self) {
        let current = self
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(control) = current.as_ref() {
            control.1.notify_all();
        }
    }
}

impl Drop for RepositoryRefreshWorker {
    fn drop(&mut self) {
        let current = self
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(control) = current.as_ref() {
            let mut shutdown = control.0.lock().unwrap_or_else(|error| error.into_inner());
            *shutdown = true;
            control.1.notify_all();
        }
    }
}

fn run_refresh_worker<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    config_path: PathBuf,
    store: SqliteRunnerStore,
    control: WorkerControl,
) {
    loop {
        let poll_interval = match RunnerConfig::load_or_default(&config_path) {
            Ok(config) => {
                refresh_due_repositories(&app, &store, &config);
                Duration::from_secs(config.updates.check_interval_hours.saturating_mul(60 * 60))
                    .clamp(MIN_POLL_INTERVAL, MAX_POLL_INTERVAL)
            }
            Err(error) => {
                tracing::debug!(%error, "failed to load configuration for repository refresh");
                MAX_POLL_INTERVAL
            }
        };
        if wait_or_shutdown(&control, poll_interval) {
            return;
        }
    }
}

fn refresh_due_repositories<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SqliteRunnerStore,
    config: &RunnerConfig,
) {
    if let Err(error) = crate::script_repositories::ensure_official_repository(store) {
        tracing::debug!(%error, "failed to ensure the official script repository");
    }
    let sources = match crate::script_repositories::list_repositories(store) {
        Ok(sources) => sources,
        Err(error) => {
            tracing::debug!(%error, "failed to list script repositories for background refresh");
            return;
        }
    };
    let now = chrono::Utc::now().timestamp().max(0) as u64;
    let interval = config.updates.check_interval_hours.saturating_mul(60 * 60);
    for source in sources.into_iter().filter(|source| source.enabled) {
        if !repository_refresh_is_due(source.last_refresh_at_unix, now, interval) {
            continue;
        }
        let url = source.url;
        let result = crate::script_repositories::refresh_repository(
            store,
            config.limits.max_file_download_bytes,
            &url,
            &mut |progress| {
                let _ = app.emit(
                    REPOSITORY_PROGRESS_EVENT,
                    RepositoryProgressPayload {
                        request_id: "automatic".to_owned(),
                        progress,
                    },
                );
                true
            },
        );
        if let Err(error) = &result {
            tracing::debug!(
                %error,
                repository_url = %repository_url_for_diagnostics(&url),
                "background repository refresh failed"
            );
        }
        emit_repository_changed(app, &url);
    }
}

fn wait_or_shutdown(control: &WorkerControl, duration: Duration) -> bool {
    let (shutdown, wake) = &**control;
    let shutdown = shutdown.lock().unwrap_or_else(|error| error.into_inner());
    if *shutdown {
        return true;
    }
    let (shutdown, _) = wake
        .wait_timeout(shutdown, duration)
        .unwrap_or_else(|error| error.into_inner());
    *shutdown
}

fn repository_refresh_is_due(last_refresh: Option<u64>, now: u64, interval: u64) -> bool {
    last_refresh.is_none_or(|last_refresh| now.saturating_sub(last_refresh) >= interval)
}

fn repository_url_for_diagnostics(value: &str) -> String {
    let Ok(mut url) = url::Url::parse(value) else {
        return "invalid repository URL".to_owned();
    };
    url.set_query(url.query().map(|_| "redacted"));
    url.to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AddRepositoryRequest {
    pub(super) request_id: String,
    pub(super) url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PrepareRepositoryScriptRequest {
    pub(super) repository_url: String,
    pub(super) request_id: String,
    pub(super) script_id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct RefreshAllRepositoriesResult {
    pub(super) failures: Vec<RepositoryRefreshFailure>,
    pub(super) repositories: Vec<RepositorySource>,
}

#[derive(Debug, Serialize)]
pub(super) struct RepositoryRefreshFailure {
    pub(super) message: String,
    pub(super) repository_url: String,
}

#[tauri::command]
pub(super) fn repository_sources(
    state: State<'_, DesktopUiState>,
) -> Result<Vec<RepositorySource>, String> {
    crate::script_repositories::ensure_official_repository(&state.store)
        .map_err(|error| error.to_string())?;
    crate::script_repositories::list_repositories(&state.store).map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) fn query_repository_scripts(
    query: RepositoryScriptQuery,
    state: State<'_, DesktopUiState>,
) -> Result<PaginatedRecords<RepositoryScriptSummary>, String> {
    crate::script_repositories::query_scripts(&state.store, &query)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) fn repository_script_filter_options() -> RepositoryScriptFilterOptions {
    crate::script_repositories::script_filter_options()
}

#[tauri::command]
pub(super) fn repository_script_details(
    repository_url: String,
    script_id: String,
    state: State<'_, DesktopUiState>,
) -> Result<RepositoryScriptRecord, String> {
    crate::script_repositories::script_details(&state.store, &repository_url, &script_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) async fn add_script_repository<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    preparations: State<'_, crate::script_updates::RemotePreparationRegistry>,
    request: AddRepositoryRequest,
    state: State<'_, DesktopUiState>,
) -> Result<RepositorySource, String> {
    let preparation = preparations.start(&request.request_id)?;
    let cancellation = preparation.cancellation_token();
    let store = state.store.clone();
    let package_limit = repository_download_limit(&state)?;
    let request_id = request.request_id;
    let progress_app = app.clone();
    let source = tauri::async_runtime::spawn_blocking(move || {
        crate::script_repositories::add_repository(
            &store,
            package_limit,
            &request.url,
            &mut |progress| {
                let _ = progress_app.emit(
                    REPOSITORY_PROGRESS_EVENT,
                    RepositoryProgressPayload {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
                !cancellation.is_cancelled()
            },
        )
    })
    .await
    .map_err(|error| format!("repository task failed: {error}"))?
    .map_err(|error| error.to_string())?;
    emit_repository_changed(&app, &source.url);
    Ok(source)
}

#[tauri::command]
pub(super) async fn preview_script_repository<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    preparations: State<'_, crate::script_updates::RemotePreparationRegistry>,
    request: AddRepositoryRequest,
    state: State<'_, DesktopUiState>,
) -> Result<crate::script_repositories::RepositoryPreview, String> {
    let preparation = preparations.start(&request.request_id)?;
    let cancellation = preparation.cancellation_token();
    let package_limit = repository_download_limit(&state)?;
    let request_id = request.request_id;
    let progress_app = app;
    tauri::async_runtime::spawn_blocking(move || {
        crate::script_repositories::preview_repository(
            package_limit,
            &request.url,
            &mut |progress| {
                let _ = progress_app.emit(
                    REPOSITORY_PROGRESS_EVENT,
                    RepositoryProgressPayload {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
                !cancellation.is_cancelled()
            },
        )
    })
    .await
    .map_err(|error| format!("repository preview task failed: {error}"))?
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) async fn refresh_script_repository<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    preparations: State<'_, crate::script_updates::RemotePreparationRegistry>,
    request_id: String,
    url: String,
    state: State<'_, DesktopUiState>,
) -> Result<RepositorySource, String> {
    let preparation = preparations.start(&request_id)?;
    let cancellation = preparation.cancellation_token();
    let store = state.store.clone();
    let package_limit = repository_download_limit(&state)?;
    let progress_app = app.clone();
    let refresh_url = url.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::script_repositories::refresh_repository(
            &store,
            package_limit,
            &url,
            &mut |progress| {
                let _ = progress_app.emit(
                    REPOSITORY_PROGRESS_EVENT,
                    RepositoryProgressPayload {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
                !cancellation.is_cancelled()
            },
        )
    })
    .await
    .map_err(|error| format!("repository refresh task failed: {error}"))?;
    match result {
        Ok(source) => {
            emit_repository_changed(&app, &source.url);
            Ok(source)
        }
        Err(error) => {
            emit_repository_changed(&app, &refresh_url);
            Err(error.to_string())
        }
    }
}

#[tauri::command]
pub(super) async fn refresh_all_script_repositories<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    preparations: State<'_, crate::script_updates::RemotePreparationRegistry>,
    request_id: String,
    state: State<'_, DesktopUiState>,
) -> Result<RefreshAllRepositoriesResult, String> {
    let preparation = preparations.start(&request_id)?;
    let cancellation = preparation.cancellation_token();
    let store = state.store.clone();
    let package_limit = repository_download_limit(&state)?;
    crate::script_repositories::ensure_official_repository(&store)
        .map_err(|error| error.to_string())?;
    let sources =
        crate::script_repositories::list_repositories(&store).map_err(|error| error.to_string())?;
    let enabled = sources
        .into_iter()
        .filter(|source| source.enabled)
        .collect::<Vec<_>>();
    let mut failures = Vec::new();
    for source in enabled {
        if cancellation.is_cancelled() {
            return Err("repository refresh was cancelled".to_owned());
        }
        let app_for_task = app.clone();
        let request_id_for_task = request_id.clone();
        let store_for_task = store.clone();
        let cancellation_for_task = cancellation.clone();
        let url = source.url.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            crate::script_repositories::refresh_repository(
                &store_for_task,
                package_limit,
                &url,
                &mut |progress| {
                    let _ = app_for_task.emit(
                        REPOSITORY_PROGRESS_EVENT,
                        RepositoryProgressPayload {
                            request_id: request_id_for_task.clone(),
                            progress,
                        },
                    );
                    !cancellation_for_task.is_cancelled()
                },
            )
        })
        .await
        .map_err(|error| format!("repository refresh task failed: {error}"))?;
        match result {
            Ok(refreshed) => emit_repository_changed(&app, &refreshed.url),
            Err(error) => {
                emit_repository_changed(&app, &source.url);
                failures.push(RepositoryRefreshFailure {
                    message: error.to_string(),
                    repository_url: source.url,
                });
            }
        }
    }
    Ok(RefreshAllRepositoriesResult {
        failures,
        repositories: crate::script_repositories::list_repositories(&store)
            .map_err(|error| error.to_string())?,
    })
}

#[tauri::command]
pub(super) fn set_script_repository_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    enabled: bool,
    state: State<'_, DesktopUiState>,
    url: String,
) -> Result<RepositorySource, String> {
    let source = state
        .store
        .set_repository_enabled(&url, enabled)
        .map_err(|error| error.to_string())?;
    emit_repository_changed(&app, &source.url);
    state.repository_refresh_worker.wake();
    Ok(source)
}

#[tauri::command]
pub(super) fn remove_script_repository<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, DesktopUiState>,
    url: String,
) -> Result<bool, String> {
    let removed = state
        .store
        .remove_repository_source(&url)
        .map_err(|error| error.to_string())?;
    if removed {
        emit_repository_changed(&app, &url);
    }
    Ok(removed)
}

#[tauri::command]
pub(super) async fn prepare_repository_script<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    preparations: State<'_, crate::script_updates::RemotePreparationRegistry>,
    request: PrepareRepositoryScriptRequest,
    reviews: State<'_, crate::script_updates::RemotePackageReviews>,
    state: State<'_, DesktopUiState>,
) -> Result<RemotePackageReviewPayload, String> {
    let preparation = preparations.start(&request.request_id)?;
    let core = current_core(&state).map_err(|error| error.to_string())?;
    let store = state.store.clone();
    let package_limit = repository_download_limit(&state)?;
    let request_id = request.request_id;
    let repository_url = request.repository_url.clone();
    let progress_app = app.clone();
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        let mut progress = |progress| {
            if preparation.is_cancelled() {
                return false;
            }
            let _ = progress_app.emit(
                REMOTE_PACKAGE_PROGRESS_EVENT,
                RemotePackageProgressPayload {
                    request_id: request_id.clone(),
                    progress,
                },
            );
            !preparation.is_cancelled()
        };
        crate::script_repositories::prepare_repository_script_with_progress(
            &core,
            &store,
            package_limit,
            &request.repository_url,
            &request.script_id,
            &mut progress,
        )
    })
    .await
    .map_err(|error| format!("repository package task failed: {error}"))?
    .map_err(|error| error.to_string())?;
    emit_repository_changed(&app, &repository_url);
    let review = prepared.review.clone();
    let review_id = reviews.insert(prepared)?;
    Ok(RemotePackageReviewPayload { review_id, review })
}

#[derive(Clone, Serialize)]
struct RepositoryProgressPayload {
    request_id: String,
    #[serde(flatten)]
    progress: crate::script_repositories::RepositoryRefreshProgress,
}

fn repository_download_limit(state: &DesktopUiState) -> Result<u64, String> {
    Ok(current_runner_config(state)
        .map_err(|error| error.to_string())?
        .limits
        .max_file_download_bytes as u64)
}

fn emit_repository_changed<R: tauri::Runtime>(app: &tauri::AppHandle<R>, repository_url: &str) {
    if let Err(error) = app.emit(REPOSITORY_CHANGED_EVENT, repository_url) {
        tracing::warn!(%error, "failed to publish repository change event");
    }
}

#[cfg(test)]
mod tests {
    use super::{repository_refresh_is_due, repository_url_for_diagnostics};

    #[test]
    fn repositories_without_a_refresh_are_due_immediately() {
        assert!(repository_refresh_is_due(None, 1_000, 3_600));
    }

    #[test]
    fn repositories_refresh_only_after_the_interval() {
        assert!(!repository_refresh_is_due(Some(1_000), 4_599, 3_600));
        assert!(repository_refresh_is_due(Some(1_000), 4_600, 3_600));
    }

    #[test]
    fn a_future_timestamp_does_not_force_an_early_refresh() {
        assert!(!repository_refresh_is_due(Some(2_000), 1_000, 3_600));
    }

    #[test]
    fn repository_diagnostics_hide_query_values() {
        assert_eq!(
            repository_url_for_diagnostics(
                "https://example.com/repository.json?token=private-value"
            ),
            "https://example.com/repository.json?redacted"
        );
    }
}
