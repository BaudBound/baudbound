use rusqlite::{OptionalExtension, params};

use crate::{StorageError, UpdateCheckCache};

use super::{SqliteRunnerStore, conversions::u64_to_sqlite};

impl SqliteRunnerStore {
    pub fn read_update_check_cache(&self) -> Result<Option<UpdateCheckCache>, StorageError> {
        let connection = self.connection()?;
        connection
            .query_row(
                r#"
                SELECT checked_at_unix, latest_version, published_at, release_notes,
                       update_available
                FROM update_check_cache
                WHERE id = 1
                "#,
                [],
                |row| {
                    let checked_at_unix: i64 = row.get(0)?;
                    Ok(UpdateCheckCache {
                        checked_at_unix: u64::try_from(checked_at_unix).map_err(|_| {
                            rusqlite::Error::IntegralValueOutOfRange(0, checked_at_unix)
                        })?,
                        latest_version: row.get(1)?,
                        published_at: row.get(2)?,
                        release_notes: row.get(3)?,
                        update_available: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(|source| StorageError::Sqlite {
                path: self.path().to_path_buf(),
                source,
            })
    }

    pub fn write_update_check_cache(&self, cache: &UpdateCheckCache) -> Result<(), StorageError> {
        let connection = self.connection()?;
        connection
            .execute(
                r#"
                INSERT INTO update_check_cache (
                    id, checked_at_unix, latest_version, published_at, release_notes,
                    update_available
                ) VALUES (1, ?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(id) DO UPDATE SET
                    checked_at_unix = excluded.checked_at_unix,
                    latest_version = excluded.latest_version,
                    published_at = excluded.published_at,
                    release_notes = excluded.release_notes,
                    update_available = excluded.update_available
                "#,
                params![
                    u64_to_sqlite(cache.checked_at_unix)?,
                    cache.latest_version,
                    cache.published_at,
                    cache.release_notes,
                    cache.update_available,
                ],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path().to_path_buf(),
                source,
            })?;
        Ok(())
    }
}
