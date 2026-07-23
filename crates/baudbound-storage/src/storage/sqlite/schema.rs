use std::{path::Path, time::Duration};

use rusqlite::Connection;

use crate::StorageError;

pub const CURRENT_SCHEMA_VERSION: i64 = 15;

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

    if version < CURRENT_SCHEMA_VERSION {
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

            CREATE TABLE IF NOT EXISTS update_check_cache (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                checked_at_unix INTEGER NOT NULL,
                latest_version TEXT NOT NULL,
                published_at TEXT,
                release_notes TEXT,
                update_available INTEGER NOT NULL CHECK (update_available IN (0, 1))
            );

            CREATE TABLE IF NOT EXISTS trigger_auth (
                script_id TEXT NOT NULL REFERENCES scripts(id) ON DELETE CASCADE,
                trigger_node_id TEXT NOT NULL,
                trigger_type TEXT NOT NULL CHECK (trigger_type IN ('webhook', 'websocket')),
                auth_enabled INTEGER NOT NULL CHECK (auth_enabled IN (0, 1)),
                token_hash BLOB NOT NULL CHECK (length(token_hash) = 32),
                token_preview TEXT NOT NULL,
                created_at_unix INTEGER NOT NULL,
                rotated_at_unix INTEGER,
                disabled_at_unix INTEGER,
                PRIMARY KEY (script_id, trigger_node_id)
            );

            CREATE INDEX IF NOT EXISTS trigger_auth_type_index
                ON trigger_auth(trigger_type, auth_enabled);

            PRAGMA user_version = 7;
            COMMIT;
            "#,
            )
            .map_err(|source| StorageError::Sqlite {
                path: path.to_path_buf(),
                source,
            })?;
    }

    if version < 8 {
        connection
            .execute_batch(
                r#"
                BEGIN;
                ALTER TABLE run_records
                    ADD COLUMN variable_scopes_json TEXT NOT NULL DEFAULT '{}';
                PRAGMA user_version = 8;
                COMMIT;
                "#,
            )
            .map_err(|source| StorageError::Sqlite {
                path: path.to_path_buf(),
                source,
            })?;
    }

    if version < 9 {
        connection
            .execute_batch(
                r#"
                BEGIN;
                ALTER TABLE run_records
                    ADD COLUMN has_errors INTEGER NOT NULL DEFAULT 0
                    CHECK (has_errors IN (0, 1));
                UPDATE run_records
                SET has_errors = CASE
                    WHEN status = 'failed' OR EXISTS (
                        SELECT 1
                        FROM json_each(run_records.logs_json) AS log
                        WHERE json_extract(log.value, '$.level') = 'error'
                    ) THEN 1
                    ELSE 0
                END;
                PRAGMA user_version = 9;
                COMMIT;
                "#,
            )
            .map_err(|source| StorageError::Sqlite {
                path: path.to_path_buf(),
                source,
            })?;
    }

    if version < 10 {
        connection
            .execute_batch(
                r#"
                BEGIN;
                CREATE TABLE IF NOT EXISTS script_update_state (
                    script_id TEXT PRIMARY KEY REFERENCES scripts(id) ON DELETE CASCADE,
                    automatic_checks_enabled INTEGER NOT NULL DEFAULT 0
                        CHECK (automatic_checks_enabled IN (0, 1)),
                    checked_update_url TEXT,
                    last_checked_at_unix INTEGER,
                    last_success_at_unix INTEGER,
                    latest_version TEXT,
                    package_url TEXT,
                    package_sha256 TEXT,
                    package_size INTEGER,
                    published_at TEXT,
                    release_notes TEXT,
                    last_error TEXT
                );
                PRAGMA user_version = 10;
                COMMIT;
                "#,
            )
            .map_err(|source| StorageError::Sqlite {
                path: path.to_path_buf(),
                source,
            })?;
    }

    if version < 11 {
        connection
            .execute_batch(
                r#"
                BEGIN;
                ALTER TABLE script_update_state
                    RENAME COLUMN checked_update_url TO checked_repository_url;
                PRAGMA user_version = 11;
                COMMIT;
                "#,
            )
            .map_err(|source| StorageError::Sqlite {
                path: path.to_path_buf(),
                source,
            })?;
    }

    if version < 12 {
        connection
            .execute_batch(
                r#"
                BEGIN;
                CREATE TABLE repository_sources (
                    url TEXT PRIMARY KEY,
                    name TEXT NOT NULL DEFAULT '',
                    description TEXT NOT NULL DEFAULT '',
                    homepage TEXT NOT NULL DEFAULT '',
                    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
                    official INTEGER NOT NULL DEFAULT 0 CHECK (official IN (0, 1)),
                    script_count INTEGER NOT NULL DEFAULT 0,
                    last_refresh_at_unix INTEGER,
                    last_success_at_unix INTEGER,
                    last_error TEXT,
                    revision INTEGER NOT NULL DEFAULT 0
                );
                CREATE TABLE repository_entries (
                    repository_url TEXT NOT NULL REFERENCES repository_sources(url) ON DELETE CASCADE,
                    script_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    author TEXT NOT NULL,
                    target_runtime TEXT NOT NULL,
                    risk_level TEXT NOT NULL,
                    version TEXT NOT NULL,
                    published_at TEXT NOT NULL,
                    entry_json TEXT NOT NULL,
                    PRIMARY KEY (repository_url, script_id)
                );
                CREATE INDEX repository_entries_name_index
                    ON repository_entries(name COLLATE NOCASE, script_id);
                CREATE INDEX repository_entries_repository_index
                    ON repository_entries(repository_url, name COLLATE NOCASE);
                PRAGMA user_version = 12;
                COMMIT;
                "#,
            )
            .map_err(|source| StorageError::Sqlite {
                path: path.to_path_buf(),
                source,
            })?;
    }

    if version < 13 {
        connection
            .execute_batch(
                r#"
                BEGIN;
                ALTER TABLE repository_sources ADD COLUMN etag TEXT;
                ALTER TABLE repository_sources ADD COLUMN last_modified TEXT;
                PRAGMA user_version = 13;
                COMMIT;
                "#,
            )
            .map_err(|source| StorageError::Sqlite {
                path: path.to_path_buf(),
                source,
            })?;
    }

    if version < 14 {
        connection
            .execute_batch(
                r#"
                BEGIN;
                ALTER TABLE repository_sources
                    ADD COLUMN information_mismatch_count INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE repository_entries ADD COLUMN information_mismatch TEXT;
                PRAGMA user_version = 14;
                COMMIT;
                "#,
            )
            .map_err(|source| StorageError::Sqlite {
                path: path.to_path_buf(),
                source,
            })?;
    }

    if version < 15 {
        connection
            .execute_batch(
                r#"
                BEGIN;
                ALTER TABLE repository_entries
                    ADD COLUMN information_mismatch_refresh_required INTEGER NOT NULL DEFAULT 0
                    CHECK (information_mismatch_refresh_required IN (0, 1));
                UPDATE repository_entries
                SET information_mismatch_refresh_required = 1
                WHERE information_mismatch IS NOT NULL;
                PRAGMA user_version = 15;
                COMMIT;
                "#,
            )
            .map_err(|source| StorageError::Sqlite {
                path: path.to_path_buf(),
                source,
            })?;
    }

    Ok(())
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
