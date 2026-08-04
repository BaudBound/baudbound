use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{OptionalExtension, params, types::Type};

use crate::{
    NewSystemLog, PaginatedRecords, SortDirection, StorageError, StoredSystemLog, SystemLogDetail,
    SystemLogQuery, SystemLogSeverity, SystemLogSort, SystemLogSummary,
};

use super::{SqliteRunnerStore, conversions::usize_to_sqlite};

const MAX_DETAILS: usize = 32;
const MAX_DETAILS_BYTES: usize = 128 * 1024;
const MAX_DETAIL_LABEL_BYTES: usize = 128;
const MAX_DETAIL_VALUE_BYTES: usize = 32 * 1024;
const MAX_MESSAGE_BYTES: usize = 8 * 1024;
const MAX_PAGE_SIZE: usize = 200;
const MAX_RETAINED_MESSAGES: usize = 1_000;
const MAX_SEARCH_BYTES: usize = 1_024;
const MAX_SOURCE_BYTES: usize = 128;
const MAX_TITLE_BYTES: usize = 256;

impl SqliteRunnerStore {
    pub fn append_system_log(
        &self,
        message: NewSystemLog,
    ) -> Result<StoredSystemLog, StorageError> {
        validate_message(&message)?;
        let id = random_message_id()?;
        let timestamp_unix_ms = current_timestamp_millis()?;
        let details_json =
            serde_json::to_string(&message.details).map_err(|source| StorageError::Json {
                path: self.path.clone(),
                source,
            })?;
        if details_json.len() > MAX_DETAILS_BYTES {
            return Err(StorageError::Operation(format!(
                "system log details must contain at most {MAX_DETAILS_BYTES} bytes"
            )));
        }
        let timestamp = i64::try_from(timestamp_unix_ms).map_err(|_| {
            StorageError::Operation("system log timestamp exceeds SQLite limits".to_owned())
        })?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction()
            .map_err(|source| self.sqlite_error(source))?;
        transaction
            .execute(
                r#"
                INSERT INTO system_logs (
                    id, timestamp_unix_ms, severity, source, title, message, details_json, unread
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)
                "#,
                params![
                    id,
                    timestamp,
                    severity_text(message.severity),
                    message.source,
                    message.title,
                    message.message,
                    details_json,
                ],
            )
            .map_err(|source| self.sqlite_error(source))?;
        transaction
            .execute(
                r#"
                DELETE FROM system_logs
                WHERE id IN (
                    SELECT id
                    FROM system_logs
                    ORDER BY timestamp_unix_ms DESC, id DESC
                    LIMIT -1 OFFSET ?1
                )
                "#,
                params![usize_to_sqlite(MAX_RETAINED_MESSAGES)?],
            )
            .map_err(|source| self.sqlite_error(source))?;
        transaction
            .commit()
            .map_err(|source| self.sqlite_error(source))?;

        Ok(StoredSystemLog {
            details: message.details,
            id,
            message: message.message,
            severity: message.severity,
            source: message.source,
            timestamp_unix_ms,
            title: message.title,
            unread: true,
        })
    }

    pub fn find_system_log(&self, id: &str) -> Result<Option<StoredSystemLog>, StorageError> {
        validate_text("system log id", id, 64, false)?;
        let connection = self.connection()?;
        connection
            .query_row(
                &format!("{SYSTEM_LOG_SELECT} WHERE id = ?1"),
                params![id],
                row_to_system_log,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn query_system_logs(
        &self,
        query: &SystemLogQuery,
    ) -> Result<PaginatedRecords<StoredSystemLog>, StorageError> {
        validate_text("system log search", &query.search, MAX_SEARCH_BYTES, true)?;
        let connection = self.connection()?;
        let search = query.search.trim();
        let severity = query.severity.map(severity_text).unwrap_or_default();
        let total = connection
            .query_row(
                &format!("SELECT COUNT(*) FROM system_logs {SYSTEM_LOG_FILTER}"),
                params![search, severity],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|source| self.sqlite_error(source))?;
        let sql = format!(
            "{SYSTEM_LOG_SELECT} {SYSTEM_LOG_FILTER} ORDER BY {} {}, id {} LIMIT ?3 OFFSET ?4",
            sort_expression(query.sort),
            direction_sql(query.direction),
            direction_sql(query.direction),
        );
        let limit = query.limit.clamp(1, MAX_PAGE_SIZE);
        let mut statement = connection
            .prepare(&sql)
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map(
                params![
                    search,
                    severity,
                    usize_to_sqlite(limit)?,
                    usize_to_sqlite(query.offset)?,
                ],
                row_to_system_log,
            )
            .map_err(|source| self.sqlite_error(source))?;
        let items = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| self.sqlite_error(source))?;
        Ok(PaginatedRecords {
            items,
            total: usize::try_from(total).unwrap_or(usize::MAX),
        })
    }

    pub fn system_log_summary(&self) -> Result<SystemLogSummary, StorageError> {
        let connection = self.connection()?;
        connection
            .query_row(
                r#"
                SELECT COUNT(*),
                    coalesce(SUM(CASE WHEN unread = 1 THEN 1 ELSE 0 END), 0),
                    coalesce(SUM(CASE WHEN unread = 1 AND severity = 'error' THEN 1 ELSE 0 END), 0),
                    coalesce(SUM(CASE WHEN unread = 1 AND severity = 'info' THEN 1 ELSE 0 END), 0),
                    coalesce(SUM(CASE WHEN unread = 1 AND severity = 'success' THEN 1 ELSE 0 END), 0),
                    coalesce(SUM(CASE WHEN unread = 1 AND severity = 'warning' THEN 1 ELSE 0 END), 0)
                FROM system_logs
                "#,
                [],
                |row| {
                    Ok(SystemLogSummary {
                        total: usize::try_from(row.get::<_, i64>(0)?).unwrap_or(usize::MAX),
                        unread: usize::try_from(row.get::<_, i64>(1)?).unwrap_or(usize::MAX),
                        unread_errors: usize::try_from(row.get::<_, i64>(2)?).unwrap_or(usize::MAX),
                        unread_info: usize::try_from(row.get::<_, i64>(3)?).unwrap_or(usize::MAX),
                        unread_successes: usize::try_from(row.get::<_, i64>(4)?).unwrap_or(usize::MAX),
                        unread_warnings: usize::try_from(row.get::<_, i64>(5)?).unwrap_or(usize::MAX),
                    })
                },
            )
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn mark_system_logs_read(&self) -> Result<usize, StorageError> {
        let connection = self.connection()?;
        connection
            .execute("UPDATE system_logs SET unread = 0 WHERE unread = 1", [])
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn clear_system_logs(&self) -> Result<usize, StorageError> {
        let connection = self.connection()?;
        connection
            .execute("DELETE FROM system_logs", [])
            .map_err(|source| self.sqlite_error(source))
    }
}

const SYSTEM_LOG_SELECT: &str = r#"
    SELECT id, timestamp_unix_ms, severity, source, title, message, details_json, unread
    FROM system_logs
"#;

const SYSTEM_LOG_FILTER: &str = r#"
    WHERE (?1 = '' OR instr(lower(source || char(10) || title || char(10) || message || char(10) || details_json), lower(?1)) > 0)
      AND (?2 = '' OR severity = ?2)
"#;

fn row_to_system_log(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredSystemLog> {
    let details_json = row.get::<_, String>(6)?;
    let details =
        serde_json::from_str::<Vec<SystemLogDetail>>(&details_json).map_err(|source| {
            rusqlite::Error::FromSqlConversionFailure(6, Type::Text, Box::new(source))
        })?;
    let severity_text = row.get::<_, String>(2)?;
    let severity = parse_severity(&severity_text).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            Type::Text,
            format!("invalid stored system log severity {severity_text:?}").into(),
        )
    })?;
    Ok(StoredSystemLog {
        details,
        id: row.get(0)?,
        timestamp_unix_ms: u64::try_from(row.get::<_, i64>(1)?).unwrap_or_default(),
        severity,
        source: row.get(3)?,
        title: row.get(4)?,
        message: row.get(5)?,
        unread: row.get::<_, i64>(7)? != 0,
    })
}

fn validate_message(message: &NewSystemLog) -> Result<(), StorageError> {
    validate_text(
        "system log source",
        &message.source,
        MAX_SOURCE_BYTES,
        false,
    )?;
    validate_text("system log title", &message.title, MAX_TITLE_BYTES, false)?;
    validate_text(
        "system log message",
        &message.message,
        MAX_MESSAGE_BYTES,
        false,
    )?;
    if message.details.len() > MAX_DETAILS {
        return Err(StorageError::Operation(format!(
            "system logs may contain at most {MAX_DETAILS} detail fields"
        )));
    }
    for detail in &message.details {
        validate_text(
            "system log detail label",
            &detail.label,
            MAX_DETAIL_LABEL_BYTES,
            false,
        )?;
        validate_text(
            "system log detail value",
            &detail.value,
            MAX_DETAIL_VALUE_BYTES,
            true,
        )?;
    }
    Ok(())
}

fn validate_text(
    label: &str,
    value: &str,
    max_bytes: usize,
    allow_empty: bool,
) -> Result<(), StorageError> {
    if (!allow_empty && value.trim().is_empty()) || value.len() > max_bytes || value.contains('\0')
    {
        return Err(StorageError::Operation(format!(
            "{label} must {}contain at most {max_bytes} bytes and no null characters",
            if allow_empty { "" } else { "not be empty, " }
        )));
    }
    Ok(())
}

fn random_message_id() -> Result<String, StorageError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|source| {
        StorageError::Operation(format!(
            "could not generate a system log identifier: {source}"
        ))
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn current_timestamp_millis() -> Result<u64, StorageError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .map_err(|source| {
            StorageError::Operation(format!("system clock is before UNIX epoch: {source}"))
        })
}

fn severity_text(severity: SystemLogSeverity) -> &'static str {
    match severity {
        SystemLogSeverity::Error => "error",
        SystemLogSeverity::Info => "info",
        SystemLogSeverity::Success => "success",
        SystemLogSeverity::Warning => "warning",
    }
}

fn parse_severity(value: &str) -> Option<SystemLogSeverity> {
    match value {
        "error" => Some(SystemLogSeverity::Error),
        "info" => Some(SystemLogSeverity::Info),
        "success" => Some(SystemLogSeverity::Success),
        "warning" => Some(SystemLogSeverity::Warning),
        _ => None,
    }
}

fn sort_expression(sort: SystemLogSort) -> &'static str {
    match sort {
        SystemLogSort::Message => "message COLLATE NOCASE",
        SystemLogSort::Severity => "severity COLLATE NOCASE",
        SystemLogSort::Source => "source COLLATE NOCASE",
        SystemLogSort::Time => "timestamp_unix_ms",
        SystemLogSort::Title => "title COLLATE NOCASE",
    }
}

fn direction_sql(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Ascending => "ASC",
        SortDirection::Descending => "DESC",
    }
}
