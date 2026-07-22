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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StoredVariableChange {
    pub name: String,
    pub scope: String,
    pub script_id: Option<String>,
    pub updated_at_unix: u64,
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum NetworkTriggerType {
    Webhook,
    Websocket,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct NetworkTriggerDefinition {
    pub node_id: String,
    pub trigger_type: NetworkTriggerType,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct TriggerAuthStatus {
    pub auth_enabled: bool,
    pub created_at_unix: u64,
    pub disabled_at_unix: Option<u64>,
    pub node_id: String,
    pub rotated_at_unix: Option<u64>,
    pub script_id: String,
    pub token_preview: String,
    pub trigger_type: NetworkTriggerType,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct GeneratedTriggerToken {
    pub status: TriggerAuthStatus,
    pub token: String,
}

#[derive(Debug, Clone)]
pub struct ScriptApprovalResult {
    pub approval: ScriptApproval,
    pub generated_trigger_tokens: Vec<GeneratedTriggerToken>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerAuthentication {
    Authenticated,
    Disabled,
    InvalidToken,
    MissingToken,
}

#[derive(Debug, Clone)]
pub struct ApproveScriptRequest {
    pub approved_permissions: Vec<String>,
    pub network_triggers: Vec<NetworkTriggerDefinition>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_type: Option<String>,
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
    pub variable_scopes: BTreeMap<String, String>,
    #[serde(default)]
    pub variables: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaginatedRecords<T> {
    pub items: Vec<T>,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Ascending,
    #[default]
    Descending,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunHistorySort {
    #[default]
    Completed,
    RecentLog,
    RunId,
    Script,
    Status,
    Trigger,
    TriggerType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunHistoryQuery {
    pub direction: SortDirection,
    pub limit: usize,
    pub offset: usize,
    pub script_id: Option<String>,
    pub search: String,
    pub sort: RunHistorySort,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunLogSort {
    Level,
    Message,
    Node,
    Run,
    Script,
    #[default]
    Time,
    Type,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunLogQuery {
    pub direction: SortDirection,
    pub limit: usize,
    pub offset: usize,
    pub search: String,
    pub sort: RunLogSort,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StoredRunLogRecord {
    pub action_type: Option<String>,
    pub level: String,
    pub log_index: usize,
    pub message: String,
    pub node_id: Option<String>,
    pub run_id: String,
    pub script_id: String,
    pub script_name: String,
    pub timestamp_unix_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StoredVariableRecord {
    pub name: String,
    pub scope: String,
    pub script_id: Option<String>,
    pub script_name: Option<String>,
    pub updated_at_unix: u64,
    pub value: serde_json::Value,
    pub version: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct RunStatistics {
    pub cancelled: usize,
    pub completed: usize,
    pub failed: usize,
    pub total: usize,
    pub with_errors: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct UpdateCheckCache {
    pub checked_at_unix: u64,
    pub latest_version: String,
    pub published_at: Option<String>,
    pub release_notes: Option<String>,
    pub update_available: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScriptUpdateState {
    pub automatic_checks_enabled: bool,
    pub checked_update_url: Option<String>,
    pub last_checked_at_unix: Option<u64>,
    pub last_error: Option<String>,
    pub last_success_at_unix: Option<u64>,
    pub latest_version: Option<String>,
    pub package_sha256: Option<String>,
    pub package_size: Option<u64>,
    pub package_url: Option<String>,
    pub published_at: Option<String>,
    pub release_notes: Option<String>,
    pub script_id: String,
}

impl ScriptUpdateState {
    #[must_use]
    pub fn empty(script_id: impl Into<String>) -> Self {
        Self {
            automatic_checks_enabled: false,
            checked_update_url: None,
            last_checked_at_unix: None,
            last_error: None,
            last_success_at_unix: None,
            latest_version: None,
            package_sha256: None,
            package_size: None,
            package_url: None,
            published_at: None,
            release_notes: None,
            script_id: script_id.into(),
        }
    }
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
    #[error(
        "network trigger auth state was not found for {script_id}:{node_id} ({trigger_type:?})"
    )]
    TriggerAuthNotFound {
        script_id: String,
        node_id: String,
        trigger_type: NetworkTriggerType,
    },
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
    fn approve_script(
        &self,
        request: ApproveScriptRequest,
    ) -> Result<ScriptApprovalResult, StorageError>;
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
    fn run_statistics(&self) -> Result<RunStatistics, StorageError>;
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
    fn list_trigger_auth_statuses(
        &self,
        script_reference: &str,
    ) -> Result<Vec<TriggerAuthStatus>, StorageError>;
    fn rotate_trigger_auth_token(
        &self,
        script_reference: &str,
        node_id: &str,
        trigger_type: NetworkTriggerType,
    ) -> Result<GeneratedTriggerToken, StorageError>;
    fn set_trigger_auth_enabled(
        &self,
        script_reference: &str,
        node_id: &str,
        trigger_type: NetworkTriggerType,
        enabled: bool,
    ) -> Result<TriggerAuthStatus, StorageError>;
    fn authenticate_trigger(
        &self,
        script_id: &str,
        node_id: &str,
        trigger_type: NetworkTriggerType,
        provided_token: Option<&str>,
    ) -> Result<TriggerAuthentication, StorageError>;
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
