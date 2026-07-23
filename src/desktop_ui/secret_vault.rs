use std::{
    sync::{Arc, Mutex},
    thread,
};

use baudbound_storage::SqliteRunnerStore;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

pub(super) const SECRET_VAULT_EVENT_CHANNEL: &str = "runner-secret-vault";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SecretVaultStatus {
    Initializing,
    Available,
    Unavailable,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct SecretVaultSnapshot {
    pub(super) error: Option<String>,
    pub(super) status: SecretVaultStatus,
}

#[derive(Clone)]
pub(super) struct SecretVaultController {
    inner: Arc<Mutex<SecretVaultState>>,
}

struct SecretVaultState {
    attempt_active: bool,
    snapshot: SecretVaultSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StartResult {
    AlreadyAvailable,
    AlreadyInitializing,
    Started,
}

impl Default for SecretVaultController {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SecretVaultState {
                attempt_active: false,
                snapshot: SecretVaultSnapshot {
                    error: None,
                    status: SecretVaultStatus::Initializing,
                },
            })),
        }
    }
}

impl SecretVaultController {
    pub(super) fn snapshot(&self) -> SecretVaultSnapshot {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .snapshot
            .clone()
    }

    pub(super) fn start<R: Runtime>(
        &self,
        app: AppHandle<R>,
        store: SqliteRunnerStore,
    ) -> StartResult {
        let result = self.begin_attempt();
        if result != StartResult::Started {
            return result;
        }

        let worker_state = self.clone();
        let spawn_failure_state = self.clone();
        match thread::Builder::new()
            .name("baudbound-secret-vault".to_owned())
            .spawn(move || {
                let snapshot = match crate::secrets::desktop_secret_cipher() {
                    Ok(cipher) => {
                        store.set_secret_cipher(cipher);
                        SecretVaultSnapshot {
                            error: None,
                            status: SecretVaultStatus::Available,
                        }
                    }
                    Err(error) => {
                        let error = format!("{error:#}");
                        tracing::warn!(
                            error = %error,
                            "encrypted secret storage is unavailable; continuing without secret access"
                        );
                        SecretVaultSnapshot {
                            error: Some(error),
                            status: SecretVaultStatus::Unavailable,
                        }
                    }
                };
                worker_state.finish_attempt(snapshot.clone());
                if let Err(error) = app.emit(SECRET_VAULT_EVENT_CHANNEL, snapshot) {
                    tracing::warn!(%error, "failed to publish secret-vault status");
                }
            })
        {
            Ok(_) => StartResult::Started,
            Err(error) => {
                let message = format!("failed to start credential-vault worker: {error}");
                spawn_failure_state.finish_attempt(SecretVaultSnapshot {
                    error: Some(message.clone()),
                    status: SecretVaultStatus::Unavailable,
                });
                tracing::warn!(error = %message, "encrypted secret storage is unavailable");
                StartResult::Started
            }
        }
    }

    fn begin_attempt(&self) -> StartResult {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.attempt_active {
            return StartResult::AlreadyInitializing;
        }
        if state.snapshot.status == SecretVaultStatus::Available {
            return StartResult::AlreadyAvailable;
        }
        state.attempt_active = true;
        state.snapshot = SecretVaultSnapshot {
            error: None,
            status: SecretVaultStatus::Initializing,
        };
        StartResult::Started
    }

    fn finish_attempt(&self, snapshot: SecretVaultSnapshot) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.attempt_active = false;
        state.snapshot = snapshot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prevents_overlapping_attempts_and_allows_retry_after_failure() {
        let controller = SecretVaultController::default();
        assert_eq!(controller.begin_attempt(), StartResult::Started);
        assert_eq!(controller.begin_attempt(), StartResult::AlreadyInitializing);

        controller.finish_attempt(SecretVaultSnapshot {
            error: Some("unavailable".to_owned()),
            status: SecretVaultStatus::Unavailable,
        });
        assert_eq!(controller.begin_attempt(), StartResult::Started);
    }

    #[test]
    fn does_not_replace_an_available_vault() {
        let controller = SecretVaultController::default();
        assert_eq!(controller.begin_attempt(), StartResult::Started);
        controller.finish_attempt(SecretVaultSnapshot {
            error: None,
            status: SecretVaultStatus::Available,
        });

        assert_eq!(controller.begin_attempt(), StartResult::AlreadyAvailable);
    }
}
