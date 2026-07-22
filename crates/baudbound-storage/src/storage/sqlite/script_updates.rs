use rusqlite::{OptionalExtension, params};

use crate::{ScriptUpdateState, StorageError};

use super::{SqliteRunnerStore, conversions::row_i64_to_bool, rows::resolve_script};

impl SqliteRunnerStore {
    pub fn script_update_state(
        &self,
        script_reference: &str,
    ) -> Result<ScriptUpdateState, StorageError> {
        let connection = self.connection()?;
        let script = resolve_script(&connection, &self.path, script_reference)?;
        connection
            .query_row(
                r#"
                SELECT script_id, automatic_checks_enabled, checked_update_url,
                    last_checked_at_unix, last_success_at_unix, latest_version,
                    package_url, package_sha256, package_size, published_at,
                    release_notes, last_error
                FROM script_update_state
                WHERE script_id = ?1
                "#,
                params![script.id],
                row_to_script_update_state,
            )
            .optional()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })
            .map(|state| state.unwrap_or_else(|| ScriptUpdateState::empty(script.id)))
    }

    pub fn list_script_update_states(&self) -> Result<Vec<ScriptUpdateState>, StorageError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT scripts.id,
                    COALESCE(script_update_state.automatic_checks_enabled, 0),
                    script_update_state.checked_update_url,
                    script_update_state.last_checked_at_unix,
                    script_update_state.last_success_at_unix,
                    script_update_state.latest_version,
                    script_update_state.package_url,
                    script_update_state.package_sha256,
                    script_update_state.package_size,
                    script_update_state.published_at,
                    script_update_state.release_notes,
                    script_update_state.last_error
                FROM scripts
                LEFT JOIN script_update_state ON script_update_state.script_id = scripts.id
                ORDER BY scripts.id
                "#,
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        let rows = statement
            .query_map([], row_to_script_update_state)
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

    pub fn set_script_automatic_update_checks(
        &self,
        script_reference: &str,
        enabled: bool,
    ) -> Result<ScriptUpdateState, StorageError> {
        let script = self.resolve_reference(script_reference)?;
        let connection = self.connection()?;
        connection
            .execute(
                r#"
                INSERT INTO script_update_state (script_id, automatic_checks_enabled)
                VALUES (?1, ?2)
                ON CONFLICT(script_id) DO UPDATE SET
                    automatic_checks_enabled = excluded.automatic_checks_enabled
                "#,
                params![script.id, i64::from(enabled)],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        drop(connection);
        self.script_update_state(&script.id)
    }

    pub fn record_script_update_success(
        &self,
        state: &ScriptUpdateState,
    ) -> Result<(), StorageError> {
        let checked_at = state
            .last_checked_at_unix
            .ok_or_else(|| StorageError::Operation("update check time is missing".to_owned()))?;
        let success_at = state.last_success_at_unix.ok_or_else(|| {
            StorageError::Operation("successful update check time is missing".to_owned())
        })?;
        let checked_at = sqlite_integer("update check time", checked_at)?;
        let success_at = sqlite_integer("successful update check time", success_at)?;
        let package_size = state
            .package_size
            .map(|value| sqlite_integer("package size", value))
            .transpose()?;
        let connection = self.connection()?;
        connection
            .execute(
                r#"
                INSERT INTO script_update_state (
                    script_id, automatic_checks_enabled, checked_update_url,
                    last_checked_at_unix, last_success_at_unix, latest_version,
                    package_url, package_sha256, package_size, published_at,
                    release_notes, last_error
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL)
                ON CONFLICT(script_id) DO UPDATE SET
                    checked_update_url = excluded.checked_update_url,
                    last_checked_at_unix = excluded.last_checked_at_unix,
                    last_success_at_unix = excluded.last_success_at_unix,
                    latest_version = excluded.latest_version,
                    package_url = excluded.package_url,
                    package_sha256 = excluded.package_sha256,
                    package_size = excluded.package_size,
                    published_at = excluded.published_at,
                    release_notes = excluded.release_notes,
                    last_error = NULL
                "#,
                params![
                    state.script_id,
                    i64::from(state.automatic_checks_enabled),
                    state.checked_update_url,
                    checked_at,
                    success_at,
                    state.latest_version,
                    state.package_url,
                    state.package_sha256,
                    package_size,
                    state.published_at,
                    state.release_notes,
                ],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }

    pub fn record_script_update_failure(
        &self,
        script_reference: &str,
        update_url: &str,
        checked_at_unix: u64,
        error: &str,
    ) -> Result<(), StorageError> {
        let script = self.resolve_reference(script_reference)?;
        let checked_at_unix = sqlite_integer("update check time", checked_at_unix)?;
        let connection = self.connection()?;
        connection
            .execute(
                r#"
                INSERT INTO script_update_state (
                    script_id, checked_update_url, last_checked_at_unix, last_error
                ) VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(script_id) DO UPDATE SET
                    checked_update_url = excluded.checked_update_url,
                    last_checked_at_unix = excluded.last_checked_at_unix,
                    latest_version = NULL,
                    package_url = NULL,
                    package_sha256 = NULL,
                    package_size = NULL,
                    published_at = NULL,
                    release_notes = NULL,
                    last_error = excluded.last_error
                "#,
                params![script.id, update_url, checked_at_unix, error],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }

    pub fn clear_script_update_discovery(
        &self,
        script_reference: &str,
    ) -> Result<(), StorageError> {
        let script = self.resolve_reference(script_reference)?;
        let connection = self.connection()?;
        connection
            .execute(
                r#"
                UPDATE script_update_state SET
                    checked_update_url = NULL,
                    last_checked_at_unix = NULL,
                    last_success_at_unix = NULL,
                    latest_version = NULL,
                    package_url = NULL,
                    package_sha256 = NULL,
                    package_size = NULL,
                    published_at = NULL,
                    release_notes = NULL,
                    last_error = NULL
                WHERE script_id = ?1
                "#,
                params![script.id],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }
}

fn row_to_script_update_state(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScriptUpdateState> {
    Ok(ScriptUpdateState {
        script_id: row.get(0)?,
        automatic_checks_enabled: row_i64_to_bool(1, row.get(1)?)?,
        checked_update_url: row.get(2)?,
        last_checked_at_unix: optional_unsigned(3, row.get(3)?)?,
        last_success_at_unix: optional_unsigned(4, row.get(4)?)?,
        latest_version: row.get(5)?,
        package_url: row.get(6)?,
        package_sha256: row.get(7)?,
        package_size: optional_unsigned(8, row.get(8)?)?,
        published_at: row.get(9)?,
        release_notes: row.get(10)?,
        last_error: row.get(11)?,
    })
}

fn sqlite_integer(label: &str, value: u64) -> Result<i64, StorageError> {
    i64::try_from(value)
        .map_err(|_| StorageError::Operation(format!("{label} exceeds the storage limit")))
}

fn optional_unsigned(index: usize, value: Option<i64>) -> rusqlite::Result<Option<u64>> {
    value
        .map(|value| {
            u64::try_from(value).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })
        })
        .transpose()
}
