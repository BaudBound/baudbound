use std::collections::BTreeMap;

use rusqlite::{OptionalExtension, params, params_from_iter, types::Value};

use crate::{
    PaginatedRecords, RepositoryCacheReplacement, RepositoryScriptQuery, RepositoryScriptRecord,
    RepositoryScriptSort, RepositoryScriptSummary, RepositorySource, SortDirection, StorageError,
};

use super::{
    SqliteRunnerStore,
    conversions::{row_i64_to_bool, row_i64_to_usize, usize_to_sqlite},
};

impl SqliteRunnerStore {
    pub fn ensure_repository_source(
        &self,
        url: &str,
        official: bool,
    ) -> Result<RepositorySource, StorageError> {
        let connection = self.connection()?;
        connection
            .execute(
                r#"
                INSERT INTO repository_sources (url, official)
                VALUES (?1, ?2)
                ON CONFLICT(url) DO UPDATE SET
                    official = CASE WHEN excluded.official = 1 THEN 1 ELSE repository_sources.official END
                "#,
                params![url, i64::from(official)],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        drop(connection);
        self.repository_source(url)?
            .ok_or_else(|| StorageError::Operation("repository source was not stored".to_owned()))
    }

    pub fn repository_source(&self, url: &str) -> Result<Option<RepositorySource>, StorageError> {
        let connection = self.connection()?;
        connection
            .query_row(
                r#"
                SELECT url, name, description, homepage, enabled, official, script_count,
                    last_refresh_at_unix, last_success_at_unix, last_error, revision,
                    etag, last_modified, information_mismatch_count
                FROM repository_sources
                WHERE url = ?1
                "#,
                params![url],
                row_to_repository_source,
            )
            .optional()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })
    }

    pub fn list_repository_sources(&self) -> Result<Vec<RepositorySource>, StorageError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT url, name, description, homepage, enabled, official, script_count,
                    last_refresh_at_unix, last_success_at_unix, last_error, revision,
                    etag, last_modified, information_mismatch_count
                FROM repository_sources
                ORDER BY official DESC, name COLLATE NOCASE, url
                "#,
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        let rows = statement
            .query_map([], row_to_repository_source)
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

    pub fn set_repository_enabled(
        &self,
        url: &str,
        enabled: bool,
    ) -> Result<RepositorySource, StorageError> {
        let connection = self.connection()?;
        let changed = connection
            .execute(
                "UPDATE repository_sources SET enabled = ?2, revision = revision + 1 WHERE url = ?1",
                params![url, i64::from(enabled)],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        if changed == 0 {
            return Err(StorageError::Operation(
                "repository source was not found".to_owned(),
            ));
        }
        drop(connection);
        self.repository_source(url)?
            .ok_or_else(|| StorageError::Operation("repository source was not found".to_owned()))
    }

    pub fn remove_repository_source(&self, url: &str) -> Result<bool, StorageError> {
        let connection = self.connection()?;
        let official = connection
            .query_row(
                "SELECT official FROM repository_sources WHERE url = ?1",
                params![url],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        if official == Some(1) {
            return Err(StorageError::Operation(
                "the official repository can be disabled but not removed".to_owned(),
            ));
        }
        connection
            .execute(
                "DELETE FROM repository_sources WHERE url = ?1",
                params![url],
            )
            .map(|changed| changed > 0)
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })
    }

    pub fn replace_repository_cache(
        &self,
        replacement: &RepositoryCacheReplacement,
    ) -> Result<RepositorySource, StorageError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        let existing_mismatches = {
            let mut statement = transaction
                .prepare(
                    r#"
                    SELECT script_id, information_mismatch
                    FROM repository_entries
                    WHERE repository_url = ?1 AND information_mismatch IS NOT NULL
                    "#,
                )
                .map_err(|source| StorageError::Sqlite {
                    path: self.path.clone(),
                    source,
                })?;
            let rows = statement
                .query_map(params![replacement.url], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|source| StorageError::Sqlite {
                    path: self.path.clone(),
                    source,
                })?;
            rows.collect::<Result<BTreeMap<_, _>, _>>()
                .map_err(|source| StorageError::Sqlite {
                    path: self.path.clone(),
                    source,
                })?
        };
        transaction
            .execute(
                r#"
                UPDATE repository_sources SET
                    name = ?2,
                    description = ?3,
                    homepage = ?4,
                    script_count = ?5,
                    last_refresh_at_unix = ?6,
                    last_success_at_unix = ?6,
                    last_error = NULL,
                    etag = ?7,
                    last_modified = ?8,
                    revision = revision + 1
                WHERE url = ?1
                "#,
                params![
                    replacement.url,
                    replacement.name,
                    replacement.description,
                    replacement.homepage,
                    usize_to_sqlite(replacement.entries.len())?,
                    u64_to_sqlite(replacement.refreshed_at_unix)?,
                    replacement.etag,
                    replacement.last_modified,
                ],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        transaction
            .execute(
                "DELETE FROM repository_entries WHERE repository_url = ?1",
                params![replacement.url],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        for entry in &replacement.entries {
            let information_mismatch = existing_mismatches.get(&entry.script_id);
            transaction
                .execute(
                    r#"
                    INSERT INTO repository_entries (
                        repository_url, script_id, name, summary, author, target_runtime,
                        risk_level, version, published_at, entry_json, information_mismatch,
                        information_mismatch_refresh_required
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0)
                    "#,
                    params![
                        replacement.url,
                        entry.script_id,
                        entry.name,
                        entry.summary,
                        entry.author,
                        entry.target_runtime,
                        entry.risk_level,
                        entry.version,
                        entry.published_at,
                        entry.entry_json,
                        information_mismatch,
                    ],
                )
                .map_err(|source| StorageError::Sqlite {
                    path: self.path.clone(),
                    source,
                })?;
        }
        transaction
            .execute(
                r#"
                UPDATE repository_sources
                SET information_mismatch_count = (
                    SELECT COUNT(*) FROM repository_entries
                    WHERE repository_url = ?1 AND information_mismatch IS NOT NULL
                )
                WHERE url = ?1
                "#,
                params![replacement.url],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        transaction
            .commit()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        drop(connection);
        self.repository_source(&replacement.url)?
            .ok_or_else(|| StorageError::Operation("repository source was not found".to_owned()))
    }

    pub fn record_repository_refresh_failure(
        &self,
        url: &str,
        refreshed_at_unix: u64,
        error: &str,
    ) -> Result<RepositorySource, StorageError> {
        let connection = self.connection()?;
        connection
            .execute(
                r#"
                UPDATE repository_sources SET
                    last_refresh_at_unix = ?2,
                    last_error = ?3,
                    revision = revision + 1
                WHERE url = ?1
                "#,
                params![url, u64_to_sqlite(refreshed_at_unix)?, error],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        drop(connection);
        self.repository_source(url)?
            .ok_or_else(|| StorageError::Operation("repository source was not found".to_owned()))
    }

    pub fn record_repository_not_modified(
        &self,
        url: &str,
        refreshed_at_unix: u64,
    ) -> Result<RepositorySource, StorageError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        let changed = transaction
            .execute(
                r#"
                UPDATE repository_sources SET
                    last_refresh_at_unix = ?2,
                    last_success_at_unix = ?2,
                    last_error = NULL,
                    revision = revision + 1
                WHERE url = ?1
                "#,
                params![url, u64_to_sqlite(refreshed_at_unix)?],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        if changed == 0 {
            return Err(StorageError::Operation(
                "repository source was not found".to_owned(),
            ));
        }
        transaction
            .execute(
                r#"
                UPDATE repository_entries
                SET information_mismatch_refresh_required = 0
                WHERE repository_url = ?1 AND information_mismatch IS NOT NULL
                "#,
                params![url],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        transaction
            .commit()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        drop(connection);
        self.repository_source(url)?
            .ok_or_else(|| StorageError::Operation("repository source was not found".to_owned()))
    }

    pub fn query_repository_scripts(
        &self,
        query: &RepositoryScriptQuery,
    ) -> Result<PaginatedRecords<RepositoryScriptSummary>, StorageError> {
        let limit = query.limit.clamp(1, 100);
        let mut conditions = vec!["repository_sources.enabled = 1".to_owned()];
        let mut values = Vec::<Value>::new();
        if !query.search.trim().is_empty() {
            conditions.push(
                "(repository_entries.name LIKE ? OR repository_entries.summary LIKE ? OR repository_entries.author LIKE ? OR repository_entries.entry_json LIKE ?)"
                    .to_owned(),
            );
            let search = format!("%{}%", query.search.trim());
            values.extend([
                Value::Text(search.clone()),
                Value::Text(search.clone()),
                Value::Text(search.clone()),
                Value::Text(search),
            ]);
        }
        add_text_filters(
            &mut conditions,
            &mut values,
            "repository_entries.repository_url",
            &query.repository_urls,
        );
        add_text_filters(
            &mut conditions,
            &mut values,
            "repository_entries.risk_level",
            &query.risk_levels,
        );
        add_json_array_filters(
            &mut conditions,
            &mut values,
            "$.target_runtimes",
            &query.target_runtimes,
        );
        add_json_array_filters(
            &mut conditions,
            &mut values,
            "$.permissions",
            &query.permissions,
        );
        add_json_array_filters(
            &mut conditions,
            &mut values,
            "$.capabilities",
            &query.capabilities,
        );
        let includes_installed = query.installed.contains(&true);
        let includes_not_installed = query.installed.contains(&false);
        if includes_installed != includes_not_installed {
            conditions.push(if includes_installed {
                "scripts.id IS NOT NULL".to_owned()
            } else {
                "scripts.id IS NULL".to_owned()
            });
        }
        let where_clause = conditions.join(" AND ");
        let connection = self.connection()?;
        let count_sql = format!(
            "SELECT COUNT(*) FROM repository_entries JOIN repository_sources ON repository_sources.url = repository_entries.repository_url LEFT JOIN scripts ON scripts.id = repository_entries.script_id WHERE {where_clause}"
        );
        let total = connection
            .query_row(&count_sql, params_from_iter(values.iter()), |row| {
                row_i64_to_usize(0, row.get(0)?)
            })
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;

        let order = repository_order(query.sort);
        let direction = match query.direction {
            SortDirection::Ascending => "ASC",
            SortDirection::Descending => "DESC",
        };
        let sql = format!(
            r#"
            SELECT repository_entries.repository_url, repository_sources.name,
                repository_sources.official, repository_entries.script_id,
                repository_entries.name, repository_entries.summary,
                repository_entries.author, repository_entries.target_runtime,
                repository_entries.risk_level, repository_entries.version,
                repository_entries.published_at, repository_entries.entry_json,
                repository_entries.information_mismatch,
                repository_entries.information_mismatch_refresh_required,
                scripts.id IS NOT NULL
            FROM repository_entries
            JOIN repository_sources ON repository_sources.url = repository_entries.repository_url
            LEFT JOIN scripts ON scripts.id = repository_entries.script_id
            WHERE {where_clause}
            ORDER BY {order} {direction}, repository_entries.script_id ASC
            LIMIT ? OFFSET ?
            "#
        );
        let mut page_values = values;
        page_values.push(Value::Integer(usize_to_sqlite(limit)?));
        page_values.push(Value::Integer(usize_to_sqlite(query.offset)?));
        let mut statement = connection
            .prepare(&sql)
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        let rows = statement
            .query_map(params_from_iter(page_values.iter()), |row| {
                let entry_json = row.get::<_, String>(11)?;
                let entry =
                    serde_json::from_str::<serde_json::Value>(&entry_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            11,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                let minimum_runner_version = entry
                    .get("minimum_runner_version")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        rusqlite::Error::FromSqlConversionFailure(
                            11,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "repository entry is missing minimum_runner_version",
                            )),
                        )
                    })?
                    .to_owned();
                let target_runtimes = entry
                    .get("target_runtimes")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| {
                        rusqlite::Error::FromSqlConversionFailure(
                            11,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "repository entry is missing target_runtimes",
                            )),
                        )
                    })?
                    .iter()
                    .map(|value| {
                        value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                            rusqlite::Error::FromSqlConversionFailure(
                                11,
                                rusqlite::types::Type::Text,
                                Box::new(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    "repository entry contains an invalid target runtime",
                                )),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(RepositoryScriptSummary {
                    repository_url: row.get(0)?,
                    repository_name: row.get(1)?,
                    official: row_i64_to_bool(2, row.get(2)?)?,
                    script_id: row.get(3)?,
                    name: row.get(4)?,
                    summary: row.get(5)?,
                    author: row.get(6)?,
                    target_runtimes,
                    risk_level: row.get(8)?,
                    version: row.get(9)?,
                    published_at: row.get(10)?,
                    minimum_runner_version,
                    information_mismatch: row.get(12)?,
                    information_mismatch_refresh_required: row_i64_to_bool(13, row.get(13)?)?,
                    installed: row_i64_to_bool(14, row.get(14)?)?,
                })
            })
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        let items = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(PaginatedRecords { items, total })
    }

    pub fn repository_script(
        &self,
        repository_url: &str,
        script_id: &str,
    ) -> Result<Option<RepositoryScriptRecord>, StorageError> {
        let connection = self.connection()?;
        connection
            .query_row(
                r#"
                SELECT repository_entries.repository_url, repository_sources.name,
                    repository_sources.official, repository_entries.script_id,
                    repository_entries.name, repository_entries.summary,
                    repository_entries.author, repository_entries.target_runtime,
                    repository_entries.risk_level, repository_entries.version,
                    repository_entries.published_at, repository_entries.entry_json,
                    repository_entries.information_mismatch,
                    repository_entries.information_mismatch_refresh_required,
                    scripts.id IS NOT NULL
                FROM repository_entries
                JOIN repository_sources
                    ON repository_sources.url = repository_entries.repository_url
                LEFT JOIN scripts ON scripts.id = repository_entries.script_id
                WHERE repository_entries.repository_url = ?1
                    AND repository_entries.script_id = ?2
                "#,
                params![repository_url, script_id],
                row_to_repository_script,
            )
            .optional()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })
    }

    pub fn record_repository_information_mismatch(
        &self,
        repository_url: &str,
        script_id: &str,
        message: &str,
    ) -> Result<RepositorySource, StorageError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        let changed = transaction
            .execute(
                r#"
                UPDATE repository_entries
                SET information_mismatch = ?3,
                    information_mismatch_refresh_required = 1
                WHERE repository_url = ?1 AND script_id = ?2
                "#,
                params![repository_url, script_id, message],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        if changed == 0 {
            return Err(StorageError::Operation(
                "repository script was not found".to_owned(),
            ));
        }
        transaction
            .execute(
                r#"
                UPDATE repository_sources
                SET information_mismatch_count = (
                    SELECT COUNT(*) FROM repository_entries
                    WHERE repository_url = ?1 AND information_mismatch IS NOT NULL
                ),
                revision = revision + 1
                WHERE url = ?1
                "#,
                params![repository_url],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        transaction
            .commit()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        drop(connection);
        self.repository_source(repository_url)?
            .ok_or_else(|| StorageError::Operation("repository source was not found".to_owned()))
    }

    pub fn clear_repository_information_mismatch(
        &self,
        repository_url: &str,
        script_id: &str,
    ) -> Result<RepositorySource, StorageError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        transaction
            .execute(
                r#"
                UPDATE repository_entries
                SET information_mismatch = NULL,
                    information_mismatch_refresh_required = 0
                WHERE repository_url = ?1 AND script_id = ?2
                "#,
                params![repository_url, script_id],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        transaction
            .execute(
                r#"
                UPDATE repository_sources
                SET information_mismatch_count = (
                    SELECT COUNT(*) FROM repository_entries
                    WHERE repository_url = ?1 AND information_mismatch IS NOT NULL
                ),
                revision = revision + 1
                WHERE url = ?1
                "#,
                params![repository_url],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        transaction
            .commit()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        drop(connection);
        self.repository_source(repository_url)?
            .ok_or_else(|| StorageError::Operation("repository source was not found".to_owned()))
    }
}

fn row_to_repository_source(row: &rusqlite::Row<'_>) -> rusqlite::Result<RepositorySource> {
    Ok(RepositorySource {
        url: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        homepage: row.get(3)?,
        enabled: row_i64_to_bool(4, row.get(4)?)?,
        official: row_i64_to_bool(5, row.get(5)?)?,
        script_count: row_i64_to_usize(6, row.get(6)?)?,
        last_refresh_at_unix: optional_u64(7, row.get(7)?)?,
        last_success_at_unix: optional_u64(8, row.get(8)?)?,
        last_error: row.get(9)?,
        revision: u64::try_from(row.get::<_, i64>(10)?).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                10,
                rusqlite::types::Type::Integer,
                Box::new(error),
            )
        })?,
        etag: row.get(11)?,
        last_modified: row.get(12)?,
        information_mismatch_count: row_i64_to_usize(13, row.get(13)?)?,
    })
}

fn row_to_repository_script(row: &rusqlite::Row<'_>) -> rusqlite::Result<RepositoryScriptRecord> {
    let entry_json = row.get::<_, String>(11)?;
    let entry: serde_json::Value = serde_json::from_str(&entry_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let target_runtimes = entry
        .get("target_runtimes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                11,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "repository entry is missing target_runtimes",
                )),
            )
        })?
        .iter()
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    11,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "repository entry contains an invalid target runtime",
                    )),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RepositoryScriptRecord {
        repository_url: row.get(0)?,
        repository_name: row.get(1)?,
        official: row_i64_to_bool(2, row.get(2)?)?,
        script_id: row.get(3)?,
        name: row.get(4)?,
        summary: row.get(5)?,
        author: row.get(6)?,
        target_runtimes,
        risk_level: row.get(8)?,
        version: row.get(9)?,
        published_at: row.get(10)?,
        entry,
        information_mismatch: row.get(12)?,
        information_mismatch_refresh_required: row_i64_to_bool(13, row.get(13)?)?,
        installed: row_i64_to_bool(14, row.get(14)?)?,
    })
}

fn add_text_filters(
    conditions: &mut Vec<String>,
    values: &mut Vec<Value>,
    column: &str,
    selected: &[String],
) {
    let selected = selected
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return;
    }
    let placeholders = std::iter::repeat_n("?", selected.len())
        .collect::<Vec<_>>()
        .join(", ");
    conditions.push(format!("{column} IN ({placeholders})"));
    values.extend(
        selected
            .into_iter()
            .map(|value| Value::Text(value.to_owned())),
    );
}

fn add_json_array_filters(
    conditions: &mut Vec<String>,
    values: &mut Vec<Value>,
    json_path: &str,
    selected: &[String],
) {
    for value in selected
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM json_each(repository_entries.entry_json, '{json_path}') WHERE json_each.value = ?)"
        ));
        values.push(Value::Text(value.to_owned()));
    }
}

fn repository_order(sort: RepositoryScriptSort) -> &'static str {
    match sort {
        RepositoryScriptSort::Author => "repository_entries.author COLLATE NOCASE",
        RepositoryScriptSort::Published => "repository_entries.published_at",
        RepositoryScriptSort::Repository => "repository_sources.name COLLATE NOCASE",
        RepositoryScriptSort::Risk => "repository_entries.risk_level",
        RepositoryScriptSort::Name => "repository_entries.name COLLATE NOCASE",
        RepositoryScriptSort::Version => "repository_entries.version",
    }
}

fn u64_to_sqlite(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| {
        StorageError::Operation("repository timestamp exceeds storage limits".to_owned())
    })
}

fn optional_u64(index: usize, value: Option<i64>) -> rusqlite::Result<Option<u64>> {
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
