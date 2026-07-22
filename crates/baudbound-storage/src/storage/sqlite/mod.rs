use std::{
    fmt, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, RwLock},
};

use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    ApproveScriptRequest, ImportScriptRequest, InstalledScript, RunStatistics, ScriptApproval,
    ScriptApprovalResult, ScriptStore, SecretCipher, SecretStatus, StorageError, StoredRunRecord,
    StoredVariable, StoredVariableChange, StoredVariableScope,
    storage::filesystem::{
        copy_file, create_dir_all, current_unix_timestamp, package_file_name_from_path,
        remove_file_inside_root, sha256_file, validate_package_file_name, validate_script_id,
    },
};

mod conversions;
mod history_queries;
mod network_auth;
mod rows;
mod run_retention;
mod schema;
mod scoped_variables;
mod secrets;
mod update_cache;

use conversions::{
    bool_to_sqlite, row_i64_to_usize, u32_to_sqlite, u64_to_sqlite, unix_timestamp_for_sqlite,
    usize_to_sqlite,
};
use rows::{resolve_script, row_to_approval, row_to_installed_script, row_to_run_record};
pub use run_retention::RunRetentionPolicy;
use run_retention::{append_run_record_with_retention, prune_run_records};
use schema::{configure_connection, migrate, query_schema_version};

pub use schema::CURRENT_SCHEMA_VERSION;

#[derive(Clone, Debug)]
pub struct SqliteRunnerStore {
    path: PathBuf,
    root: PathBuf,
    connection: Arc<Mutex<Connection>>,
    run_retention: Arc<RwLock<RunRetentionPolicy>>,
    secret_cipher: Arc<RwLock<Option<SecretCipher>>>,
    variable_change_observer: Arc<RwLock<Option<VariableChangeObserver>>>,
}

#[derive(Clone)]
struct VariableChangeObserver(Arc<dyn Fn(StoredVariableChange) + Send + Sync + 'static>);

impl fmt::Debug for VariableChangeObserver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VariableChangeObserver(..)")
    }
}

impl SqliteRunnerStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let path = path.into();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|source| StorageError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let root = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        let connection = Connection::open(&path).map_err(|source| StorageError::Sqlite {
            path: path.clone(),
            source,
        })?;
        restrict_database_permissions(&path)?;
        configure_connection(&connection, &path)?;
        migrate(&connection, &path)?;

        Ok(Self {
            path,
            root,
            connection: Arc::new(Mutex::new(connection)),
            run_retention: Arc::new(RwLock::new(RunRetentionPolicy::default())),
            secret_cipher: Arc::new(RwLock::new(None)),
            variable_change_observer: Arc::new(RwLock::new(None)),
        })
    }

    #[must_use]
    pub fn with_secret_cipher(self, secret_cipher: SecretCipher) -> Self {
        self.set_secret_cipher(secret_cipher);
        self
    }

    pub fn set_secret_cipher(&self, secret_cipher: SecretCipher) {
        *self
            .secret_cipher
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(secret_cipher);
    }

    pub fn set_variable_change_observer<F>(&self, observer: F)
    where
        F: Fn(StoredVariableChange) + Send + Sync + 'static,
    {
        *self
            .variable_change_observer
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(VariableChangeObserver(Arc::new(observer)));
    }

    fn notify_variable_changed(&self, change: StoredVariableChange) {
        let observer = self
            .variable_change_observer
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(observer) = observer {
            (observer.0)(change);
        }
    }

    #[must_use]
    pub fn has_secret_cipher(&self) -> bool {
        self.secret_cipher
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    }

    fn secret_cipher(&self) -> Result<SecretCipher, StorageError> {
        self.secret_cipher
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .ok_or(StorageError::SecretKeyUnavailable)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn schema_version(&self) -> Result<i64, StorageError> {
        let connection = self.connection()?;
        query_schema_version(&connection, &self.path)
    }

    pub fn set_run_retention_policy(
        &self,
        policy: RunRetentionPolicy,
    ) -> Result<usize, StorageError> {
        policy.validate()?;
        let mut current_policy = self
            .run_retention
            .write()
            .map_err(|_| StorageError::Operation("run retention lock is poisoned".to_owned()))?;
        let mut connection = self.connection()?;
        let deleted = prune_run_records(&mut connection, &self.path, policy)?;
        *current_policy = policy;
        Ok(deleted)
    }

    pub fn clear_run_records(&self) -> Result<usize, StorageError> {
        let connection = self.connection()?;
        connection
            .execute("DELETE FROM run_records", [])
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })
    }

    pub fn clear_run_logs(&self) -> Result<usize, StorageError> {
        let connection = self.connection()?;
        connection
            .execute(
                r#"
                UPDATE run_records
                SET logs_json = '[]',
                    has_errors = CASE WHEN status = 'failed' THEN 1 ELSE 0 END
                WHERE logs_json <> '[]'
                "#,
                [],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })
    }

    pub fn write_service_status(&self, status: &serde_json::Value) -> Result<(), StorageError> {
        let status_json = serde_json::to_string(status).map_err(|source| StorageError::Json {
            path: self.path.clone(),
            source,
        })?;
        let updated_at_unix = unix_timestamp_for_sqlite()?;
        let connection = self.connection()?;
        connection
            .execute(
                r#"
                INSERT INTO service_status (id, status_json, updated_at_unix)
                VALUES (1, ?1, ?2)
                ON CONFLICT(id) DO UPDATE SET
                    status_json = excluded.status_json,
                    updated_at_unix = excluded.updated_at_unix
                "#,
                params![status_json, updated_at_unix],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;

        Ok(())
    }

    pub fn read_service_status(&self) -> Result<Option<serde_json::Value>, StorageError> {
        let connection = self.connection()?;
        let status_json = connection
            .query_row(
                "SELECT status_json FROM service_status WHERE id = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        status_json
            .map(|content| {
                serde_json::from_str(&content).map_err(|source| StorageError::Json {
                    path: self.path.clone(),
                    source,
                })
            })
            .transpose()
    }

    pub fn clear_service_status(&self) -> Result<bool, StorageError> {
        let connection = self.connection()?;
        let deleted = connection
            .execute("DELETE FROM service_status WHERE id = 1", [])
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(deleted > 0)
    }

    pub fn request_trigger_reload(&self) -> Result<(), StorageError> {
        let connection = self.connection()?;
        self.request_trigger_reload_with_connection(&connection)
    }

    fn request_trigger_reload_with_connection(
        &self,
        connection: &Connection,
    ) -> Result<(), StorageError> {
        let requested_at_unix = unix_timestamp_for_sqlite()?;
        connection
            .execute(
                r#"
                INSERT INTO runner_signals (name, requested_at_unix)
                VALUES ('trigger_reload', ?1)
                ON CONFLICT(name) DO UPDATE SET
                    requested_at_unix = excluded.requested_at_unix
                "#,
                params![requested_at_unix],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }

    pub fn consume_trigger_reload_request(&self) -> Result<bool, StorageError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        let exists = transaction
            .query_row(
                "SELECT 1 FROM runner_signals WHERE name = 'trigger_reload'",
                [],
                |_| Ok(()),
            )
            .optional()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?
            .is_some();
        if exists {
            transaction
                .execute(
                    "DELETE FROM runner_signals WHERE name = 'trigger_reload'",
                    [],
                )
                .map_err(|source| StorageError::Sqlite {
                    path: self.path.clone(),
                    source,
                })?;
        }
        transaction
            .commit()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(exists)
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
        self.connection
            .lock()
            .map_err(|_| StorageError::Operation("SQLite storage lock is poisoned".to_owned()))
    }

    fn ensure_layout(&self) -> Result<(), StorageError> {
        create_dir_all(self.scripts_dir())
    }

    fn scripts_dir(&self) -> PathBuf {
        self.root.join("scripts")
    }

    fn package_path(&self, package_file_name: &str) -> Result<PathBuf, StorageError> {
        validate_package_file_name(package_file_name)?;
        Ok(self.scripts_dir().join(package_file_name))
    }

    fn ensure_package_file_available(
        &self,
        connection: &Connection,
        package_file_name: &str,
        allowed_script_id: Option<&str>,
    ) -> Result<(), StorageError> {
        let existing = connection
            .query_row(
                "SELECT id FROM scripts WHERE package_file_name = ?1",
                params![package_file_name],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        if let Some(script_id) = existing
            && Some(script_id.as_str()) != allowed_script_id
        {
            return Err(StorageError::PackageFileNameInUse {
                file_name: package_file_name.to_owned(),
                script_id,
            });
        }
        Ok(())
    }

    fn install_package(
        &self,
        connection: &Connection,
        request: ImportScriptRequest,
        existing: Option<InstalledScript>,
    ) -> Result<InstalledScript, StorageError> {
        validate_script_id(&request.id)?;
        self.ensure_layout()?;

        let package_file_name = package_file_name_from_path(&request.package_source)?;
        self.ensure_package_file_available(connection, &package_file_name, Some(&request.id))?;

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

        connection
            .execute(
                r#"
                INSERT INTO scripts (
                    id,
                    enabled,
                    name,
                    package_hash,
                    package_file_name,
                    package_path,
                    imported_at_unix,
                    package_format_version,
                    script_language_version,
                    target_runtime,
                    asset_count,
                    risk_level
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                ON CONFLICT(id) DO UPDATE SET
                    enabled = excluded.enabled,
                    name = excluded.name,
                    package_hash = excluded.package_hash,
                    package_file_name = excluded.package_file_name,
                    package_path = excluded.package_path,
                    imported_at_unix = excluded.imported_at_unix,
                    package_format_version = excluded.package_format_version,
                    script_language_version = excluded.script_language_version,
                    target_runtime = excluded.target_runtime,
                    asset_count = excluded.asset_count,
                    risk_level = excluded.risk_level
                "#,
                params![
                    installed.id,
                    bool_to_sqlite(installed.enabled),
                    installed.name,
                    installed.package_hash,
                    installed.package_file_name,
                    installed.package_path.to_string_lossy(),
                    u64_to_sqlite(installed.imported_at_unix)?,
                    u32_to_sqlite(installed.package_format_version),
                    u32_to_sqlite(installed.script_language_version),
                    installed.target_runtime,
                    usize_to_sqlite(installed.asset_count)?,
                    installed.risk_level,
                ],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;

        if let Some(previous_package_path) = previous_package_path
            && previous_package_path != installed.package_path
        {
            remove_file_inside_root(&self.scripts_dir(), &previous_package_path)?;
        }

        self.request_trigger_reload_with_connection(connection)?;
        Ok(installed)
    }

    fn resolve_reference(&self, reference: &str) -> Result<InstalledScript, StorageError> {
        let connection = self.connection()?;
        resolve_script(&connection, &self.path, reference)
    }
}

#[cfg(unix)]
fn restrict_database_permissions(path: &Path) -> Result<(), StorageError> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|source| StorageError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions).map_err(|source| StorageError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(not(unix))]
fn restrict_database_permissions(_path: &Path) -> Result<(), StorageError> {
    Ok(())
}

impl ScriptStore for SqliteRunnerStore {
    fn append_run_record(&self, record: StoredRunRecord) -> Result<(), StorageError> {
        let logs_json =
            serde_json::to_string(&record.logs).map_err(|source| StorageError::Json {
                path: self.path.clone(),
                source,
            })?;
        let variables_json =
            serde_json::to_string(&record.variables).map_err(|source| StorageError::Json {
                path: self.path.clone(),
                source,
            })?;
        let variable_scopes_json =
            serde_json::to_string(&record.variable_scopes).map_err(|source| {
                StorageError::Json {
                    path: self.path.clone(),
                    source,
                }
            })?;
        let policy = *self
            .run_retention
            .read()
            .map_err(|_| StorageError::Operation("run retention lock is poisoned".to_owned()))?;
        let mut connection = self.connection()?;
        append_run_record_with_retention(
            &mut connection,
            &self.path,
            &record,
            &logs_json,
            &variables_json,
            &variable_scopes_json,
            policy,
        )
    }

    fn approve_script(
        &self,
        request: ApproveScriptRequest,
    ) -> Result<ScriptApprovalResult, StorageError> {
        let network_triggers = request.network_triggers;
        let approval = ScriptApproval {
            approved_at_unix: current_unix_timestamp(),
            approved_permissions: request.approved_permissions,
            package_hash: request.package_hash,
            script_id: request.script_id,
        };
        let permissions_json =
            serde_json::to_string(&approval.approved_permissions).map_err(|source| {
                StorageError::Json {
                    path: self.path.clone(),
                    source,
                }
            })?;
        let connection = self.connection()?;
        let transaction =
            connection
                .unchecked_transaction()
                .map_err(|source| StorageError::Sqlite {
                    path: self.path.clone(),
                    source,
                })?;
        transaction
            .execute(
                r#"
                INSERT INTO approvals (
                    script_id,
                    package_hash,
                    approved_permissions_json,
                    approved_at_unix
                )
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(script_id) DO UPDATE SET
                    package_hash = excluded.package_hash,
                    approved_permissions_json = excluded.approved_permissions_json,
                    approved_at_unix = excluded.approved_at_unix
                "#,
                params![
                    approval.script_id,
                    approval.package_hash,
                    permissions_json,
                    u64_to_sqlite(approval.approved_at_unix)?,
                ],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        let generated_trigger_tokens = self.reconcile_network_trigger_auth_with_connection(
            &transaction,
            &approval.script_id,
            &network_triggers,
        )?;
        self.request_trigger_reload_with_connection(&transaction)?;
        transaction
            .commit()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(ScriptApprovalResult {
            approval,
            generated_trigger_tokens,
        })
    }

    fn find_script_approval(
        &self,
        script_reference: &str,
    ) -> Result<Option<ScriptApproval>, StorageError> {
        let installed = self.resolve_reference(script_reference)?;
        let connection = self.connection()?;
        let approval = connection
            .query_row(
                r#"
                SELECT script_id, package_hash, approved_permissions_json, approved_at_unix
                FROM approvals
                WHERE script_id = ?1
                "#,
                params![installed.id],
                row_to_approval,
            )
            .optional()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(approval)
    }

    fn import_script(&self, request: ImportScriptRequest) -> Result<InstalledScript, StorageError> {
        let connection = self.connection()?;
        let exists = connection
            .query_row(
                "SELECT 1 FROM scripts WHERE id = ?1",
                params![request.id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?
            .is_some();
        if exists {
            return Err(StorageError::AlreadyInstalled(request.id));
        }
        self.install_package(&connection, request, None)
    }

    fn update_script(&self, request: ImportScriptRequest) -> Result<InstalledScript, StorageError> {
        let connection = self.connection()?;
        let existing = resolve_script(&connection, &self.path, &request.id)?;
        let installed = self.install_package(&connection, request, Some(existing))?;
        connection
            .execute(
                "DELETE FROM approvals WHERE script_id = ?1",
                params![installed.id],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(installed)
    }

    fn list_scripts(&self) -> Result<Vec<InstalledScript>, StorageError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT id, enabled, name, package_hash, package_file_name, package_path,
                    imported_at_unix, package_format_version, script_language_version,
                    target_runtime, asset_count, risk_level
                FROM scripts
                ORDER BY id
                "#,
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        let rows = statement
            .query_map([], row_to_installed_script)
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })
    }

    fn list_run_records(
        &self,
        script_reference: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<StoredRunRecord>, StorageError> {
        let script_id = script_reference
            .map(|reference| self.resolve_reference(reference).map(|script| script.id))
            .transpose()?;
        let limit = limit.unwrap_or(usize::MAX).min(i64::MAX as usize);
        let connection = self.connection()?;

        let records = if let Some(script_id) = script_id {
            let mut statement = connection
                .prepare(
                    r#"
                    SELECT run_id, script_id, status, trigger_node_id, completed_at_unix,
                        logs_json, variables_json, variable_scopes_json
                    FROM run_records
                    WHERE script_id = ?1
                    ORDER BY completed_at_unix DESC, rowid DESC
                    LIMIT ?2
                    "#,
                )
                .map_err(|source| StorageError::Sqlite {
                    path: self.path.clone(),
                    source,
                })?;
            let rows = statement
                .query_map(
                    params![script_id, usize_to_sqlite(limit)?],
                    row_to_run_record,
                )
                .map_err(|source| StorageError::Sqlite {
                    path: self.path.clone(),
                    source,
                })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|source| StorageError::Sqlite {
                    path: self.path.clone(),
                    source,
                })?
        } else {
            let mut statement = connection
                .prepare(
                    r#"
                    SELECT run_id, script_id, status, trigger_node_id, completed_at_unix,
                        logs_json, variables_json, variable_scopes_json
                    FROM run_records
                    ORDER BY completed_at_unix DESC, rowid DESC
                    LIMIT ?1
                    "#,
                )
                .map_err(|source| StorageError::Sqlite {
                    path: self.path.clone(),
                    source,
                })?;
            let rows = statement
                .query_map(params![usize_to_sqlite(limit)?], row_to_run_record)
                .map_err(|source| StorageError::Sqlite {
                    path: self.path.clone(),
                    source,
                })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|source| StorageError::Sqlite {
                    path: self.path.clone(),
                    source,
                })?
        };
        Ok(records)
    }

    fn run_statistics(&self) -> Result<RunStatistics, StorageError> {
        let connection = self.connection()?;
        connection
            .query_row(
                r#"
                SELECT
                    COUNT(*),
                    COALESCE(SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN status = 'cancelled' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(has_errors), 0)
                FROM run_records
                "#,
                [],
                |row| {
                    Ok(RunStatistics {
                        total: row_i64_to_usize(0, row.get(0)?)?,
                        completed: row_i64_to_usize(1, row.get(1)?)?,
                        failed: row_i64_to_usize(2, row.get(2)?)?,
                        cancelled: row_i64_to_usize(3, row.get(3)?)?,
                        with_errors: row_i64_to_usize(4, row.get(4)?)?,
                    })
                },
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })
    }

    fn remove_script(&self, reference: &str) -> Result<InstalledScript, StorageError> {
        let installed = self.resolve_reference(reference)?;
        let connection = self.connection()?;
        connection
            .execute("DELETE FROM scripts WHERE id = ?1", params![installed.id])
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        remove_file_inside_root(&self.scripts_dir(), &installed.package_path)?;
        self.request_trigger_reload_with_connection(&connection)?;
        Ok(installed)
    }

    fn revoke_script_approval(
        &self,
        script_reference: &str,
    ) -> Result<Option<ScriptApproval>, StorageError> {
        let installed = self.resolve_reference(script_reference)?;
        let existing = self.find_script_approval(&installed.id)?;
        let connection = self.connection()?;
        connection
            .execute(
                "DELETE FROM approvals WHERE script_id = ?1",
                params![installed.id],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        self.request_trigger_reload_with_connection(&connection)?;
        Ok(existing)
    }

    fn set_script_enabled(
        &self,
        reference: &str,
        enabled: bool,
    ) -> Result<InstalledScript, StorageError> {
        let mut installed = self.resolve_reference(reference)?;
        let connection = self.connection()?;
        connection
            .execute(
                "UPDATE scripts SET enabled = ?1 WHERE id = ?2",
                params![bool_to_sqlite(enabled), installed.id],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        installed.enabled = enabled;
        self.request_trigger_reload_with_connection(&connection)?;
        Ok(installed)
    }

    fn find_script(&self, reference: &str) -> Result<InstalledScript, StorageError> {
        self.resolve_reference(reference)
    }

    fn verify_script_package_hash(&self, reference: &str) -> Result<InstalledScript, StorageError> {
        let installed = self.resolve_reference(reference)?;
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

    fn list_trigger_auth_statuses(
        &self,
        script_reference: &str,
    ) -> Result<Vec<crate::TriggerAuthStatus>, StorageError> {
        self.list_network_trigger_auth_statuses(script_reference)
    }

    fn rotate_trigger_auth_token(
        &self,
        script_reference: &str,
        node_id: &str,
        trigger_type: crate::NetworkTriggerType,
    ) -> Result<crate::GeneratedTriggerToken, StorageError> {
        self.rotate_network_trigger_auth_token(script_reference, node_id, trigger_type)
    }

    fn set_trigger_auth_enabled(
        &self,
        script_reference: &str,
        node_id: &str,
        trigger_type: crate::NetworkTriggerType,
        enabled: bool,
    ) -> Result<crate::TriggerAuthStatus, StorageError> {
        self.set_network_trigger_auth_enabled(script_reference, node_id, trigger_type, enabled)
    }

    fn authenticate_trigger(
        &self,
        script_id: &str,
        node_id: &str,
        trigger_type: crate::NetworkTriggerType,
        provided_token: Option<&str>,
    ) -> Result<crate::TriggerAuthentication, StorageError> {
        self.authenticate_network_trigger(script_id, node_id, trigger_type, provided_token)
    }

    fn load_variable(
        &self,
        scope: StoredVariableScope,
        script_id: &str,
        name: &str,
    ) -> Result<Option<StoredVariable>, StorageError> {
        self.load_scoped_variable(scope, script_id, name)
    }

    fn compare_and_set_variable(
        &self,
        scope: StoredVariableScope,
        script_id: &str,
        name: &str,
        expected_version: Option<u64>,
        value: &serde_json::Value,
    ) -> Result<bool, StorageError> {
        self.compare_and_set_scoped_variable(scope, script_id, name, expected_version, value)
    }

    fn list_secret_statuses(
        &self,
        script_reference: &str,
    ) -> Result<Vec<SecretStatus>, StorageError> {
        self.list_stored_secret_statuses(script_reference)
    }

    fn read_secret(
        &self,
        script_id: &str,
        name: &str,
    ) -> Result<Option<serde_json::Value>, StorageError> {
        self.read_stored_secret(script_id, name)
    }

    fn set_secret(
        &self,
        script_reference: &str,
        name: &str,
        value: &serde_json::Value,
    ) -> Result<SecretStatus, StorageError> {
        self.set_stored_secret(script_reference, name, value)
    }

    fn remove_secret(&self, script_reference: &str, name: &str) -> Result<bool, StorageError> {
        self.remove_stored_secret(script_reference, name)
    }
}
