//! Storage for installed runner scripts and run logs.

use std::{
    collections::BTreeMap,
    fs,
    io::{self, BufRead, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const INDEX_FILE_NAME: &str = "index.json";
const APPROVALS_FILE_NAME: &str = "approvals.json";
const RUN_HISTORY_FILE_NAME: &str = "runs.jsonl";
const SCRIPTS_DIR_NAME: &str = "scripts";
const STORAGE_FORMAT_VERSION: u32 = 1;

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

    fn read_approvals(&self) -> Result<ApprovalIndex, StorageError> {
        let approvals_path = self.approvals_path();
        match fs::read_to_string(&approvals_path) {
            Ok(content) => serde_json::from_str(&content).map_err(|source| StorageError::Json {
                path: approvals_path,
                source,
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ApprovalIndex::default()),
            Err(source) => Err(StorageError::Io {
                path: approvals_path,
                source,
            }),
        }
    }

    fn write_approvals(&self, approvals: &ApprovalIndex) -> Result<(), StorageError> {
        self.ensure_layout()?;
        let approvals_path = self.approvals_path();
        let content =
            serde_json::to_string_pretty(approvals).map_err(|source| StorageError::Json {
                path: approvals_path.clone(),
                source,
            })?;
        write_atomic(&approvals_path, content.as_bytes())
    }

    fn revoke_script_approval_by_id(
        &self,
        script_id: &str,
    ) -> Result<Option<ScriptApproval>, StorageError> {
        validate_script_id(script_id)?;
        let mut approvals = self.read_approvals()?;
        let removed = approvals.approvals.remove(script_id);
        self.write_approvals(&approvals)?;
        Ok(removed)
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
        self.ensure_layout()?;
        let path = self.run_history_path();
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|source| StorageError::Io {
                path: path.clone(),
                source,
            })?;
        let line = serde_json::to_string(&record).map_err(|source| StorageError::Json {
            path: path.clone(),
            source,
        })?;
        file.write_all(line.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .map_err(|source| StorageError::Io { path, source })
    }

    fn approve_script(
        &self,
        request: ApproveScriptRequest,
    ) -> Result<ScriptApproval, StorageError> {
        validate_script_id(&request.script_id)?;
        let mut approvals = self.read_approvals()?;
        let approval = ScriptApproval {
            approved_at_unix: current_unix_timestamp(),
            approved_permissions: request.approved_permissions,
            package_hash: request.package_hash,
            script_id: request.script_id,
        };
        approvals
            .approvals
            .insert(approval.script_id.clone(), approval.clone());
        self.write_approvals(&approvals)?;
        Ok(approval)
    }

    fn find_script_approval(
        &self,
        script_reference: &str,
    ) -> Result<Option<ScriptApproval>, StorageError> {
        let installed = self.find_script(script_reference)?;
        let approvals = self.read_approvals()?;
        Ok(approvals.approvals.get(&installed.id).cloned())
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
        let script_id = match script_reference {
            Some(reference) => Some(self.find_script(reference)?.id),
            None => None,
        };
        let path = self.run_history_path();
        let file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(StorageError::Io { path, source }),
        };

        let mut records = Vec::new();
        for line in io::BufReader::new(file).lines() {
            let line = line.map_err(|source| StorageError::Io {
                path: path.clone(),
                source,
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let record = serde_json::from_str::<StoredRunRecord>(&line).map_err(|source| {
                StorageError::Json {
                    path: path.clone(),
                    source,
                }
            })?;
            if script_id
                .as_deref()
                .is_none_or(|script_id| record.script_id == script_id)
            {
                records.push(record);
            }
        }

        records.sort_by(|left, right| {
            right
                .completed_at_unix
                .cmp(&left.completed_at_unix)
                .then_with(|| right.run_id.cmp(&left.run_id))
        });
        if let Some(limit) = limit {
            records.truncate(limit);
        }
        Ok(records)
    }

    fn remove_script(&self, reference: &str) -> Result<InstalledScript, StorageError> {
        let mut index = self.read_index()?;
        let installed = self.resolve_reference(&index, reference)?;
        index.scripts.remove(&installed.id);
        self.write_index(&index)?;
        self.revoke_script_approval_by_id(&installed.id)?;

        remove_file_inside_root(&self.scripts_dir(), &installed.package_path)?;

        Ok(installed)
    }

    fn revoke_script_approval(
        &self,
        script_reference: &str,
    ) -> Result<Option<ScriptApproval>, StorageError> {
        let installed = self.find_script(script_reference)?;
        self.revoke_script_approval_by_id(&installed.id)
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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct StorageIndex {
    #[serde(default = "storage_format_version")]
    format_version: u32,
    #[serde(default)]
    scripts: BTreeMap<String, InstalledScript>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct ApprovalIndex {
    #[serde(default = "storage_format_version")]
    format_version: u32,
    #[serde(default)]
    approvals: BTreeMap<String, ScriptApproval>,
}

fn storage_format_version() -> u32 {
    STORAGE_FORMAT_VERSION
}

fn validate_script_id(script_id: &str) -> Result<(), StorageError> {
    if script_id.is_empty()
        || script_id == "."
        || script_id == ".."
        || script_id
            .chars()
            .any(|character| !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_')))
    {
        return Err(StorageError::InvalidScriptId(script_id.to_owned()));
    }

    Ok(())
}

fn package_file_name_from_path(path: &Path) -> Result<String, StorageError> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| StorageError::InvalidPackageFileName(path.display().to_string()))?;
    validate_package_file_name(file_name)?;
    Ok(file_name.to_owned())
}

fn validate_package_file_name(file_name: &str) -> Result<(), StorageError> {
    let lower = file_name.to_ascii_lowercase();
    if file_name.is_empty()
        || file_name == "."
        || file_name == ".."
        || !lower.ends_with(".bbs")
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains(':')
        || file_name.chars().any(|character| character.is_control())
    {
        return Err(StorageError::InvalidPackageFileName(file_name.to_owned()));
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, StorageError> {
    let mut file = fs::File::open(path).map_err(|source| StorageError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let bytes_read = file.read(&mut buffer).map_err(|source| StorageError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), StorageError> {
    if let Some(parent) = destination.parent() {
        create_dir_all(parent)?;
    }

    fs::copy(source, destination).map_err(|source| StorageError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn write_atomic(path: &Path, content: &[u8]) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }

    let temporary_path = path.with_extension("tmp");
    fs::write(&temporary_path, content).map_err(|source| StorageError::Io {
        path: temporary_path.clone(),
        source,
    })?;
    fs::rename(&temporary_path, path).map_err(|source| StorageError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn create_dir_all(path: impl AsRef<Path>) -> Result<(), StorageError> {
    let path = path.as_ref();
    fs::create_dir_all(path).map_err(|source| StorageError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn remove_file_inside_root(root: &Path, target: &Path) -> Result<(), StorageError> {
    let root = root.canonicalize().map_err(|source| StorageError::Io {
        path: root.to_path_buf(),
        source,
    })?;

    if !target.exists() {
        return Ok(());
    }

    let target = target.canonicalize().map_err(|source| StorageError::Io {
        path: target.to_path_buf(),
        source,
    })?;

    if !target.starts_with(&root) {
        return Err(StorageError::PathOutsideRoot { path: target, root });
    }

    fs::remove_file(&target).map_err(|source| StorageError::Io {
        path: target,
        source,
    })
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_lists_finds_and_removes_script() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("script.bbs");
        fs::write(&package_path, b"package bytes").expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let imported = store
            .import_script(ImportScriptRequest {
                id: "script-1".to_owned(),
                name: "Script One".to_owned(),
                package_source: package_path,
                package_format_version: 1,
                script_language_version: 1,
                target_runtime: "Generic Desktop".to_owned(),
                asset_count: 0,
                risk_level: "low".to_owned(),
            })
            .expect("script should import");

        assert!(imported.package_path.exists());
        assert_eq!(imported.package_file_name, "script.bbs");
        assert_eq!(store.list_scripts().expect("scripts should list").len(), 1);
        assert_eq!(
            store
                .find_script("Script One")
                .expect("script should be found by name")
                .id,
            "script-1"
        );

        let removed = store
            .remove_script("script-1")
            .expect("script should be removed");
        assert_eq!(removed.id, "script-1");
        assert!(
            store
                .list_scripts()
                .expect("scripts should list")
                .is_empty()
        );
        assert!(!store.root().join("scripts").join("script.bbs").exists());
    }

    #[test]
    fn updates_existing_script_package() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("script.bbs");
        let updated_package_path = temporary_directory.path().join("script-updated.bbs");
        fs::write(&package_path, b"package bytes").expect("test package should be written");
        fs::write(&updated_package_path, b"updated bytes").expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        store
            .import_script(ImportScriptRequest {
                id: "script-1".to_owned(),
                name: "Script One".to_owned(),
                package_source: package_path,
                package_format_version: 1,
                script_language_version: 1,
                target_runtime: "Generic Desktop".to_owned(),
                asset_count: 0,
                risk_level: "low".to_owned(),
            })
            .expect("script should import");

        let updated = store
            .update_script(ImportScriptRequest {
                id: "script-1".to_owned(),
                name: "Script One Updated".to_owned(),
                package_source: updated_package_path,
                package_format_version: 1,
                script_language_version: 1,
                target_runtime: "Generic Desktop".to_owned(),
                asset_count: 0,
                risk_level: "medium".to_owned(),
            })
            .expect("script should update");

        assert_eq!(updated.name, "Script One Updated");
        assert_eq!(updated.risk_level, "medium");
        assert_eq!(updated.package_file_name, "script-updated.bbs");
        assert!(updated.package_path.exists());
        assert!(!store.root().join("scripts").join("script.bbs").exists());
        assert!(store.verify_script_package_hash("script-1").is_ok());
    }

    #[test]
    fn stores_finds_and_revokes_script_approvals() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("script.bbs");
        fs::write(&package_path, b"package bytes").expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let imported = store
            .import_script(ImportScriptRequest {
                id: "script-1".to_owned(),
                name: "Script One".to_owned(),
                package_source: package_path,
                package_format_version: 1,
                script_language_version: 1,
                target_runtime: "Generic Desktop".to_owned(),
                asset_count: 0,
                risk_level: "medium".to_owned(),
            })
            .expect("script should import");

        let approval = store
            .approve_script(ApproveScriptRequest {
                approved_permissions: vec!["http_request".to_owned()],
                package_hash: imported.package_hash.clone(),
                script_id: imported.id.clone(),
            })
            .expect("script should approve");

        assert_eq!(approval.script_id, "script-1");
        assert_eq!(approval.package_hash, imported.package_hash);
        assert_eq!(approval.approved_permissions, ["http_request"]);
        assert_eq!(
            store
                .find_script_approval("Script One")
                .expect("approval lookup should succeed")
                .expect("approval should exist")
                .script_id,
            "script-1"
        );

        let revoked = store
            .revoke_script_approval("script-1")
            .expect("approval should revoke");
        assert!(revoked.is_some());
        assert!(
            store
                .find_script_approval("script-1")
                .expect("approval lookup should succeed")
                .is_none()
        );
    }

    #[test]
    fn update_and_remove_clear_script_approval() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("script.bbs");
        let updated_package_path = temporary_directory.path().join("script-updated.bbs");
        fs::write(&package_path, b"package bytes").expect("test package should be written");
        fs::write(&updated_package_path, b"updated bytes").expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let imported = store
            .import_script(ImportScriptRequest {
                id: "script-1".to_owned(),
                name: "Script One".to_owned(),
                package_source: package_path,
                package_format_version: 1,
                script_language_version: 1,
                target_runtime: "Generic Desktop".to_owned(),
                asset_count: 0,
                risk_level: "high".to_owned(),
            })
            .expect("script should import");

        store
            .approve_script(ApproveScriptRequest {
                approved_permissions: vec!["file_write_limited".to_owned()],
                package_hash: imported.package_hash,
                script_id: "script-1".to_owned(),
            })
            .expect("script should approve");

        store
            .update_script(ImportScriptRequest {
                id: "script-1".to_owned(),
                name: "Script One Updated".to_owned(),
                package_source: updated_package_path.clone(),
                package_format_version: 1,
                script_language_version: 1,
                target_runtime: "Generic Desktop".to_owned(),
                asset_count: 0,
                risk_level: "high".to_owned(),
            })
            .expect("script should update");
        assert!(
            store
                .find_script_approval("script-1")
                .expect("approval lookup should succeed")
                .is_none()
        );

        let updated = store.find_script("script-1").expect("script should exist");
        store
            .approve_script(ApproveScriptRequest {
                approved_permissions: vec!["file_write_limited".to_owned()],
                package_hash: updated.package_hash,
                script_id: "script-1".to_owned(),
            })
            .expect("script should approve again");

        store
            .remove_script("script-1")
            .expect("script should remove");
        assert!(matches!(
            store.find_script_approval("script-1"),
            Err(StorageError::NotFound(_))
        ));
    }

    #[test]
    fn detects_hash_mismatch() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("script.bbs");
        fs::write(&package_path, b"package bytes").expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        let imported = store
            .import_script(ImportScriptRequest {
                id: "script-1".to_owned(),
                name: "Script One".to_owned(),
                package_source: package_path,
                package_format_version: 1,
                script_language_version: 1,
                target_runtime: "Generic Desktop".to_owned(),
                asset_count: 0,
                risk_level: "low".to_owned(),
            })
            .expect("script should import");

        fs::write(&imported.package_path, b"tampered bytes").expect("stored package should mutate");

        assert!(matches!(
            store.verify_script_package_hash("script-1"),
            Err(StorageError::HashMismatch { .. })
        ));
    }

    #[test]
    fn rejects_unsafe_script_ids() {
        assert!(matches!(
            validate_script_id("../nope"),
            Err(StorageError::InvalidScriptId(_))
        ));
        assert!(matches!(
            validate_script_id("bad/slash"),
            Err(StorageError::InvalidScriptId(_))
        ));
    }

    #[test]
    fn appends_and_filters_run_records() {
        let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
        let package_path = temporary_directory.path().join("script.bbs");
        fs::write(&package_path, b"package bytes").expect("test package should be written");

        let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
        store
            .import_script(ImportScriptRequest {
                id: "script-1".to_owned(),
                name: "Script One".to_owned(),
                package_source: package_path,
                package_format_version: 1,
                script_language_version: 1,
                target_runtime: "Generic Desktop".to_owned(),
                asset_count: 0,
                risk_level: "low".to_owned(),
            })
            .expect("script should import");

        store
            .append_run_record(test_run_record("run-1", "script-1", 10))
            .expect("first run should append");
        store
            .append_run_record(test_run_record("run-2", "script-2", 20))
            .expect("second run should append");
        store
            .append_run_record(test_run_record("run-3", "script-1", 30))
            .expect("third run should append");

        let all_records = store
            .list_run_records(None, Some(2))
            .expect("all records should list");
        assert_eq!(
            all_records
                .iter()
                .map(|record| record.run_id.as_str())
                .collect::<Vec<_>>(),
            ["run-3", "run-2"]
        );

        let script_records = store
            .list_run_records(Some("Script One"), None)
            .expect("script records should list");
        assert_eq!(
            script_records
                .iter()
                .map(|record| record.run_id.as_str())
                .collect::<Vec<_>>(),
            ["run-3", "run-1"]
        );
    }

    fn test_run_record(run_id: &str, script_id: &str, completed_at_unix: u64) -> StoredRunRecord {
        StoredRunRecord {
            completed_at_unix,
            logs: vec![RunLogEntry {
                level: "info".to_owned(),
                message: format!("{run_id} completed"),
                node_id: None,
            }],
            run_id: run_id.to_owned(),
            script_id: script_id.to_owned(),
            status: "completed".to_owned(),
            trigger_node_id: "n-trigger".to_owned(),
            variables: BTreeMap::new(),
        }
    }
}
