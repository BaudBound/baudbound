use std::{path::Path, path::PathBuf};

use rusqlite::{Connection, OptionalExtension, Row, params, types::Type};

use crate::{InstalledScript, RunLogEntry, ScriptApproval, StorageError, StoredRunRecord};

use super::conversions::{row_i64_to_bool, row_i64_to_u32, row_i64_to_u64, row_i64_to_usize};

pub(super) fn resolve_script(
    connection: &Connection,
    database_path: &Path,
    reference: &str,
) -> Result<InstalledScript, StorageError> {
    if let Some(script) = connection
        .query_row(
            r#"
            SELECT id, enabled, name, package_hash, package_file_name, package_path,
                imported_at_unix, package_format_version, script_language_version,
                target_runtime, asset_count, risk_level
            FROM scripts
            WHERE id = ?1
            "#,
            params![reference],
            row_to_installed_script,
        )
        .optional()
        .map_err(|source| StorageError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?
    {
        return Ok(script);
    }

    let normalized_reference = reference.to_lowercase();
    let mut statement = connection
        .prepare(
            r#"
            SELECT id, enabled, name, package_hash, package_file_name, package_path,
                imported_at_unix, package_format_version, script_language_version,
                target_runtime, asset_count, risk_level
            FROM scripts
            WHERE lower(name) = ?1
            ORDER BY id
            "#,
        )
        .map_err(|source| StorageError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;
    let rows = statement
        .query_map(params![normalized_reference], row_to_installed_script)
        .map_err(|source| StorageError::Sqlite {
            path: database_path.to_path_buf(),
            source,
        })?;
    let mut matches =
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|source| StorageError::Sqlite {
                path: database_path.to_path_buf(),
                source,
            })?;

    match matches.len() {
        0 => Err(StorageError::NotFound(reference.to_owned())),
        1 => Ok(matches.remove(0)),
        _ => Err(StorageError::Operation(format!(
            "{reference:?} matches multiple scripts; use the script id"
        ))),
    }
}

pub(super) fn row_to_installed_script(row: &Row<'_>) -> rusqlite::Result<InstalledScript> {
    Ok(InstalledScript {
        id: row.get(0)?,
        enabled: row_i64_to_bool(1, row.get(1)?)?,
        name: row.get(2)?,
        package_hash: row.get(3)?,
        package_file_name: row.get(4)?,
        package_path: PathBuf::from(row.get::<_, String>(5)?),
        imported_at_unix: row_i64_to_u64(6, row.get(6)?)?,
        package_format_version: row_i64_to_u32(7, row.get(7)?)?,
        script_language_version: row_i64_to_u32(8, row.get(8)?)?,
        target_runtime: row.get(9)?,
        asset_count: row_i64_to_usize(10, row.get(10)?)?,
        risk_level: row.get(11)?,
    })
}

pub(super) fn row_to_approval(row: &Row<'_>) -> rusqlite::Result<ScriptApproval> {
    let permissions_json = row.get::<_, String>(2)?;
    let approved_permissions =
        serde_json::from_str::<Vec<String>>(&permissions_json).map_err(|source| {
            rusqlite::Error::FromSqlConversionFailure(2, Type::Text, Box::new(source))
        })?;
    Ok(ScriptApproval {
        script_id: row.get(0)?,
        package_hash: row.get(1)?,
        approved_permissions,
        approved_at_unix: row_i64_to_u64(3, row.get(3)?)?,
    })
}

pub(super) fn row_to_run_record(row: &Row<'_>) -> rusqlite::Result<StoredRunRecord> {
    let logs_json = row.get::<_, String>(5)?;
    let variables_json = row.get::<_, String>(6)?;
    let completed_at_unix = row_i64_to_u64(4, row.get(4)?)?;
    let mut logs = serde_json::from_str::<Vec<RunLogEntry>>(&logs_json).map_err(|source| {
        rusqlite::Error::FromSqlConversionFailure(5, Type::Text, Box::new(source))
    })?;
    let fallback_timestamp_unix_ms = completed_at_unix.saturating_mul(1_000);
    for log in &mut logs {
        if log.timestamp_unix_ms == 0 {
            log.timestamp_unix_ms = fallback_timestamp_unix_ms;
        }
    }
    let variables = serde_json::from_str(&variables_json).map_err(|source| {
        rusqlite::Error::FromSqlConversionFailure(6, Type::Text, Box::new(source))
    })?;
    Ok(StoredRunRecord {
        run_id: row.get(0)?,
        script_id: row.get(1)?,
        status: row.get(2)?,
        trigger_node_id: row.get(3)?,
        completed_at_unix,
        logs,
        variables,
    })
}
