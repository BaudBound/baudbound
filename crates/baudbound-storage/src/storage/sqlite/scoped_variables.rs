use rusqlite::{OptionalExtension, params};

use crate::{StorageError, StoredVariable, StoredVariableScope};

use super::{SqliteRunnerStore, conversions::unix_timestamp_for_sqlite};

impl SqliteRunnerStore {
    pub(super) fn load_scoped_variable(
        &self,
        scope: StoredVariableScope,
        script_id: &str,
        name: &str,
    ) -> Result<Option<StoredVariable>, StorageError> {
        let connection = self.connection()?;
        let row = match scope {
            StoredVariableScope::Persistent => connection
                .query_row(
                    "SELECT value_json, version FROM persistent_variables WHERE script_id = ?1 AND name = ?2",
                    params![script_id, name],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional(),
            StoredVariableScope::Global => connection
                .query_row(
                    "SELECT value_json, version FROM global_variables WHERE name = ?1",
                    params![name],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional(),
        }
        .map_err(|source| StorageError::Sqlite {
            path: self.path.clone(),
            source,
        })?;

        row.map(|(value_json, version)| {
            let value = serde_json::from_str(&value_json).map_err(|source| StorageError::Json {
                path: self.path.clone(),
                source,
            })?;
            let version = u64::try_from(version).map_err(|_| {
                StorageError::Operation("stored variable version is invalid".to_owned())
            })?;
            Ok(StoredVariable { value, version })
        })
        .transpose()
    }

    pub(super) fn compare_and_set_scoped_variable(
        &self,
        scope: StoredVariableScope,
        script_id: &str,
        name: &str,
        expected_version: Option<u64>,
        value: &serde_json::Value,
    ) -> Result<bool, StorageError> {
        let value_json = serde_json::to_string(value).map_err(|source| StorageError::Json {
            path: self.path.clone(),
            source,
        })?;
        let updated_at_unix = unix_timestamp_for_sqlite()?;
        let expected_version = expected_version
            .map(i64::try_from)
            .transpose()
            .map_err(|_| {
                StorageError::Operation("variable version exceeds SQLite range".to_owned())
            })?;
        let connection = self.connection()?;

        let changed = match (scope, expected_version) {
            (StoredVariableScope::Persistent, None) => connection.execute(
                "INSERT OR IGNORE INTO persistent_variables (script_id, name, value_json, version, updated_at_unix) VALUES (?1, ?2, ?3, 1, ?4)",
                params![script_id, name, value_json, updated_at_unix],
            ),
            (StoredVariableScope::Persistent, Some(version)) => connection.execute(
                "UPDATE persistent_variables SET value_json = ?1, version = version + 1, updated_at_unix = ?2 WHERE script_id = ?3 AND name = ?4 AND version = ?5",
                params![value_json, updated_at_unix, script_id, name, version],
            ),
            (StoredVariableScope::Global, None) => connection.execute(
                "INSERT OR IGNORE INTO global_variables (name, value_json, version, updated_at_unix) VALUES (?1, ?2, 1, ?3)",
                params![name, value_json, updated_at_unix],
            ),
            (StoredVariableScope::Global, Some(version)) => connection.execute(
                "UPDATE global_variables SET value_json = ?1, version = version + 1, updated_at_unix = ?2 WHERE name = ?3 AND version = ?4",
                params![value_json, updated_at_unix, name, version],
            ),
        }
        .map_err(|source| StorageError::Sqlite {
            path: self.path.clone(),
            source,
        })?;

        Ok(changed == 1)
    }
}
