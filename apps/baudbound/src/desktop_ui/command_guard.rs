use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::Path,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use baudbound_core::RunnerConfig;
use baudbound_storage::{NetworkTriggerType, ScriptStore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{Runtime, WebviewWindow};

use super::DesktopUiState;

const CHALLENGE_LIFETIME: Duration = Duration::from_secs(120);
const MAX_PENDING_CHALLENGES: usize = 128;

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
    digest: [u8; 32],
    expires_at: Instant,
    operation_kind: &'static str,
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

pub(super) fn ensure_main_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), String> {
    if window.label() != "main" {
        return Err("sensitive operations are only allowed from the main window".to_owned());
    }
    #[cfg(not(test))]
    if !window
        .is_focused()
        .map_err(|error| format!("failed to verify the desktop window focus: {error}"))?
    {
        return Err("focus the BaudBound window before confirming this operation".to_owned());
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
        | SensitiveOperation::RunScript { reference } => {
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
