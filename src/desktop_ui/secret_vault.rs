use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use anyhow::{Result, anyhow};
use baudbound_storage::SqliteRunnerStore;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};

pub(super) const SECRET_VAULT_EVENT_CHANNEL: &str = "runner-secret-vault";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SecretStorageMode {
    OperatingSystem,
    Password,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SecretVaultStatus {
    Initializing,
    Available,
    Locked,
    Unavailable,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct SecretVaultSnapshot {
    pub(super) error: Option<String>,
    pub(super) mode: SecretStorageMode,
    pub(super) status: SecretVaultStatus,
}

#[derive(Clone)]
pub(super) struct SecretVaultController {
    inner: Arc<Mutex<SecretVaultState>>,
    runner_home: PathBuf,
}

struct SecretVaultState {
    attempt_active: bool,
    snapshot: SecretVaultSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StartResult {
    AlreadyAvailable,
    AlreadyInitializing,
    PasswordLocked,
    Started,
}

impl SecretVaultController {
    pub(super) fn new(runner_home: PathBuf) -> Self {
        let mode = match crate::secrets::desktop_secret_storage_mode(&runner_home) {
            crate::secrets::DesktopSecretStorageMode::OperatingSystem => {
                SecretStorageMode::OperatingSystem
            }
            crate::secrets::DesktopSecretStorageMode::Password => SecretStorageMode::Password,
        };
        Self {
            inner: Arc::new(Mutex::new(SecretVaultState {
                attempt_active: false,
                snapshot: SecretVaultSnapshot {
                    error: None,
                    mode,
                    status: if mode == SecretStorageMode::Password {
                        SecretVaultStatus::Locked
                    } else {
                        SecretVaultStatus::Initializing
                    },
                },
            })),
            runner_home,
        }
    }

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
        if self.snapshot().mode == SecretStorageMode::Password {
            store.clear_secret_cipher();
            self.finish_attempt(SecretVaultSnapshot {
                error: None,
                mode: SecretStorageMode::Password,
                status: SecretVaultStatus::Locked,
            });
            return StartResult::PasswordLocked;
        }

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
                            mode: SecretStorageMode::OperatingSystem,
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
                            mode: SecretStorageMode::OperatingSystem,
                            status: SecretVaultStatus::Unavailable,
                        }
                    }
                };
                worker_state.finish_attempt(snapshot.clone());
                publish(&app, snapshot.clone());
                if snapshot.status == SecretVaultStatus::Available {
                    let state = app.state::<super::DesktopUiState>();
                    match super::desktop_config::start_deferred_background_runner(&state) {
                        Ok(Some(message)) => tracing::info!(%message),
                        Ok(None) => {}
                        Err(error) => tracing::warn!(
                            %error,
                            "failed to start the background runner after secret storage became available"
                        ),
                    }
                }
            })
        {
            Ok(_) => StartResult::Started,
            Err(error) => {
                let message = format!("failed to start credential-vault worker: {error}");
                spawn_failure_state.finish_attempt(SecretVaultSnapshot {
                    error: Some(message.clone()),
                    mode: SecretStorageMode::OperatingSystem,
                    status: SecretVaultStatus::Unavailable,
                });
                tracing::warn!(error = %message, "encrypted secret storage is unavailable");
                StartResult::Started
            }
        }
    }

    pub(super) fn unlock_password(&self, password: &str, store: &SqliteRunnerStore) -> Result<()> {
        if self.snapshot().mode != SecretStorageMode::Password {
            return Err(anyhow!("password protected storage is not enabled"));
        }
        let cipher = crate::secrets::unlock_password_secret_cipher(&self.runner_home, password)?;
        store.set_secret_cipher(cipher);
        self.finish_attempt(SecretVaultSnapshot {
            error: None,
            mode: SecretStorageMode::Password,
            status: SecretVaultStatus::Available,
        });
        Ok(())
    }

    pub(super) fn lock_password(&self, store: &SqliteRunnerStore) -> Result<()> {
        if self.snapshot().mode != SecretStorageMode::Password {
            return Err(anyhow!("password protected storage is not enabled"));
        }
        store.clear_secret_cipher();
        self.finish_attempt(SecretVaultSnapshot {
            error: None,
            mode: SecretStorageMode::Password,
            status: SecretVaultStatus::Locked,
        });
        Ok(())
    }

    pub(super) fn switch(
        &self,
        mode: SecretStorageMode,
        password: Option<&str>,
        store: &SqliteRunnerStore,
    ) -> Result<usize> {
        self.begin_transition(mode)?;
        let result = (|| match mode {
            SecretStorageMode::Password => {
                let password = password
                    .ok_or_else(|| anyhow!("a password is required for protected storage"))?;
                let cipher =
                    crate::secrets::prepare_password_secret_cipher(&self.runner_home, password)?;
                let cleared = store.clear_all_stored_secrets().inspect_err(|_| {
                    crate::secrets::discard_pending_password_secret_storage(&self.runner_home);
                })?;
                if let Err(error) =
                    crate::secrets::commit_password_secret_storage(&self.runner_home)
                {
                    crate::secrets::discard_pending_password_secret_storage(&self.runner_home);
                    return Err(error);
                }
                store.set_secret_cipher(cipher);
                self.finish_attempt(SecretVaultSnapshot {
                    error: None,
                    mode,
                    status: SecretVaultStatus::Available,
                });
                Ok(cleared)
            }
            SecretStorageMode::OperatingSystem => {
                let cipher = crate::secrets::reset_desktop_secret_cipher()?;
                let cleared = store.clear_all_stored_secrets()?;
                crate::secrets::remove_password_secret_storage(&self.runner_home)?;
                store.set_secret_cipher(cipher);
                self.finish_attempt(SecretVaultSnapshot {
                    error: None,
                    mode,
                    status: SecretVaultStatus::Available,
                });
                Ok(cleared)
            }
        })();
        if result.is_err() {
            self.cancel_attempt();
        }
        result
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
        if state.snapshot.mode == SecretStorageMode::Password {
            return StartResult::PasswordLocked;
        }
        state.attempt_active = true;
        state.snapshot = SecretVaultSnapshot {
            error: None,
            mode: SecretStorageMode::OperatingSystem,
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

    fn begin_transition(&self, mode: SecretStorageMode) -> Result<()> {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.attempt_active {
            return Err(anyhow!(
                "wait for the current secret storage operation to finish"
            ));
        }
        if state.snapshot.mode == mode {
            return Err(anyhow!(
                "the selected secret storage mode is already active"
            ));
        }
        state.attempt_active = true;
        Ok(())
    }

    fn cancel_attempt(&self) {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .attempt_active = false;
    }
}

pub(super) fn publish<R: Runtime>(app: &AppHandle<R>, snapshot: SecretVaultSnapshot) {
    if let Err(error) = app.emit(SECRET_VAULT_EVENT_CHANNEL, snapshot) {
        tracing::warn!(%error, "failed to publish secret-vault status");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_storage_starts_locked() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        std::fs::write(
            crate::secrets::password_secret_key_path(directory.path()),
            b"{}",
        )
        .expect("marker should be written");
        let controller = SecretVaultController::new(directory.path().to_path_buf());

        assert_eq!(controller.snapshot().mode, SecretStorageMode::Password);
        assert_eq!(controller.snapshot().status, SecretVaultStatus::Locked);
    }

    #[test]
    fn prevents_overlapping_operating_system_vault_attempts() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let controller = SecretVaultController::new(directory.path().to_path_buf());
        assert_eq!(controller.begin_attempt(), StartResult::Started);
        assert_eq!(controller.begin_attempt(), StartResult::AlreadyInitializing);

        controller.finish_attempt(SecretVaultSnapshot {
            error: Some("unavailable".to_owned()),
            mode: SecretStorageMode::OperatingSystem,
            status: SecretVaultStatus::Unavailable,
        });
        assert_eq!(controller.begin_attempt(), StartResult::Started);
    }

    #[test]
    fn failed_switch_releases_the_transition_lock() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let controller = SecretVaultController::new(directory.path().to_path_buf());
        let store = SqliteRunnerStore::open(directory.path().join("runner.sqlite3"))
            .expect("store should open");

        controller
            .switch(SecretStorageMode::Password, None, &store)
            .expect_err("missing password should reject the switch");
        assert!(
            controller
                .begin_transition(SecretStorageMode::Password)
                .is_ok()
        );
    }
}
