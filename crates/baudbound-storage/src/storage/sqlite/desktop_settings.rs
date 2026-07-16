use rusqlite::{OptionalExtension, params};

use super::SqliteRunnerStore;
use crate::{DesktopSettings, StorageError};

impl SqliteRunnerStore {
    pub fn read_desktop_settings(&self) -> Result<DesktopSettings, StorageError> {
        let connection = self.connection()?;
        connection
            .query_row(
                r#"
                SELECT
                    automatic_update_checks,
                    keep_running_on_close,
                    launch_at_login,
                    start_background_runner_on_launch,
                    start_minimized_to_tray
                FROM desktop_settings
                WHERE id = 1
                "#,
                [],
                |row| {
                    Ok(DesktopSettings {
                        automatic_update_checks: row.get(0)?,
                        keep_running_on_close: row.get(1)?,
                        launch_at_login: row.get(2)?,
                        start_background_runner_on_launch: row.get(3)?,
                        start_minimized_to_tray: row.get(4)?,
                    })
                },
            )
            .optional()
            .map(Option::unwrap_or_default)
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })
    }

    pub fn write_desktop_settings(&self, settings: &DesktopSettings) -> Result<(), StorageError> {
        let connection = self.connection()?;
        connection
            .execute(
                r#"
                INSERT INTO desktop_settings (
                    id,
                    automatic_update_checks,
                    keep_running_on_close,
                    launch_at_login,
                    start_background_runner_on_launch,
                    start_minimized_to_tray
                )
                VALUES (1, ?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(id) DO UPDATE SET
                    automatic_update_checks = excluded.automatic_update_checks,
                    keep_running_on_close = excluded.keep_running_on_close,
                    launch_at_login = excluded.launch_at_login,
                    start_background_runner_on_launch = excluded.start_background_runner_on_launch,
                    start_minimized_to_tray = excluded.start_minimized_to_tray
                "#,
                params![
                    settings.automatic_update_checks,
                    settings.keep_running_on_close,
                    settings.launch_at_login,
                    settings.start_background_runner_on_launch,
                    settings.start_minimized_to_tray,
                ],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }
}
