use std::{collections::BTreeMap, fs, path::PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use super::{
    ActionPayload, DashboardPayload, DesktopUiState, PackageActionPayload, build_dashboard_payload,
    command_guard::{SensitiveOperation, SensitiveOperationGuard, ensure_main_window},
    consume_package_selection, consume_sensitive_operation, current_core, current_runner_config,
    repositories, run_locked_action, run_locked_value,
};

#[tauri::command]
pub(super) async fn check_script_update<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    reference: String,
    state: State<'_, DesktopUiState>,
) -> Result<ActionPayload, String> {
    let store = state.store.clone();
    let package_limit = current_runner_config(&state)
        .map_err(|error| error.to_string())?
        .limits
        .max_file_download_bytes as u64;
    let reference_for_check = reference.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::script_updates::check_script_update(&store, package_limit, &reference_for_check)
    })
    .await
    .map_err(|error| format!("update check task failed: {error}"))?;
    if let Err(error) = app.emit(crate::script_updates::SCRIPT_UPDATE_EVENT, &reference) {
        tracing::debug!(%error, "failed to publish manual script update state event");
    }
    result.map_err(|error| error.to_string())?;
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload {
        dashboard,
        message: format!("Checked {reference} for updates."),
    })
}

#[derive(Serialize)]
pub(super) struct ScriptUpdateBatchPayload {
    dashboard: DashboardPayload,
    errors: BTreeMap<String, String>,
}

#[tauri::command]
pub(super) async fn check_script_updates<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    references: Vec<String>,
    state: State<'_, DesktopUiState>,
) -> Result<ScriptUpdateBatchPayload, String> {
    if references.is_empty() || references.len() > 1_000 {
        return Err("select between 1 and 1,000 scripts to check".to_owned());
    }
    let store = state.store.clone();
    let package_limit = current_runner_config(&state)
        .map_err(|error| error.to_string())?
        .limits
        .max_file_download_bytes as u64;
    let references_for_check = references.clone();
    let results = tauri::async_runtime::spawn_blocking(move || {
        crate::script_updates::check_script_updates(&store, package_limit, &references_for_check)
    })
    .await
    .map_err(|error| format!("update check task failed: {error}"))?;
    for reference in &references {
        if let Err(error) = app.emit(crate::script_updates::SCRIPT_UPDATE_EVENT, reference) {
            tracing::debug!(%error, "failed to publish batch script update state event");
        }
    }
    let errors = results
        .into_iter()
        .filter_map(|(script_id, result)| result.err().map(|error| (script_id, error)))
        .collect();
    Ok(ScriptUpdateBatchPayload {
        dashboard: build_dashboard_payload(&state).map_err(|error| error.to_string())?,
        errors,
    })
}

#[tauri::command]
pub(super) async fn select_package_file<R: tauri::Runtime>(
    operation: PackageFileOperation,
    guard: State<'_, SensitiveOperationGuard>,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<Option<PackageFileSelection>, String> {
    ensure_main_window(&window)?;
    let selected = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .add_filter("BaudBound package", &["bbs"])
        .pick_file()
        .await;

    selected
        .map(|file| {
            let package_path = file
                .path()
                .to_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| "the selected package path is not valid UTF-8".to_owned())?;
            let sensitive_operation = operation.sensitive_operation(package_path.clone());
            let challenge = guard.prepare_package_selection(&sensitive_operation, &state)?;
            Ok(PackageFileSelection {
                confirmation_id: challenge.into_confirmation_id(),
                package_path,
            })
        })
        .transpose()
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PackageFileOperation {
    Import,
    Update,
}

impl PackageFileOperation {
    fn sensitive_operation(self, package_path: String) -> SensitiveOperation {
        match self {
            Self::Import => SensitiveOperation::ImportScriptPackage { package_path },
            Self::Update => SensitiveOperation::UpdateScriptPackage { package_path },
        }
    }
}

#[derive(Serialize)]
pub(super) struct PackageFileSelection {
    confirmation_id: String,
    package_path: String,
}

#[tauri::command]
pub(super) fn approve_script<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    reference: String,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<PackageActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::ApproveScript {
            reference: reference.clone(),
        },
        &guard,
        &state,
        &window,
    )?;
    let result = run_locked_value(&state, || {
        Ok(current_core(&state)?.approve_installed(&state.store, &reference)?)
    })?;
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(PackageActionPayload {
        dashboard,
        generated_trigger_tokens: result.generated_trigger_tokens,
        message: format!("Approved {reference}."),
    })
}

#[tauri::command]
pub(super) fn revoke_script_approval<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    reference: String,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::RevokeScriptApproval {
            reference: reference.clone(),
        },
        &guard,
        &state,
        &window,
    )?;
    run_locked_action(&state, || {
        let revoked = current_core(&state)?.revoke_approval(&state.store, &reference)?;
        Ok(if revoked.is_some() {
            format!("Revoked approval for {reference}.")
        } else {
            format!("No approval was stored for {reference}.")
        })
    })
}

#[tauri::command]
pub(super) fn import_script_package<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    package_path: String,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_package_selection(
        &confirmation_id,
        &SensitiveOperation::ImportScriptPackage {
            package_path: package_path.clone(),
        },
        &guard,
        &state,
        &window,
    )?;
    let path = PathBuf::from(package_path);
    let script = run_locked_value(&state, || {
        Ok(current_core(&state)?.import_package(&state.store, &path)?)
    })?;
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload {
        dashboard,
        message: format!(
            "Imported {} ({}) as {}.",
            script.name, script.id, script.package_file_name
        ),
    })
}

#[tauri::command]
pub(super) fn update_script_package<R: tauri::Runtime>(
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    package_path: String,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    consume_package_selection(
        &confirmation_id,
        &SensitiveOperation::UpdateScriptPackage {
            package_path: package_path.clone(),
        },
        &guard,
        &state,
        &window,
    )?;
    let path = PathBuf::from(package_path);
    let has_repository_url = !baudbound_script::load_script_package(&path)
        .map_err(|error| error.to_string())?
        .manifest
        .repository_url
        .trim()
        .is_empty();
    let script = run_locked_value(&state, || {
        let script = current_core(&state)?.update_package(&state.store, &path)?;
        crate::script_updates::reconcile_script_update_state_after_install(
            &state.store,
            &script.id,
            has_repository_url,
        )?;
        Ok(script)
    })?;
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    Ok(ActionPayload {
        dashboard,
        message: format!(
            "Updated {} ({}) as {}.",
            script.name, script.id, script.package_file_name
        ),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PrepareRemotePackageRequest {
    operation: crate::script_updates::RemotePackageOperation,
    request_id: String,
    source: crate::script_updates::RemotePackageSource,
    url: String,
}

#[derive(Serialize)]
pub(super) struct RemotePackageReviewPayload {
    pub(super) review_id: String,
    #[serde(flatten)]
    pub(super) review: crate::script_updates::RemotePackageReview,
}

pub(super) const REMOTE_PACKAGE_PROGRESS_EVENT: &str = "runner-remote-package-progress";

#[derive(Clone, Serialize)]
pub(super) struct RemotePackageProgressPayload {
    pub(super) request_id: String,
    #[serde(flatten)]
    pub(super) progress: crate::script_updates::RemotePreparationProgress,
}

#[tauri::command]
pub(super) async fn prepare_remote_script_package<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    preparations: State<'_, crate::script_updates::RemotePreparationRegistry>,
    request: PrepareRemotePackageRequest,
    reviews: State<'_, crate::script_updates::RemotePackageReviews>,
    state: State<'_, DesktopUiState>,
) -> Result<RemotePackageReviewPayload, String> {
    let preparation = preparations.start(&request.request_id)?;
    let request_id = request.request_id.clone();
    let core = current_core(&state).map_err(|error| error.to_string())?;
    let store = state.store.clone();
    let package_limit = current_runner_config(&state)
        .map_err(|error| error.to_string())?
        .limits
        .max_file_download_bytes as u64;
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        let mut progress = |progress| {
            if preparation.is_cancelled() {
                return false;
            }
            let _ = app.emit(
                REMOTE_PACKAGE_PROGRESS_EVENT,
                RemotePackageProgressPayload {
                    request_id: request_id.clone(),
                    progress,
                },
            );
            !preparation.is_cancelled()
        };
        crate::script_updates::prepare_remote_package_with_progress(
            &core,
            &store,
            package_limit,
            request.operation,
            request.source,
            &request.url,
            &mut progress,
        )
    })
    .await
    .map_err(|error| format!("remote package preparation task failed: {error}"))?
    .map_err(|error| error.to_string())?;
    let review = prepared.review.clone();
    let review_id = reviews.insert(prepared)?;
    Ok(RemotePackageReviewPayload { review_id, review })
}

#[tauri::command]
pub(super) async fn prepare_discovered_script_update<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    preparations: State<'_, crate::script_updates::RemotePreparationRegistry>,
    reference: String,
    request_id: String,
    reviews: State<'_, crate::script_updates::RemotePackageReviews>,
    state: State<'_, DesktopUiState>,
) -> Result<RemotePackageReviewPayload, String> {
    let preparation = preparations.start(&request_id)?;
    let core = current_core(&state).map_err(|error| error.to_string())?;
    let store = state.store.clone();
    let package_limit = current_runner_config(&state)
        .map_err(|error| error.to_string())?
        .limits
        .max_file_download_bytes as u64;
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        let mut progress = |progress| {
            if preparation.is_cancelled() {
                return false;
            }
            let _ = app.emit(
                REMOTE_PACKAGE_PROGRESS_EVENT,
                RemotePackageProgressPayload {
                    request_id: request_id.clone(),
                    progress,
                },
            );
            !preparation.is_cancelled()
        };
        crate::script_updates::prepare_discovered_update_with_progress(
            &core,
            &store,
            package_limit,
            &reference,
            &mut progress,
        )
    })
    .await
    .map_err(|error| format!("remote update preparation task failed: {error}"))?
    .map_err(|error| error.to_string())?;
    let review = prepared.review.clone();
    let review_id = reviews.insert(prepared)?;
    Ok(RemotePackageReviewPayload { review_id, review })
}

#[tauri::command]
pub(super) fn cancel_remote_script_package_preparation(
    request_id: String,
    preparations: State<'_, crate::script_updates::RemotePreparationRegistry>,
) -> Result<bool, String> {
    preparations.cancel(&request_id)
}

#[tauri::command]
pub(super) fn discard_remote_package_review(
    review_id: String,
    reviews: State<'_, crate::script_updates::RemotePackageReviews>,
) -> Result<bool, String> {
    reviews.discard(&review_id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InstallRemoteScriptPackageRequest {
    review_id: String,
    sha256: String,
}

#[tauri::command]
pub(super) fn install_remote_script_package<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    confirmation_id: String,
    guard: State<'_, SensitiveOperationGuard>,
    request: InstallRemoteScriptPackageRequest,
    reviews: State<'_, crate::script_updates::RemotePackageReviews>,
    state: State<'_, DesktopUiState>,
    window: tauri::WebviewWindow<R>,
) -> Result<ActionPayload, String> {
    let InstallRemoteScriptPackageRequest { review_id, sha256 } = request;
    consume_sensitive_operation(
        &confirmation_id,
        &SensitiveOperation::InstallRemoteScriptPackage {
            review_id: review_id.clone(),
            sha256: sha256.clone(),
        },
        &guard,
        &state,
        &window,
    )?;
    let prepared = reviews.take(&review_id, &sha256)?;
    let actual_hash = sha256_path(prepared.download.file.path())?;
    if actual_hash != sha256 {
        return Err("the downloaded package changed after review".to_owned());
    }
    let operation = prepared.review.operation;
    let script_id = prepared.review.script_id.clone();
    let script_name = prepared.review.script_name.clone();
    let has_repository_url = !prepared.review.repository_url.trim().is_empty();
    run_locked_value(&state, || {
        let directory = tempfile::Builder::new()
            .prefix("baudbound-reviewed-package-")
            .tempdir()?;
        let package_path = directory.path().join(format!("{script_id}.bbs"));
        fs::copy(prepared.download.file.path(), &package_path)?;
        match operation {
            crate::script_updates::RemotePackageOperation::Import => {
                current_core(&state)?.import_package(&state.store, &package_path)?;
            }
            crate::script_updates::RemotePackageOperation::Update => {
                current_core(&state)?.update_package(&state.store, &package_path)?;
                crate::script_updates::reconcile_script_update_state_after_install(
                    &state.store,
                    &script_id,
                    has_repository_url,
                )?;
            }
        }
        Ok(())
    })?;
    let package_urls = std::iter::once(prepared.download.original_url.to_string())
        .chain(
            prepared
                .download
                .redirect_urls
                .iter()
                .map(ToString::to_string),
        )
        .collect::<Vec<_>>();
    let publishers = package_urls
        .iter()
        .filter_map(|value| url::Url::parse(value).ok())
        .filter_map(|url| baudbound_security::github_publisher_for_url(&url))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    state
        .blacklist
        .record_provenance(
            &script_id,
            crate::blacklist::TrustedProvenance {
                final_package_url: Some(prepared.download.final_url.to_string()),
                package_urls,
                publishers,
                repository_url: prepared.trusted_repository_url,
            },
        )
        .map_err(|error| error.to_string())?;
    let dashboard = build_dashboard_payload(&state).map_err(|error| error.to_string())?;
    if let Err(error) = app.emit(repositories::REPOSITORY_CHANGED_EVENT, &script_id) {
        tracing::warn!(%error, "failed to publish repository script change event");
    }
    Ok(ActionPayload {
        dashboard,
        message: format!(
            "{} {script_name}. Review and approve the installed package before running it.",
            match operation {
                crate::script_updates::RemotePackageOperation::Import => "Imported",
                crate::script_updates::RemotePackageOperation::Update => "Updated",
            }
        ),
    })
}

fn sha256_path(path: &std::path::Path) -> Result<String, String> {
    use sha2::{Digest as _, Sha256};
    use std::io::Read as _;

    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
