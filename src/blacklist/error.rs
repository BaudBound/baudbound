use baudbound_security::BlacklistSeverity;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum BlacklistError {
    #[error(transparent)]
    BackendApi(#[from] crate::backend_api::BackendApiError),
    #[error("the Official blacklist response is invalid: {0}")]
    InvalidResponse(String),
    #[error("the repository URL is invalid")]
    InvalidRepositoryUrl,
    #[error("blacklist storage failed: {0}")]
    Io(std::io::Error),
    #[error("blacklist JSON is invalid: {0}")]
    Json(serde_json::Error),
    #[error("the Official blacklist contains too many entries")]
    TooManyEntries,
    #[error("the blacklist state file is too large")]
    StateTooLarge,
    #[error("content is restricted by the Official blacklist ({severity:?}): {titles}")]
    Restricted {
        severity: BlacklistSeverity,
        titles: String,
    },
    #[error("blacklist enforcement storage failed: {0}")]
    Storage(baudbound_storage::StorageError),
}
