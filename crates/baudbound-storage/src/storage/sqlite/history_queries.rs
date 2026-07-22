use rusqlite::{params, types::Type};

use crate::{
    PaginatedRecords, RunHistoryQuery, RunHistorySort, RunLogQuery, RunLogSort, SortDirection,
    StorageError, StoredRunLogRecord, StoredRunRecord, StoredVariableRecord,
};

use super::{SqliteRunnerStore, conversions::usize_to_sqlite, rows::row_to_run_record};

const MAX_PAGE_SIZE: usize = 200;
const MAX_SEARCH_BYTES: usize = 1024;
const MAX_FILTER_BYTES: usize = 256;

impl SqliteRunnerStore {
    pub fn find_run_record_by_id(
        &self,
        run_id: &str,
    ) -> Result<Option<StoredRunRecord>, StorageError> {
        use rusqlite::OptionalExtension;

        let connection = self.connection()?;
        connection
            .query_row(
                r#"
                SELECT run_id, script_id, status, trigger_node_id, completed_at_unix,
                    logs_json, variables_json, variable_scopes_json
                FROM run_records
                WHERE run_id = ?1
                "#,
                params![run_id],
                row_to_run_record,
            )
            .optional()
            .map_err(|source| self.sqlite_error(source))
    }

    pub fn query_run_history(
        &self,
        query: &RunHistoryQuery,
    ) -> Result<PaginatedRecords<StoredRunRecord>, StorageError> {
        validate_query_text("run search", &query.search, MAX_SEARCH_BYTES)?;
        if let Some(script_id) = &query.script_id {
            validate_query_text("script filter", script_id, MAX_FILTER_BYTES)?;
        }
        if let Some(status) = &query.status {
            validate_query_text("status filter", status, MAX_FILTER_BYTES)?;
            if !matches!(status.as_str(), "completed" | "failed" | "cancelled") {
                return Err(StorageError::Operation(format!(
                    "unsupported run status filter {status:?}"
                )));
            }
        }
        let connection = self.connection()?;
        let search = query.search.trim();
        let script_id = query.script_id.as_deref().unwrap_or("");
        let status = query.status.as_deref().unwrap_or("");
        let count_sql = format!("SELECT COUNT(*) {RUN_FILTER_SQL}");
        let total = connection
            .query_row(&count_sql, params![search, script_id, status], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|source| self.sqlite_error(source))?;
        let sql = format!(
            "SELECT run.run_id, run.script_id, run.status, run.trigger_node_id, \
             run.completed_at_unix, run.logs_json, run.variables_json, \
             run.variable_scopes_json {RUN_FILTER_SQL} ORDER BY {} {}, run.run_id ASC LIMIT ?4 OFFSET ?5",
            run_sort_expression(query.sort),
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
                    script_id,
                    status,
                    usize_to_sqlite(limit)?,
                    usize_to_sqlite(query.offset)?,
                ],
                row_to_run_record,
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

    pub fn query_run_logs(
        &self,
        query: &RunLogQuery,
    ) -> Result<PaginatedRecords<StoredRunLogRecord>, StorageError> {
        validate_query_text("log search", &query.search, MAX_SEARCH_BYTES)?;
        let connection = self.connection()?;
        let search = query.search.trim();
        let count_sql = format!("SELECT COUNT(*) {LOG_FROM_SQL}");
        let total = connection
            .query_row(&count_sql, params![search], |row| row.get::<_, i64>(0))
            .map_err(|source| self.sqlite_error(source))?;
        let sql = format!(
            "SELECT run.run_id, run.script_id, coalesce(script.name, run.script_id), CAST(log.key AS INTEGER), \
             coalesce(json_extract(log.value, '$.timestamp_unix_ms'), run.completed_at_unix * 1000), \
             coalesce(json_extract(log.value, '$.level'), ''), json_extract(log.value, '$.node_id'), \
             json_extract(log.value, '$.action_type'), coalesce(json_extract(log.value, '$.message'), '') \
             {LOG_FROM_SQL} ORDER BY {} {}, run.run_id ASC, CAST(log.key AS INTEGER) ASC LIMIT ?2 OFFSET ?3",
            log_sort_expression(query.sort),
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
                    usize_to_sqlite(limit)?,
                    usize_to_sqlite(query.offset)?,
                ],
                row_to_log_record,
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

    pub fn list_stored_variables(&self) -> Result<Vec<StoredVariableRecord>, StorageError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT variable.name, 'persistent', variable.script_id, scripts.name,
                    variable.value_json, variable.version, variable.updated_at_unix
                FROM persistent_variables AS variable
                JOIN scripts ON scripts.id = variable.script_id
                UNION ALL
                SELECT name, 'global', NULL, NULL, value_json, version, updated_at_unix
                FROM global_variables
                ORDER BY 2, 4, 1
                "#,
            )
            .map_err(|source| self.sqlite_error(source))?;
        let rows = statement
            .query_map([], |row| {
                let value_json = row.get::<_, String>(4)?;
                let value = serde_json::from_str(&value_json).map_err(|source| {
                    rusqlite::Error::FromSqlConversionFailure(4, Type::Text, Box::new(source))
                })?;
                Ok(StoredVariableRecord {
                    name: row.get(0)?,
                    scope: row.get(1)?,
                    script_id: row.get(2)?,
                    script_name: row.get(3)?,
                    value,
                    version: u64::try_from(row.get::<_, i64>(5)?).unwrap_or_default(),
                    updated_at_unix: u64::try_from(row.get::<_, i64>(6)?).unwrap_or_default(),
                })
            })
            .map_err(|source| self.sqlite_error(source))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|source| self.sqlite_error(source))
    }

    fn sqlite_error(&self, source: rusqlite::Error) -> StorageError {
        StorageError::Sqlite {
            path: self.path.clone(),
            source,
        }
    }
}

fn validate_query_text(label: &str, value: &str, max_bytes: usize) -> Result<(), StorageError> {
    if value.len() > max_bytes || value.chars().any(|character| character == '\0') {
        return Err(StorageError::Operation(format!(
            "{label} must contain at most {max_bytes} bytes and no null characters"
        )));
    }
    Ok(())
}

const RUN_FILTER_SQL: &str = r#"
    FROM run_records AS run
    LEFT JOIN scripts AS script ON script.id = run.script_id
    WHERE (?1 = '' OR instr(lower(run.run_id || char(10) || run.script_id || char(10) ||
        coalesce(script.name, '') || char(10) || run.status || char(10) || run.trigger_node_id || char(10) ||
        run.logs_json), lower(?1)) > 0)
      AND (?2 = '' OR run.script_id = ?2)
      AND (?3 = '' OR run.status = ?3)
"#;

const LOG_FROM_SQL: &str = r#"
    FROM run_records AS run
    LEFT JOIN scripts AS script ON script.id = run.script_id
    JOIN json_each(run.logs_json) AS log
    WHERE (?1 = '' OR instr(lower(run.run_id || char(10) || run.script_id || char(10) ||
        coalesce(script.name, '') || char(10) || coalesce(json_extract(log.value, '$.level'), '') || char(10) ||
        coalesce(json_extract(log.value, '$.message'), '') || char(10) ||
        coalesce(json_extract(log.value, '$.node_id'), '') || char(10) ||
        coalesce(json_extract(log.value, '$.action_type'), '')), lower(?1)) > 0)
"#;
fn row_to_log_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredRunLogRecord> {
    Ok(StoredRunLogRecord {
        run_id: row.get(0)?,
        script_id: row.get(1)?,
        script_name: row.get(2)?,
        log_index: usize::try_from(row.get::<_, i64>(3)?).unwrap_or_default(),
        timestamp_unix_ms: u64::try_from(row.get::<_, i64>(4)?).unwrap_or_default(),
        level: row.get(5)?,
        node_id: row.get(6)?,
        action_type: row.get(7)?,
        message: row.get(8)?,
    })
}

fn direction_sql(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Ascending => "ASC",
        SortDirection::Descending => "DESC",
    }
}

fn run_sort_expression(sort: RunHistorySort) -> &'static str {
    match sort {
        RunHistorySort::Completed => "run.completed_at_unix",
        RunHistorySort::RecentLog => "json_extract(run.logs_json, '$[#-1].message')",
        RunHistorySort::RunId => "run.run_id",
        RunHistorySort::Script => "coalesce(script.name, run.script_id) COLLATE NOCASE",
        RunHistorySort::Status => "run.status",
        RunHistorySort::Trigger => "run.trigger_node_id",
        RunHistorySort::TriggerType => {
            "(SELECT json_extract(value, '$.action_type') FROM json_each(run.logs_json) WHERE json_extract(value, '$.node_id') = run.trigger_node_id LIMIT 1)"
        }
    }
}

fn log_sort_expression(sort: RunLogSort) -> &'static str {
    match sort {
        RunLogSort::Level => "json_extract(log.value, '$.level')",
        RunLogSort::Message => "json_extract(log.value, '$.message') COLLATE NOCASE",
        RunLogSort::Node => "json_extract(log.value, '$.node_id')",
        RunLogSort::Run => "run.run_id",
        RunLogSort::Script => "coalesce(script.name, run.script_id) COLLATE NOCASE",
        RunLogSort::Time => "json_extract(log.value, '$.timestamp_unix_ms')",
        RunLogSort::Type => "json_extract(log.value, '$.action_type')",
    }
}
