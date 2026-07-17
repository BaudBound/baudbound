use std::collections::BTreeMap;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::{
    GeneratedTriggerToken, NetworkTriggerDefinition, NetworkTriggerType, StorageError,
    TriggerAuthStatus, TriggerAuthentication, storage::filesystem::current_unix_timestamp,
};

use super::{SqliteRunnerStore, conversions::u64_to_sqlite};

impl SqliteRunnerStore {
    pub(super) fn reconcile_network_trigger_auth_with_connection(
        &self,
        connection: &Connection,
        script_id: &str,
        definitions: &[NetworkTriggerDefinition],
    ) -> Result<Vec<GeneratedTriggerToken>, StorageError> {
        let desired = definitions
            .iter()
            .map(|definition| (definition.node_id.as_str(), definition.trigger_type))
            .collect::<BTreeMap<_, _>>();
        let existing =
            self.list_network_trigger_auth_statuses_with_connection(connection, script_id)?;

        for status in existing {
            if desired.get(status.node_id.as_str()) != Some(&status.trigger_type) {
                connection
                    .execute(
                        "DELETE FROM trigger_auth WHERE script_id = ?1 AND trigger_node_id = ?2",
                        params![script_id, status.node_id],
                    )
                    .map_err(|source| StorageError::Sqlite {
                        path: self.path.clone(),
                        source,
                    })?;
            }
        }

        let mut generated_tokens = Vec::new();
        for definition in definitions {
            let exists = connection
                .query_row(
                    r#"
                    SELECT 1 FROM trigger_auth
                    WHERE script_id = ?1 AND trigger_node_id = ?2 AND trigger_type = ?3
                    "#,
                    params![
                        script_id,
                        definition.node_id,
                        trigger_type_name(definition.trigger_type),
                    ],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|source| StorageError::Sqlite {
                    path: self.path.clone(),
                    source,
                })?
                .is_some();
            if !exists {
                generated_tokens
                    .push(self.insert_network_trigger_auth(connection, script_id, definition)?);
            }
        }
        Ok(generated_tokens)
    }

    pub(super) fn list_network_trigger_auth_statuses(
        &self,
        script_reference: &str,
    ) -> Result<Vec<TriggerAuthStatus>, StorageError> {
        let installed = self.resolve_reference(script_reference)?;
        let connection = self.connection()?;
        self.list_network_trigger_auth_statuses_with_connection(&connection, &installed.id)
    }

    pub(super) fn rotate_network_trigger_auth_token(
        &self,
        script_reference: &str,
        node_id: &str,
        trigger_type: NetworkTriggerType,
    ) -> Result<GeneratedTriggerToken, StorageError> {
        let installed = self.resolve_reference(script_reference)?;
        let token = generate_token(trigger_type)?;
        let token_hash = hash_token(&token);
        let token_preview = token_preview(&token);
        let rotated_at_unix = current_unix_timestamp();
        let connection = self.connection()?;
        let changed = connection
            .execute(
                r#"
                UPDATE trigger_auth
                SET token_hash = ?1, token_preview = ?2, rotated_at_unix = ?3
                WHERE script_id = ?4 AND trigger_node_id = ?5 AND trigger_type = ?6
                "#,
                params![
                    token_hash.as_slice(),
                    token_preview,
                    u64_to_sqlite(rotated_at_unix)?,
                    installed.id,
                    node_id,
                    trigger_type_name(trigger_type),
                ],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        if changed == 0 {
            return Err(trigger_auth_not_found(&installed.id, node_id, trigger_type));
        }
        self.request_trigger_reload_with_connection(&connection)?;
        let status = self.trigger_auth_status_with_connection(
            &connection,
            &installed.id,
            node_id,
            trigger_type,
        )?;
        Ok(GeneratedTriggerToken { status, token })
    }

    pub(super) fn set_network_trigger_auth_enabled(
        &self,
        script_reference: &str,
        node_id: &str,
        trigger_type: NetworkTriggerType,
        enabled: bool,
    ) -> Result<TriggerAuthStatus, StorageError> {
        let installed = self.resolve_reference(script_reference)?;
        let disabled_at_unix = (!enabled).then(current_unix_timestamp);
        let connection = self.connection()?;
        let changed = connection
            .execute(
                r#"
                UPDATE trigger_auth
                SET auth_enabled = ?1, disabled_at_unix = ?2
                WHERE script_id = ?3 AND trigger_node_id = ?4 AND trigger_type = ?5
                "#,
                params![
                    i64::from(enabled),
                    disabled_at_unix.map(u64_to_sqlite).transpose()?,
                    installed.id,
                    node_id,
                    trigger_type_name(trigger_type),
                ],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        if changed == 0 {
            return Err(trigger_auth_not_found(&installed.id, node_id, trigger_type));
        }
        self.request_trigger_reload_with_connection(&connection)?;
        self.trigger_auth_status_with_connection(&connection, &installed.id, node_id, trigger_type)
    }

    pub(super) fn authenticate_network_trigger(
        &self,
        script_id: &str,
        node_id: &str,
        trigger_type: NetworkTriggerType,
        provided_token: Option<&str>,
    ) -> Result<TriggerAuthentication, StorageError> {
        let connection = self.connection()?;
        let state = connection
            .query_row(
                r#"
                SELECT auth_enabled, token_hash
                FROM trigger_auth
                WHERE script_id = ?1 AND trigger_node_id = ?2 AND trigger_type = ?3
                "#,
                params![script_id, node_id, trigger_type_name(trigger_type)],
                |row| Ok((row.get::<_, bool>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?
            .ok_or_else(|| trigger_auth_not_found(script_id, node_id, trigger_type))?;
        if !state.0 {
            return Ok(TriggerAuthentication::Disabled);
        }
        let Some(provided_token) = provided_token else {
            return Ok(TriggerAuthentication::MissingToken);
        };
        let provided_hash = hash_token(provided_token);
        Ok(if constant_time_equal(&state.1, &provided_hash) {
            TriggerAuthentication::Authenticated
        } else {
            TriggerAuthentication::InvalidToken
        })
    }

    fn insert_network_trigger_auth(
        &self,
        connection: &Connection,
        script_id: &str,
        definition: &NetworkTriggerDefinition,
    ) -> Result<GeneratedTriggerToken, StorageError> {
        let token = generate_token(definition.trigger_type)?;
        let token_hash = hash_token(&token);
        let preview = token_preview(&token);
        let created_at_unix = current_unix_timestamp();
        connection
            .execute(
                r#"
                INSERT INTO trigger_auth (
                    script_id, trigger_node_id, trigger_type, auth_enabled, token_hash,
                    token_preview, created_at_unix, rotated_at_unix, disabled_at_unix
                )
                VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, NULL, NULL)
                "#,
                params![
                    script_id,
                    definition.node_id,
                    trigger_type_name(definition.trigger_type),
                    token_hash.as_slice(),
                    preview,
                    u64_to_sqlite(created_at_unix)?,
                ],
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        Ok(GeneratedTriggerToken {
            status: TriggerAuthStatus {
                auth_enabled: true,
                created_at_unix,
                disabled_at_unix: None,
                node_id: definition.node_id.clone(),
                rotated_at_unix: None,
                script_id: script_id.to_owned(),
                token_preview: token_preview(&token),
                trigger_type: definition.trigger_type,
            },
            token,
        })
    }

    pub(super) fn list_network_trigger_auth_statuses_with_connection(
        &self,
        connection: &Connection,
        script_id: &str,
    ) -> Result<Vec<TriggerAuthStatus>, StorageError> {
        let mut statement = connection
            .prepare(
                r#"
                SELECT script_id, trigger_node_id, trigger_type, auth_enabled, token_preview,
                    created_at_unix, rotated_at_unix, disabled_at_unix
                FROM trigger_auth
                WHERE script_id = ?1
                ORDER BY trigger_node_id
                "#,
            )
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?;
        let rows = statement
            .query_map(params![script_id], row_to_trigger_auth_status)
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

    fn trigger_auth_status_with_connection(
        &self,
        connection: &Connection,
        script_id: &str,
        node_id: &str,
        trigger_type: NetworkTriggerType,
    ) -> Result<TriggerAuthStatus, StorageError> {
        connection
            .query_row(
                r#"
                SELECT script_id, trigger_node_id, trigger_type, auth_enabled, token_preview,
                    created_at_unix, rotated_at_unix, disabled_at_unix
                FROM trigger_auth
                WHERE script_id = ?1 AND trigger_node_id = ?2 AND trigger_type = ?3
                "#,
                params![script_id, node_id, trigger_type_name(trigger_type)],
                row_to_trigger_auth_status,
            )
            .optional()
            .map_err(|source| StorageError::Sqlite {
                path: self.path.clone(),
                source,
            })?
            .ok_or_else(|| trigger_auth_not_found(script_id, node_id, trigger_type))
    }
}

fn row_to_trigger_auth_status(row: &rusqlite::Row<'_>) -> rusqlite::Result<TriggerAuthStatus> {
    let trigger_type = match row.get::<_, String>(2)?.as_str() {
        "webhook" => NetworkTriggerType::Webhook,
        "websocket" => NetworkTriggerType::Websocket,
        value => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                format!("unknown network trigger type {value:?}").into(),
            ));
        }
    };
    Ok(TriggerAuthStatus {
        script_id: row.get(0)?,
        node_id: row.get(1)?,
        trigger_type,
        auth_enabled: row.get(3)?,
        token_preview: row.get(4)?,
        created_at_unix: super::conversions::row_i64_to_u64(5, row.get(5)?)?,
        rotated_at_unix: row
            .get::<_, Option<i64>>(6)?
            .map(|value| super::conversions::row_i64_to_u64(6, value))
            .transpose()?,
        disabled_at_unix: row
            .get::<_, Option<i64>>(7)?
            .map(|value| super::conversions::row_i64_to_u64(7, value))
            .transpose()?,
    })
}

fn generate_token(trigger_type: NetworkTriggerType) -> Result<String, StorageError> {
    let mut random = [0_u8; 32];
    getrandom::fill(&mut random).map_err(|source| {
        StorageError::Operation(format!(
            "failed to generate network trigger token: {source}"
        ))
    })?;
    let prefix = match trigger_type {
        NetworkTriggerType::Webhook => "bbwh_",
        NetworkTriggerType::Websocket => "bbws_",
    };
    Ok(format!("{prefix}{}", URL_SAFE_NO_PAD.encode(random)))
}

fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn token_preview(token: &str) -> String {
    token
        .chars()
        .rev()
        .take(6)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn trigger_type_name(trigger_type: NetworkTriggerType) -> &'static str {
    match trigger_type {
        NetworkTriggerType::Webhook => "webhook",
        NetworkTriggerType::Websocket => "websocket",
    }
}

fn trigger_auth_not_found(
    script_id: &str,
    node_id: &str,
    trigger_type: NetworkTriggerType,
) -> StorageError {
    StorageError::TriggerAuthNotFound {
        script_id: script_id.to_owned(),
        node_id: node_id.to_owned(),
        trigger_type,
    }
}
