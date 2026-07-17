use rusqlite::{OptionalExtension, params};

use super::SqliteRunnerStore;
use crate::{ApplicationSettings, DesktopSettings, SharedSettings, StorageError, TimeFormat};

impl SqliteRunnerStore {
    pub fn read_application_settings(&self) -> Result<ApplicationSettings, StorageError> {
        let connection = self.connection()?;
        connection
            .query_row(
                r#"
                SELECT
                    time_format,
                    automatic_update_checks,
                    keep_running_on_close,
                    launch_at_login,
                    start_background_runner_on_launch,
                    start_minimized_to_tray
                FROM application_settings
                WHERE id = 1
                "#,
                [],
                |row| {
                    let stored_time_format = row.get::<_, String>(0)?;
                    let time_format =
                        TimeFormat::from_storage(&stored_time_format).ok_or_else(|| {
                            rusqlite::Error::FromSqlConversionFailure(
                                0,
                                rusqlite::types::Type::Text,
                                Box::new(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    format!("invalid time format {stored_time_format:?}"),
                                )),
                            )
                        })?;
                    Ok(ApplicationSettings {
                        shared: SharedSettings { time_format },
                        desktop: DesktopSettings {
                            automatic_update_checks: row.get(1)?,
                            keep_running_on_close: row.get(2)?,
                            launch_at_login: row.get(3)?,
                            start_background_runner_on_launch: row.get(4)?,
                            start_minimized_to_tray: row.get(5)?,
                        },
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

    pub fn write_application_settings(
        &self,
        settings: &ApplicationSettings,
    ) -> Result<(), StorageError> {
        let connection = self.connection()?;
        connection
            .execute(
                r#"
                INSERT INTO application_settings (
                    id,
                    time_format,
                    automatic_update_checks,
                    keep_running_on_close,
                    launch_at_login,
                    start_background_runner_on_launch,
                    start_minimized_to_tray
                )
                VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    time_format = excluded.time_format,
                    automatic_update_checks = excluded.automatic_update_checks,
                    keep_running_on_close = excluded.keep_running_on_close,
                    launch_at_login = excluded.launch_at_login,
                    start_background_runner_on_launch = excluded.start_background_runner_on_launch,
                    start_minimized_to_tray = excluded.start_minimized_to_tray
                "#,
                params![
                    settings.shared.time_format.as_str(),
                    settings.desktop.automatic_update_checks,
                    settings.desktop.keep_running_on_close,
                    settings.desktop.launch_at_login,
                    settings.desktop.start_background_runner_on_launch,
                    settings.desktop.start_minimized_to_tray,
                ],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }

    pub fn write_shared_settings(&self, settings: &SharedSettings) -> Result<(), StorageError> {
        let connection = self.connection()?;
        connection
            .execute(
                r#"
                INSERT INTO application_settings (id, time_format)
                VALUES (1, ?1)
                ON CONFLICT(id) DO UPDATE SET time_format = excluded.time_format
                "#,
                params![settings.time_format.as_str()],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }
}
