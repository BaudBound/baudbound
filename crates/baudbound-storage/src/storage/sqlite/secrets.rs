use rusqlite::{OptionalExtension, params};

use crate::{SecretStatus, StorageError};

use super::{SqliteRunnerStore, conversions::unix_timestamp_for_sqlite};

impl SqliteRunnerStore {
    pub(super) fn list_stored_secret_statuses(
        &self,
        script_reference: &str,
    ) -> Result<Vec<SecretStatus>, StorageError> {
        let script = self.resolve_reference(script_reference)?;
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT name, updated_at_unix FROM secret_values WHERE script_id = ?1 ORDER BY name")
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        let rows = statement
            .query_map(params![script.id], |row| {
                let updated_at_unix = row.get::<_, i64>(1)?;
                Ok(SecretStatus {
                    configured: true,
                    name: row.get(0)?,
                    updated_at_unix: u64::try_from(updated_at_unix).ok(),
                })
            })
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

    pub(super) fn read_stored_secret(
        &self,
        script_id: &str,
        name: &str,
    ) -> Result<Option<serde_json::Value>, StorageError> {
        let cipher = self
            .secret_cipher
            .as_ref()
            .ok_or(StorageError::SecretKeyUnavailable)?;
        let connection = self.connection()?;
        let encrypted = connection
            .query_row(
                "SELECT nonce, ciphertext FROM secret_values WHERE script_id = ?1 AND name = ?2",
                params![script_id, name],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        encrypted
            .map(|(nonce, ciphertext)| {
                let plaintext =
                    cipher.decrypt(&nonce, &ciphertext, &secret_aad(script_id, name))?;
                serde_json::from_slice(&plaintext).map_err(|source| StorageError::Json {
                    path: self.path.clone(),
                    source,
                })
            })
            .transpose()
    }

    pub(super) fn set_stored_secret(
        &self,
        script_reference: &str,
        name: &str,
        value: &serde_json::Value,
    ) -> Result<SecretStatus, StorageError> {
        let cipher = self
            .secret_cipher
            .as_ref()
            .ok_or(StorageError::SecretKeyUnavailable)?;
        let script = self.resolve_reference(script_reference)?;
        let plaintext = zeroize::Zeroizing::new(serde_json::to_vec(value).map_err(|source| {
            StorageError::Json {
                path: self.path.clone(),
                source,
            }
        })?);
        let (nonce, ciphertext) = cipher.encrypt(&plaintext, &secret_aad(&script.id, name))?;
        let updated_at_unix = unix_timestamp_for_sqlite()?;
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO secret_values (script_id, name, nonce, ciphertext, updated_at_unix) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(script_id, name) DO UPDATE SET nonce = excluded.nonce, ciphertext = excluded.ciphertext, updated_at_unix = excluded.updated_at_unix",
                params![script.id, name, nonce.as_slice(), ciphertext, updated_at_unix],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(SecretStatus {
            configured: true,
            name: name.to_owned(),
            updated_at_unix: u64::try_from(updated_at_unix).ok(),
        })
    }

    pub(super) fn remove_stored_secret(
        &self,
        script_reference: &str,
        name: &str,
    ) -> Result<bool, StorageError> {
        let script = self.resolve_reference(script_reference)?;
        let connection = self.connection()?;
        connection
            .execute(
                "DELETE FROM secret_values WHERE script_id = ?1 AND name = ?2",
                params![script.id, name],
            )
            .map(|count| count == 1)
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })
    }
}

fn secret_aad(script_id: &str, name: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(script_id.len() + name.len() + 1);
    aad.extend_from_slice(script_id.as_bytes());
    aad.push(0);
    aad.extend_from_slice(name.as_bytes());
    aad
}
