use std::{path::Path, time::Duration};

use rusqlite::Connection;

use crate::StorageError;

pub const CURRENT_SCHEMA_VERSION: i64 = 3;

pub(super) fn configure_connection(
    connection: &Connection,
    path: &Path,
) -> Result<(), StorageError> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|source| StorageError::Sqlite {
            path: path.to_path_buf(),
            source,
        })?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|source| StorageError::Sqlite {
            path: path.to_path_buf(),
            source,
        })?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|source| StorageError::Sqlite {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(())
}

pub(super) fn migrate(connection: &Connection, path: &Path) -> Result<(), StorageError> {
    let version = query_schema_version(connection, path)?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(StorageError::Operation(format!(
            "runner database schema version {version} is newer than this runner supports ({CURRENT_SCHEMA_VERSION})"
        )));
    }
    if version == CURRENT_SCHEMA_VERSION {
        return Ok(());
    }

    connection
        .execute_batch(
            r#"
            BEGIN;

            CREATE TABLE IF NOT EXISTS scripts (
                id TEXT PRIMARY KEY,
                enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
                name TEXT NOT NULL,
                package_hash TEXT NOT NULL,
                package_file_name TEXT NOT NULL,
                package_path TEXT NOT NULL,
                imported_at_unix INTEGER NOT NULL,
                package_format_version INTEGER NOT NULL,
                script_language_version INTEGER NOT NULL,
                target_runtime TEXT NOT NULL,
                asset_count INTEGER NOT NULL,
                risk_level TEXT NOT NULL
            );

            CREATE UNIQUE INDEX IF NOT EXISTS scripts_package_file_name_unique
                ON scripts(package_file_name);

            CREATE TABLE IF NOT EXISTS approvals (
                script_id TEXT PRIMARY KEY REFERENCES scripts(id) ON DELETE CASCADE,
                package_hash TEXT NOT NULL,
                approved_permissions_json TEXT NOT NULL,
                approved_at_unix INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS run_records (
                run_id TEXT PRIMARY KEY,
                script_id TEXT NOT NULL,
                status TEXT NOT NULL,
                trigger_node_id TEXT NOT NULL,
                completed_at_unix INTEGER NOT NULL,
                logs_json TEXT NOT NULL,
                variables_json TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS run_records_completed_at_index
                ON run_records(completed_at_unix DESC);

            CREATE INDEX IF NOT EXISTS run_records_script_id_index
                ON run_records(script_id, completed_at_unix DESC);

            CREATE TABLE IF NOT EXISTS service_status (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                status_json TEXT NOT NULL,
                updated_at_unix INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS runner_signals (
                name TEXT PRIMARY KEY,
                requested_at_unix INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS persistent_variables (
                script_id TEXT NOT NULL REFERENCES scripts(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                value_json TEXT NOT NULL,
                version INTEGER NOT NULL CHECK (version >= 1),
                updated_at_unix INTEGER NOT NULL,
                PRIMARY KEY (script_id, name)
            );

            CREATE TABLE IF NOT EXISTS global_variables (
                name TEXT PRIMARY KEY,
                value_json TEXT NOT NULL,
                version INTEGER NOT NULL CHECK (version >= 1),
                updated_at_unix INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS secret_values (
                script_id TEXT NOT NULL REFERENCES scripts(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                nonce BLOB NOT NULL,
                ciphertext BLOB NOT NULL,
                updated_at_unix INTEGER NOT NULL,
                PRIMARY KEY (script_id, name)
            );

            CREATE TABLE IF NOT EXISTS desktop_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                automatic_update_checks INTEGER NOT NULL CHECK (automatic_update_checks IN (0, 1)),
                keep_running_on_close INTEGER NOT NULL CHECK (keep_running_on_close IN (0, 1)),
                launch_at_login INTEGER NOT NULL CHECK (launch_at_login IN (0, 1)),
                start_background_runner_on_launch INTEGER NOT NULL CHECK (start_background_runner_on_launch IN (0, 1)),
                start_minimized_to_tray INTEGER NOT NULL CHECK (start_minimized_to_tray IN (0, 1))
            );

            PRAGMA user_version = 3;
            COMMIT;
            "#,
        )
        .map_err(|source| StorageError::Sqlite {
            path: path.to_path_buf(),
            source,
        })
}

pub(super) fn query_schema_version(
    connection: &Connection,
    path: &Path,
) -> Result<i64, StorageError> {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|source| StorageError::Sqlite {
            path: path.to_path_buf(),
            source,
        })
}
