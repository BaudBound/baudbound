use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum BackendApiError {
    #[error("the BaudBound API URL is invalid")]
    InvalidBaseUrl,
    #[error("the BaudBound API collection name is invalid")]
    InvalidCollection,
    #[error("the BaudBound API client could not be created: {0}")]
    ClientBuild(reqwest::Error),
    #[error("the BaudBound API request failed: {0}")]
    Request(reqwest::Error),
    #[error("the BaudBound API returned HTTP {0}")]
    Status(u16),
    #[error("the BaudBound API response is invalid: {0}")]
    InvalidResponse(String),
    #[error("the BaudBound API response contains too many records")]
    TooManyRecords,
    #[error("the BaudBound API response is too large")]
    ResponseTooLarge,
    #[error("the BaudBound API response could not be read: {0}")]
    Io(std::io::Error),
    #[error("the BaudBound API response contains invalid JSON: {0}")]
    Json(serde_json::Error),
}
