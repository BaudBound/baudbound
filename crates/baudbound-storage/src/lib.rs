//! SQLite-backed durable storage for the BaudBound runner.

mod storage;

use std::{collections::BTreeMap, io, path::PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use storage::{
    secrets::SecretCipher,
    sqlite::{CURRENT_SCHEMA_VERSION, RunRetentionPolicy, SqliteRunnerStore},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredVariableScope {
    Persistent,
    Global,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredVariable {
    pub value: serde_json::Value,
    pub version: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SecretStatus {
    pub configured: bool,
    pub name: String,
    pub updated_at_unix: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstalledScript {
    pub id: String,
    pub enabled: bool,
    pub name: String,
    pub package_hash: String,
    #[serde(default)]
    pub package_file_name: String,
    pub package_path: PathBuf,
    pub imported_at_unix: u64,
    pub package_format_version: u32,
    pub script_language_version: u32,
    pub target_runtime: String,
    pub asset_count: usize,
    pub risk_level: String,
}

#[derive(Debug, Clone)]
pub struct ImportScriptRequest {
    pub id: String,
    pub name: String,
    pub package_source: PathBuf,
    pub package_format_version: u32,
    pub script_language_version: u32,
    pub target_runtime: String,
    pub asset_count: usize,
    pub risk_level: String,
}

#[derive(Debug, Clone)]
pub struct ApproveScriptRequest {
    pub approved_permissions: Vec<String>,
    pub package_hash: String,
    pub script_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScriptApproval {
    pub approved_at_unix: u64,
    pub approved_permissions: Vec<String>,
    pub package_hash: String,
    pub script_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunLogEntry {
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub timestamp_unix_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StoredRunRecord {
    pub completed_at_unix: u64,
    pub logs: Vec<RunLogEntry>,
    pub run_id: String,
    pub script_id: String,
    pub status: String,
    pub trigger_node_id: String,
    #[serde(default)]
    pub variables: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct UpdateCheckCache {
    pub checked_at_unix: u64,
    pub latest_version: String,
    pub published_at: Option<String>,
    pub release_notes: Option<String>,
    pub update_available: bool,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("script {0} is already installed")]
    AlreadyInstalled(String),
    #[error("package file name {file_name:?} is already used by script {script_id}")]
    PackageFileNameInUse {
        file_name: String,
        script_id: String,
    },
    #[error(
        "installed package hash mismatch for script {script_id}: expected {expected}, got {actual}"
    )]
    HashMismatch {
        script_id: String,
        expected: String,
        actual: String,
    },
    #[error("invalid package file name {0:?}")]
    InvalidPackageFileName(String),
    #[error("script {0} is not installed")]
    NotFound(String),
    #[error("invalid script id {0:?}")]
    InvalidScriptId(String),
    #[error("storage path {path} is outside runner storage root {root}")]
    PathOutsideRoot { path: PathBuf, root: PathBuf },
    #[error("storage operation failed: {0}")]
    Operation(String),
    #[error("secret vault key is unavailable")]
    SecretKeyUnavailable,
    #[error("secret encryption failed: {0}")]
    SecretCrypto(String),
    #[error("storage I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("storage JSON is invalid in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("SQLite storage failed for {path}: {source}")]
    Sqlite {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
}

pub trait ScriptStore: Send + Sync {
    fn append_run_record(&self, record: StoredRunRecord) -> Result<(), StorageError>;
    fn approve_script(&self, request: ApproveScriptRequest)
    -> Result<ScriptApproval, StorageError>;
    fn find_script_approval(
        &self,
        script_reference: &str,
    ) -> Result<Option<ScriptApproval>, StorageError>;
    fn import_script(&self, request: ImportScriptRequest) -> Result<InstalledScript, StorageError>;
    fn update_script(&self, request: ImportScriptRequest) -> Result<InstalledScript, StorageError>;
    fn list_scripts(&self) -> Result<Vec<InstalledScript>, StorageError>;
    fn list_run_records(
        &self,
        script_reference: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<StoredRunRecord>, StorageError>;
    fn remove_script(&self, reference: &str) -> Result<InstalledScript, StorageError>;
    fn revoke_script_approval(
        &self,
        script_reference: &str,
    ) -> Result<Option<ScriptApproval>, StorageError>;
    fn set_script_enabled(
        &self,
        reference: &str,
        enabled: bool,
    ) -> Result<InstalledScript, StorageError>;
    fn find_script(&self, reference: &str) -> Result<InstalledScript, StorageError>;
    fn verify_script_package_hash(&self, reference: &str) -> Result<InstalledScript, StorageError>;
    fn load_variable(
        &self,
        scope: StoredVariableScope,
        script_id: &str,
        name: &str,
    ) -> Result<Option<StoredVariable>, StorageError>;
    fn compare_and_set_variable(
        &self,
        scope: StoredVariableScope,
        script_id: &str,
        name: &str,
        expected_version: Option<u64>,
        value: &serde_json::Value,
    ) -> Result<bool, StorageError>;
    fn list_secret_statuses(
        &self,
        script_reference: &str,
    ) -> Result<Vec<SecretStatus>, StorageError>;
    fn read_secret(
        &self,
        script_id: &str,
        name: &str,
    ) -> Result<Option<serde_json::Value>, StorageError>;
    fn set_secret(
        &self,
        script_reference: &str,
        name: &str,
        value: &serde_json::Value,
    ) -> Result<SecretStatus, StorageError>;
    fn remove_secret(&self, script_reference: &str, name: &str) -> Result<bool, StorageError>;
}

#[cfg(test)]
mod tests;
