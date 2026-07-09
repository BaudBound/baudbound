//! Storage for installed runner scripts and run logs.

mod storage;

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use crate::storage::filesystem::{
    copy_file, create_dir_all, current_unix_timestamp, package_file_name_from_path,
    remove_file_inside_root, sha256_file, validate_package_file_name, validate_script_id,
    write_atomic,
};
use crate::storage::metadata::StorageIndex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use storage::service_control::{ConsumedServiceControl, ServiceControlCommand};
pub use storage::sqlite::{CURRENT_SCHEMA_VERSION, SqliteRunnerStore};

const INDEX_FILE_NAME: &str = "index.json";
const APPROVALS_FILE_NAME: &str = "approvals.json";
const RUN_HISTORY_FILE_NAME: &str = "runs.jsonl";
const SERVICE_CONTROL_FILE_NAME: &str = ".service-control.json";
const SERVICE_STATUS_FILE_NAME: &str = "service-status.json";
const TRIGGER_RELOAD_SIGNAL_FILE_NAME: &str = ".trigger-reload";
const SCRIPTS_DIR_NAME: &str = "scripts";
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
    #[error("storage backend is not configured")]
    NotConfigured,
    #[error("storage path {path} is outside runner storage root {root}")]
    PathOutsideRoot { path: PathBuf, root: PathBuf },
    #[error("storage operation failed: {0}")]
    Operation(String),
    #[error("storage I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("storage metadata JSON is invalid in {path}: {source}")]
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
}

#[derive(Debug, Clone)]
pub struct FilesystemScriptStore {
    root: PathBuf,
}

impl FilesystemScriptStore {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn ensure_layout(&self) -> Result<(), StorageError> {
        create_dir_all(self.scripts_dir())
    }

    fn index_path(&self) -> PathBuf {
        self.root.join(INDEX_FILE_NAME)
    }

    fn approvals_path(&self) -> PathBuf {
        self.root.join(APPROVALS_FILE_NAME)
    }

    fn run_history_path(&self) -> PathBuf {
        self.root.join(RUN_HISTORY_FILE_NAME)
    }

    fn service_status_path(&self) -> PathBuf {
        self.root.join(SERVICE_STATUS_FILE_NAME)
    }

    fn service_control_path(&self) -> PathBuf {
        self.root.join(SERVICE_CONTROL_FILE_NAME)
    }

    fn trigger_reload_signal_path(&self) -> PathBuf {
        self.root.join(TRIGGER_RELOAD_SIGNAL_FILE_NAME)
    }

    fn scripts_dir(&self) -> PathBuf {
        self.root.join(SCRIPTS_DIR_NAME)
    }

    fn package_path(&self, package_file_name: &str) -> Result<PathBuf, StorageError> {
        validate_package_file_name(package_file_name)?;
        Ok(self.scripts_dir().join(package_file_name))
    }

    fn read_index(&self) -> Result<StorageIndex, StorageError> {
        let index_path = self.index_path();
        match fs::read_to_string(&index_path) {
            Ok(content) => serde_json::from_str(&content).map_err(|source| StorageError::Json {
                path: index_path,
                source,
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(StorageIndex::default()),
            Err(source) => Err(StorageError::Io {
                path: index_path,
                source,
            }),
        }
    }

    fn write_index(&self, index: &StorageIndex) -> Result<(), StorageError> {
        self.ensure_layout()?;
        let index_path = self.index_path();
        let content = serde_json::to_string_pretty(index).map_err(|source| StorageError::Json {
            path: index_path.clone(),
            source,
        })?;
        write_atomic(&index_path, content.as_bytes())
    }

    pub fn request_trigger_reload(&self) -> Result<(), StorageError> {
        self.ensure_layout()?;
        let path = self.trigger_reload_signal_path();
        write_atomic(&path, current_unix_timestamp().to_string().as_bytes())
    }

    pub fn consume_trigger_reload_request(&self) -> Result<bool, StorageError> {
        let path = self.trigger_reload_signal_path();
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(source) => Err(StorageError::Io { path, source }),
        }
    }

    fn resolve_reference(
        &self,
        index: &StorageIndex,
        reference: &str,
    ) -> Result<InstalledScript, StorageError> {
        if let Some(script) = index.scripts.get(reference) {
            return Ok(script.clone());
        }

        let normalized_reference = reference.to_lowercase();
        let mut matches = index
            .scripts
            .values()
            .filter(|script| script.name.to_lowercase() == normalized_reference)
            .cloned()
            .collect::<Vec<_>>();

        match matches.len() {
            0 => Err(StorageError::NotFound(reference.to_owned())),
            1 => Ok(matches.remove(0)),
            _ => Err(StorageError::Operation(format!(
                "{reference:?} matches multiple scripts; use the script id"
            ))),
        }
    }

    fn ensure_package_file_available(
        &self,
        index: &StorageIndex,
        package_file_name: &str,
        allowed_script_id: Option<&str>,
    ) -> Result<(), StorageError> {
        if let Some(script) = index.scripts.values().find(|script| {
            script.package_file_name == package_file_name
                && Some(script.id.as_str()) != allowed_script_id
        }) {
            return Err(StorageError::PackageFileNameInUse {
                file_name: package_file_name.to_owned(),
                script_id: script.id.clone(),
            });
        }

        Ok(())
    }

    fn install_package(
        &self,
        mut index: StorageIndex,
        request: ImportScriptRequest,
        existing: Option<InstalledScript>,
    ) -> Result<InstalledScript, StorageError> {
        validate_script_id(&request.id)?;
        self.ensure_layout()?;

        let package_file_name = package_file_name_from_path(&request.package_source)?;
        self.ensure_package_file_available(&index, &package_file_name, Some(&request.id))?;

        let package_hash = sha256_file(&request.package_source)?;
        let package_path = self.package_path(&package_file_name)?;
        copy_file(&request.package_source, &package_path)?;

        let imported_at_unix = existing
            .as_ref()
            .map(|script| script.imported_at_unix)
            .unwrap_or_else(current_unix_timestamp);
        let enabled = existing.as_ref().is_none_or(|script| script.enabled);
        let previous_package_path = existing.as_ref().map(|script| script.package_path.clone());

        let installed = InstalledScript {
            id: request.id,
            enabled,
            name: request.name,
            package_hash,
            package_file_name,
            package_path,
            imported_at_unix,
            package_format_version: request.package_format_version,
            script_language_version: request.script_language_version,
            target_runtime: request.target_runtime,
            asset_count: request.asset_count,
            risk_level: request.risk_level,
        };

        index
            .scripts
            .insert(installed.id.clone(), installed.clone());
        self.write_index(&index)?;
        self.request_trigger_reload()?;

        if let Some(previous_package_path) = previous_package_path
            && previous_package_path != installed.package_path
        {
            remove_file_inside_root(&self.scripts_dir(), &previous_package_path)?;
        }

        Ok(installed)
    }
}

impl ScriptStore for FilesystemScriptStore {
    fn append_run_record(&self, record: StoredRunRecord) -> Result<(), StorageError> {
        storage::runs::append_run_record(self, record)
    }

    fn approve_script(
        &self,
        request: ApproveScriptRequest,
    ) -> Result<ScriptApproval, StorageError> {
        storage::approvals::approve_script(self, request)
    }

    fn find_script_approval(
        &self,
        script_reference: &str,
    ) -> Result<Option<ScriptApproval>, StorageError> {
        storage::approvals::find_script_approval(self, script_reference)
    }

    fn import_script(&self, request: ImportScriptRequest) -> Result<InstalledScript, StorageError> {
        let index = self.read_index()?;
        if index.scripts.contains_key(&request.id) {
            return Err(StorageError::AlreadyInstalled(request.id));
        }
        self.install_package(index, request, None)
    }

    fn update_script(&self, request: ImportScriptRequest) -> Result<InstalledScript, StorageError> {
        let index = self.read_index()?;
        let existing = index
            .scripts
            .get(&request.id)
            .cloned()
            .ok_or_else(|| StorageError::NotFound(request.id.clone()))?;
        let installed = self.install_package(index, request, Some(existing))?;
        self.revoke_script_approval(&installed.id)?;
        Ok(installed)
    }

    fn list_scripts(&self) -> Result<Vec<InstalledScript>, StorageError> {
        let index = self.read_index()?;
        Ok(index.scripts.into_values().collect())
    }

    fn list_run_records(
        &self,
        script_reference: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<StoredRunRecord>, StorageError> {
        storage::runs::list_run_records(self, script_reference, limit)
    }

    fn remove_script(&self, reference: &str) -> Result<InstalledScript, StorageError> {
        let mut index = self.read_index()?;
        let installed = self.resolve_reference(&index, reference)?;
        index.scripts.remove(&installed.id);
        self.write_index(&index)?;
        self.request_trigger_reload()?;
        storage::approvals::revoke_script_approval_by_id(self, &installed.id)?;

        remove_file_inside_root(&self.scripts_dir(), &installed.package_path)?;

        Ok(installed)
    }

    fn revoke_script_approval(
        &self,
        script_reference: &str,
    ) -> Result<Option<ScriptApproval>, StorageError> {
        storage::approvals::revoke_script_approval(self, script_reference)
    }

    fn set_script_enabled(
        &self,
        reference: &str,
        enabled: bool,
    ) -> Result<InstalledScript, StorageError> {
        let mut index = self.read_index()?;
        let mut installed = self.resolve_reference(&index, reference)?;
        installed.enabled = enabled;
        index
            .scripts
            .insert(installed.id.clone(), installed.clone());
        self.write_index(&index)?;
        self.request_trigger_reload()?;
        Ok(installed)
    }

    fn find_script(&self, reference: &str) -> Result<InstalledScript, StorageError> {
        let index = self.read_index()?;
        self.resolve_reference(&index, reference)
    }

    fn verify_script_package_hash(&self, reference: &str) -> Result<InstalledScript, StorageError> {
        let installed = self.find_script(reference)?;
        let actual = sha256_file(&installed.package_path)?;
        if actual != installed.package_hash {
            return Err(StorageError::HashMismatch {
                script_id: installed.id,
                expected: installed.package_hash,
                actual,
            });
        }
        Ok(installed)
    }
}

#[cfg(test)]
mod tests;
