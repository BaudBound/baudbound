use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, TransactionBehavior, params};

use crate::{StorageError, StoredRunRecord};

use super::conversions::{u64_to_sqlite, usize_to_sqlite};

pub const DEFAULT_MAX_RUN_RECORDS: usize = 10_000;
pub const DEFAULT_MAX_RUN_AGE_DAYS: u64 = 30;
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunRetentionPolicy {
    pub max_age_days: u64,
    pub max_records: usize,
}

impl RunRetentionPolicy {
    #[must_use]
    pub const fn new(max_records: usize, max_age_days: u64) -> Self {
        Self {
            max_age_days,
            max_records,
        }
    }

    pub fn validate(self) -> Result<(), StorageError> {
        if self.max_records == 0 {
            return Err(StorageError::Operation(
                "run retention max_records must be greater than zero".to_owned(),
            ));
        }
        if self.max_age_days == 0 {
            return Err(StorageError::Operation(
                "run retention max_age_days must be greater than zero".to_owned(),
            ));
        }
        usize_to_sqlite(self.max_records)?;
        self.max_age_days
            .checked_mul(SECONDS_PER_DAY)
            .ok_or_else(|| {
                StorageError::Operation("run retention max_age_days is too large".to_owned())
            })?;
        Ok(())
    }
}

impl Default for RunRetentionPolicy {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_RUN_RECORDS, DEFAULT_MAX_RUN_AGE_DAYS)
    }
}

pub(super) fn append_run_record_with_retention(
    connection: &mut Connection,
    database_path: &Path,
    record: &StoredRunRecord,
    logs_json: &str,
    variables_json: &str,
    policy: RunRetentionPolicy,
) -> Result<(), StorageError> {
    policy.validate()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|source| sqlite_error(database_path, source))?;
    transaction
        .execute(
            r#"
            INSERT INTO run_records (
                run_id, script_id, status, trigger_node_id,
                completed_at_unix, logs_json, variables_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                record.run_id,
                record.script_id,
                record.status,
                record.trigger_node_id,
                u64_to_sqlite(record.completed_at_unix)?,
                logs_json,
                variables_json,
            ],
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    prune_transaction(&transaction, database_path, policy)?;
    transaction
        .commit()
        .map_err(|source| sqlite_error(database_path, source))
}

pub(super) fn prune_run_records(
    connection: &mut Connection,
    database_path: &Path,
    policy: RunRetentionPolicy,
) -> Result<usize, StorageError> {
    policy.validate()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|source| sqlite_error(database_path, source))?;
    let deleted = prune_transaction(&transaction, database_path, policy)?;
    transaction
        .commit()
        .map_err(|source| sqlite_error(database_path, source))?;
    Ok(deleted)
}

fn prune_transaction(
    transaction: &rusqlite::Transaction<'_>,
    database_path: &Path,
    policy: RunRetentionPolicy,
) -> Result<usize, StorageError> {
    let max_age_seconds = policy
        .max_age_days
        .checked_mul(SECONDS_PER_DAY)
        .ok_or_else(|| StorageError::Operation("run retention age overflowed".to_owned()))?;
    let cutoff = current_unix_timestamp().saturating_sub(max_age_seconds);
    let expired = transaction
        .execute(
            "DELETE FROM run_records WHERE completed_at_unix < ?1",
            params![u64_to_sqlite(cutoff)?],
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    let excess = transaction
        .execute(
            r#"
            DELETE FROM run_records
            WHERE run_id IN (
                SELECT run_id
                FROM run_records
                ORDER BY completed_at_unix DESC, rowid DESC
                LIMIT -1 OFFSET ?1
            )
            "#,
            params![usize_to_sqlite(policy.max_records)?],
        )
        .map_err(|source| sqlite_error(database_path, source))?;
    Ok(expired.saturating_add(excess))
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn sqlite_error(database_path: &Path, source: rusqlite::Error) -> StorageError {
    StorageError::Sqlite {
        path: database_path.to_path_buf(),
        source,
    }
}
