use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::Path,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use baudbound_core::{MAX_RUNNER_CONFIG_BYTES, MAX_SECRET_INPUT_BYTES, RunnerConfig};
use baudbound_storage::{NetworkTriggerType, ScriptStore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{Runtime, WebviewWindow};

use super::DesktopUiState;

const CHALLENGE_LIFETIME: Duration = Duration::from_secs(120);
const MAX_PENDING_CHALLENGES: usize = 128;
const MAX_REFERENCE_BYTES: usize = 256;
const MAX_NODE_ID_BYTES: usize = 128;
const MAX_NAME_BYTES: usize = 128;
const MAX_PACKAGE_PATH_BYTES: usize = 32_768;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum SensitiveOperation {
    ApproveScript {
        reference: String,
    },
    ImportScriptPackage {
        package_path: String,
    },
    UpdateScriptPackage {
        package_path: String,
    },
    SetScriptAutomaticUpdateChecks {
        reference: String,
        enabled: bool,
    },
    InstallRemoteScriptPackage {
        review_id: String,
        sha256: String,
    },
    RunScript {
        reference: String,
    },
    SaveRunnerConfig {
        contents: String,
        restart_background: bool,
    },
    SaveRunnerConfigModel {
        config: RunnerConfig,
        restart_background: bool,
    },
    ResetRunnerConfig {
        restart_background: bool,
    },
    RotateNetworkTriggerToken {
        reference: String,
        node_id: String,
        trigger_type: NetworkTriggerType,
    },
    SetNetworkTriggerAuthEnabled {
        reference: String,
        node_id: String,
        trigger_type: NetworkTriggerType,
        enabled: bool,
    },
    SetScriptSecret {
        reference: String,
        name: String,
        value: String,
    },
    RemoveScriptSecret {
        reference: String,
        name: String,
    },
    ClearRunHistory,
    ClearRunLogs,
}

impl SensitiveOperation {
    fn kind(&self) -> &'static str {
        match self {
            Self::ApproveScript { .. } => "approve_script",
            Self::ImportScriptPackage { .. } => "import_script_package",
            Self::UpdateScriptPackage { .. } => "update_script_package",
            Self::SetScriptAutomaticUpdateChecks { .. } => "set_script_automatic_update_checks",
            Self::InstallRemoteScriptPackage { .. } => "install_remote_script_package",
            Self::RunScript { .. } => "run_script",
            Self::SaveRunnerConfig { .. } => "save_runner_config",
            Self::SaveRunnerConfigModel { .. } => "save_runner_config_model",
            Self::ResetRunnerConfig { .. } => "reset_runner_config",
            Self::RotateNetworkTriggerToken { .. } => "rotate_network_trigger_token",
            Self::SetNetworkTriggerAuthEnabled { .. } => "set_network_trigger_auth_enabled",
            Self::SetScriptSecret { .. } => "set_script_secret",
            Self::RemoveScriptSecret { .. } => "remove_script_secret",
            Self::ClearRunHistory => "clear_run_history",
            Self::ClearRunLogs => "clear_run_logs",
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::ApproveScript { reference } => format!("Approve {reference}"),
            Self::ImportScriptPackage { package_path } => {
                format!("Import package {package_path}")
            }
            Self::UpdateScriptPackage { package_path } => {
                format!("Update package from {package_path}")
            }
            Self::SetScriptAutomaticUpdateChecks { reference, enabled } => format!(
                "{} automatic update checks for {reference}",
                if *enabled { "Enable" } else { "Disable" }
            ),
            Self::InstallRemoteScriptPackage { .. } => {
                "Install the reviewed remote script package".to_owned()
            }
            Self::RunScript { reference } => format!("Run {reference}"),
            Self::SaveRunnerConfig { .. } | Self::SaveRunnerConfigModel { .. } => {
                "Save runner configuration".to_owned()
            }
            Self::ResetRunnerConfig { .. } => "Reset runner configuration".to_owned(),
            Self::RotateNetworkTriggerToken {
                reference, node_id, ..
            } => format!("Rotate network token for {reference}:{node_id}"),
            Self::SetNetworkTriggerAuthEnabled {
                reference,
                node_id,
                enabled,
                ..
            } => format!(
                "{} network authentication for {reference}:{node_id}",
                if *enabled { "Enable" } else { "Disable" }
            ),
            Self::SetScriptSecret {
                reference, name, ..
            } => format!("Set secret {name} for {reference}"),
            Self::RemoveScriptSecret { reference, name } => {
                format!("Remove secret {name} from {reference}")
            }
            Self::ClearRunHistory => "Clear stored run history".to_owned(),
            Self::ClearRunLogs => "Clear stored run logs".to_owned(),
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct ConfirmationChallenge {
    confirmation_id: String,
    expires_at_unix_ms: u128,
    operation_kind: String,
    summary: String,
}

struct PendingChallenge {
    authorization: ChallengeAuthorization,
    digest: [u8; 32],
    expires_at: Instant,
    operation_kind: &'static str,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ChallengeAuthorization {
    FocusedMainWindow,
    NativePackagePicker,
}

#[derive(Default)]
pub(super) struct SensitiveOperationGuard {
    pending: Mutex<HashMap<String, PendingChallenge>>,
}

impl SensitiveOperationGuard {
    pub(super) fn prepare(
        &self,
        operation: &SensitiveOperation,
        state: &DesktopUiState,
    ) -> Result<ConfirmationChallenge, String> {
        if matches!(
            operation,
            SensitiveOperation::ImportScriptPackage { .. }
                | SensitiveOperation::UpdateScriptPackage { .. }
        ) {
            return Err("package import and update must use the native file picker".to_owned());
        }
        self.prepare_with_authorization(operation, state, ChallengeAuthorization::FocusedMainWindow)
    }

    pub(super) fn prepare_package_selection(
        &self,
        operation: &SensitiveOperation,
        state: &DesktopUiState,
    ) -> Result<ConfirmationChallenge, String> {
        if !matches!(
            operation,
            SensitiveOperation::ImportScriptPackage { .. }
                | SensitiveOperation::UpdateScriptPackage { .. }
        ) {
            return Err("native package selection can only authorize import or update".to_owned());
        }
        self.prepare_with_authorization(
            operation,
            state,
            ChallengeAuthorization::NativePackagePicker,
        )
    }

    fn prepare_with_authorization(
        &self,
        operation: &SensitiveOperation,
        state: &DesktopUiState,
        authorization: ChallengeAuthorization,
    ) -> Result<ConfirmationChallenge, String> {
        validate_operation(operation, state)?;
        let digest = operation_digest(operation, state)?;
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "sensitive operation guard lock is poisoned".to_owned())?;
        pending.retain(|_, challenge| challenge.expires_at > Instant::now());
        if pending.len() >= MAX_PENDING_CHALLENGES {
            return Err("too many sensitive operations are awaiting confirmation".to_owned());
        }
        let confirmation_id = random_confirmation_id()?;
        let expires_at = Instant::now() + CHALLENGE_LIFETIME;
        pending.insert(
            confirmation_id.clone(),
            PendingChallenge {
                authorization,
                digest,
                expires_at,
                operation_kind: operation.kind(),
            },
        );
        let expires_at_unix_ms = SystemTime::now()
            .checked_add(CHALLENGE_LIFETIME)
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |value| value.as_millis());
        Ok(ConfirmationChallenge {
            confirmation_id,
            expires_at_unix_ms,
            operation_kind: operation.kind().to_owned(),
            summary: operation.summary(),
        })
    }

    pub(super) fn consume(
        &self,
        confirmation_id: &str,
        operation: &SensitiveOperation,
        state: &DesktopUiState,
    ) -> Result<(), String> {
        self.consume_with_authorization(
            confirmation_id,
            operation,
            state,
            ChallengeAuthorization::FocusedMainWindow,
        )
    }

    pub(super) fn consume_package_selection(
        &self,
        confirmation_id: &str,
        operation: &SensitiveOperation,
        state: &DesktopUiState,
    ) -> Result<(), String> {
        self.consume_with_authorization(
            confirmation_id,
            operation,
            state,
            ChallengeAuthorization::NativePackagePicker,
        )
    }

    fn consume_with_authorization(
        &self,
        confirmation_id: &str,
        operation: &SensitiveOperation,
        state: &DesktopUiState,
        authorization: ChallengeAuthorization,
    ) -> Result<(), String> {
        validate_operation(operation, state)?;
        let challenge = self
            .pending
            .lock()
            .map_err(|_| "sensitive operation guard lock is poisoned".to_owned())?
            .remove(confirmation_id)
            .ok_or_else(|| {
                "sensitive operation confirmation is missing or already used".to_owned()
            })?;
        if challenge.expires_at <= Instant::now() {
            return Err("sensitive operation confirmation has expired".to_owned());
        }
        if challenge.authorization != authorization {
            return Err("sensitive operation confirmation came from an invalid source".to_owned());
        }
        if challenge.operation_kind != operation.kind() {
            return Err("sensitive operation confirmation is for a different action".to_owned());
        }
        if challenge.digest != operation_digest(operation, state)? {
            return Err("sensitive operation changed after it was reviewed".to_owned());
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn expire(&self, confirmation_id: &str) {
        if let Ok(mut pending) = self.pending.lock()
            && let Some(challenge) = pending.get_mut(confirmation_id)
        {
            challenge.expires_at = Instant::now() - Duration::from_secs(1);
        }
    }
}

fn validate_operation(
    operation: &SensitiveOperation,
    state: &DesktopUiState,
) -> Result<(), String> {
    match operation {
        SensitiveOperation::ApproveScript { reference }
        | SensitiveOperation::RunScript { reference }
        | SensitiveOperation::SetScriptAutomaticUpdateChecks { reference, .. } => {
            validate_text("script reference", reference, MAX_REFERENCE_BYTES, false)
        }
        SensitiveOperation::ImportScriptPackage { package_path }
        | SensitiveOperation::UpdateScriptPackage { package_path } => {
            validate_text("package path", package_path, MAX_PACKAGE_PATH_BYTES, false)
        }
        SensitiveOperation::InstallRemoteScriptPackage { review_id, sha256 } => {
            validate_text("review ID", review_id, 128, false)?;
            validate_text("package SHA256", sha256, 64, false)?;
            if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err("package SHA256 must contain 64 hexadecimal characters".to_owned());
            }
            Ok(())
        }
        SensitiveOperation::SaveRunnerConfig { contents, .. } => {
            if contents.len() > MAX_RUNNER_CONFIG_BYTES {
                return Err(format!(
                    "runner configuration exceeds {MAX_RUNNER_CONFIG_BYTES} bytes"
                ));
            }
            RunnerConfig::from_toml(contents, &state.config_path)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        SensitiveOperation::SaveRunnerConfigModel { config, .. } => {
            let contents = config.to_pretty_toml().map_err(|error| error.to_string())?;
            RunnerConfig::from_toml(&contents, &state.config_path)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        SensitiveOperation::RotateNetworkTriggerToken {
            reference, node_id, ..
        }
        | SensitiveOperation::SetNetworkTriggerAuthEnabled {
            reference, node_id, ..
        } => {
            validate_text("script reference", reference, MAX_REFERENCE_BYTES, false)?;
            validate_text("node ID", node_id, MAX_NODE_ID_BYTES, false)
        }
        SensitiveOperation::SetScriptSecret {
            reference,
            name,
            value,
        } => {
            validate_text("script reference", reference, MAX_REFERENCE_BYTES, false)?;
            validate_text("secret name", name, MAX_NAME_BYTES, false)?;
            if value.is_empty() || value.len() > MAX_SECRET_INPUT_BYTES {
                return Err(format!(
                    "secret value must contain between 1 and {MAX_SECRET_INPUT_BYTES} bytes"
                ));
            }
            Ok(())
        }
        SensitiveOperation::RemoveScriptSecret { reference, name } => {
            validate_text("script reference", reference, MAX_REFERENCE_BYTES, false)?;
            validate_text("secret name", name, MAX_NAME_BYTES, false)
        }
        SensitiveOperation::ResetRunnerConfig { .. }
        | SensitiveOperation::ClearRunHistory
        | SensitiveOperation::ClearRunLogs => Ok(()),
    }
}

fn validate_text(
    label: &str,
    value: &str,
    max_bytes: usize,
    allow_empty: bool,
) -> Result<(), String> {
    if (!allow_empty && value.is_empty()) || value.len() > max_bytes || value.contains('\0') {
        return Err(format!(
            "{label} must contain {} and at most {max_bytes} bytes without null characters",
            if allow_empty {
                "valid text"
            } else {
                "at least one byte"
            }
        ));
    }
    Ok(())
}

impl ConfirmationChallenge {
    pub(super) fn into_confirmation_id(self) -> String {
        self.confirmation_id
    }
}

pub(super) fn ensure_main_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), String> {
    ensure_main_window_source(window)?;
    #[cfg(not(test))]
    if !window
        .is_focused()
        .map_err(|error| format!("failed to verify the desktop window focus: {error}"))?
    {
        return Err("focus the BaudBound window before confirming this operation".to_owned());
    }
    Ok(())
}

pub(super) fn ensure_main_window_source<R: Runtime>(
    window: &WebviewWindow<R>,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("sensitive operations are only allowed from the main window".to_owned());
    }
    Ok(())
}

fn operation_digest(
    operation: &SensitiveOperation,
    state: &DesktopUiState,
) -> Result<[u8; 32], String> {
    let mut hasher = Sha256::new();
    let operation_json = serde_json::to_vec(operation)
        .map_err(|error| format!("failed to secure sensitive operation: {error}"))?;
    hasher.update(operation_json);
    match operation {
        SensitiveOperation::ApproveScript { reference }
        | SensitiveOperation::RunScript { reference }
        | SensitiveOperation::SetScriptAutomaticUpdateChecks { reference, .. } => {
            let installed = state
                .store
                .verify_script_package_hash(reference)
                .map_err(|error| error.to_string())?;
            hasher.update(installed.id.as_bytes());
            hasher.update(installed.package_hash.as_bytes());
        }
        SensitiveOperation::ImportScriptPackage { package_path }
        | SensitiveOperation::UpdateScriptPackage { package_path } => {
            hash_file(Path::new(package_path), &mut hasher)?;
        }
        _ => {}
    }
    Ok(hasher.finalize().into())
}

fn hash_file(path: &Path, hasher: &mut Sha256) -> Result<(), String> {
    let mut file = File::open(path)
        .map_err(|error| format!("failed to open package {}: {error}", path.display()))?;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to read package {}: {error}", path.display()))?;
        if count == 0 {
            return Ok(());
        }
        hasher.update(&buffer[..count]);
    }
}

fn random_confirmation_id() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("failed to generate sensitive operation confirmation: {error}"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
