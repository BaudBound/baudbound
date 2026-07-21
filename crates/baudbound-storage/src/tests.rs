use super::*;
use crate::storage::filesystem::validate_script_id;

fn open_store(temporary_directory: &tempfile::TempDir) -> SqliteRunnerStore {
    SqliteRunnerStore::open(
        temporary_directory
            .path()
            .join("runner")
            .join("runner.sqlite3"),
    )
    .expect("SQLite store should open")
}

fn import_test_script(
    store: &SqliteRunnerStore,
    temporary_directory: &tempfile::TempDir,
) -> InstalledScript {
    let package_path = temporary_directory.path().join("script.bbs");
    std::fs::write(&package_path, b"package bytes").expect("test package should be written");
    store
        .import_script(ImportScriptRequest {
            id: "script-1".to_owned(),
            name: "Script One".to_owned(),
            package_source: package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 0,
            risk_level: "medium".to_owned(),
        })
        .expect("script should import")
}

#[test]
fn initializes_and_reopens_versioned_schema() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let database_path = temporary_directory
        .path()
        .join("runner")
        .join("runner.sqlite3");

    let store = SqliteRunnerStore::open(&database_path).expect("SQLite store should open");
    assert_eq!(store.path(), database_path);
    assert_eq!(
        store
            .schema_version()
            .expect("schema version should be readable"),
        CURRENT_SCHEMA_VERSION
    );

    let reopened = SqliteRunnerStore::open(&database_path).expect("SQLite store should reopen");
    assert_eq!(
        reopened
            .schema_version()
            .expect("schema version should stay current"),
        CURRENT_SCHEMA_VERSION
    );
}

#[test]
fn migrates_version_seven_run_records_to_variable_scopes() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let database_path = temporary_directory
        .path()
        .join("runner")
        .join("runner.sqlite3");
    std::fs::create_dir_all(
        database_path
            .parent()
            .expect("database should have a parent"),
    )
    .expect("database directory should be created");
    let connection = rusqlite::Connection::open(&database_path).expect("database should open");
    connection
        .execute_batch(
            r#"
            CREATE TABLE run_records (
                run_id TEXT PRIMARY KEY,
                script_id TEXT NOT NULL,
                status TEXT NOT NULL,
                trigger_node_id TEXT NOT NULL,
                completed_at_unix INTEGER NOT NULL,
                logs_json TEXT NOT NULL,
                variables_json TEXT NOT NULL
            );
            INSERT INTO run_records (
                run_id, script_id, status, trigger_node_id, completed_at_unix,
                logs_json, variables_json
            ) VALUES (
                'run-before-migration', 'script-1', 'completed', 'n-trigger', 1,
                '[{"level":"error","message":"failed action"}]', '{}'
            );
            PRAGMA user_version = 7;
            "#,
        )
        .expect("version seven schema should be created");
    drop(connection);

    let store = SqliteRunnerStore::open(&database_path).expect("schema should migrate");
    assert_eq!(
        store
            .schema_version()
            .expect("schema version should be readable"),
        CURRENT_SCHEMA_VERSION
    );
    assert_eq!(
        store
            .run_statistics()
            .expect("migrated run statistics should load"),
        RunStatistics {
            cancelled: 0,
            completed: 1,
            failed: 0,
            total: 1,
            with_errors: 1,
        }
    );
    store
        .append_run_record(test_run_record(
            "run-1",
            "script-1",
            current_test_timestamp(),
        ))
        .expect("scoped run record should append after migration");
    let records = store
        .list_run_records(None, None)
        .expect("migrated run records should list");
    assert_eq!(
        records[0]
            .variable_scopes
            .get("counter")
            .map(String::as_str),
        Some("runtime")
    );
}

#[test]
fn round_trips_the_normalized_update_check_cache() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    assert!(
        store
            .read_update_check_cache()
            .expect("empty update cache should read")
            .is_none()
    );

    let cache = UpdateCheckCache {
        checked_at_unix: 123,
        latest_version: "2.1.0".to_owned(),
        published_at: Some("2026-07-17T12:00:00Z".to_owned()),
        release_notes: Some("Changes".to_owned()),
        update_available: true,
    };
    store
        .write_update_check_cache(&cache)
        .expect("update cache should write");

    assert_eq!(
        store
            .read_update_check_cache()
            .expect("update cache should read"),
        Some(cache)
    );
}

#[test]
fn round_trips_service_status() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    assert!(
        store
            .read_service_status()
            .expect("missing status should read cleanly")
            .is_none()
    );

    let status = serde_json::json!({
        "active_service_count": 2,
        "pid": 1234,
        "state": "running"
    });
    store
        .write_service_status(&status)
        .expect("status should write");
    assert_eq!(
        store
            .read_service_status()
            .expect("status should read cleanly"),
        Some(status)
    );
    assert!(store.clear_service_status().expect("status should clear"));
    assert!(
        !store
            .clear_service_status()
            .expect("status is already clear")
    );
}

#[test]
fn trigger_reload_signal_is_one_shot() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    assert!(
        !store
            .consume_trigger_reload_request()
            .expect("missing signal should read cleanly")
    );

    store
        .request_trigger_reload()
        .expect("reload request should write");
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("reload request should be consumed")
    );
    assert!(
        !store
            .consume_trigger_reload_request()
            .expect("reload request should only be consumed once")
    );
}

#[test]
fn supports_complete_script_lifecycle() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    let imported = import_test_script(&store, &temporary_directory);

    assert!(imported.package_path.exists());
    assert_eq!(imported.package_file_name, "script.bbs");
    assert!(store.verify_script_package_hash("script-1").is_ok());
    assert_eq!(
        store
            .find_script("Script One")
            .expect("script should resolve by name")
            .id,
        "script-1"
    );
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("import should request reload")
    );

    let disabled = store
        .set_script_enabled("script-1", false)
        .expect("script should disable");
    assert!(!disabled.enabled);
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("disable should request reload")
    );

    let enabled = store
        .set_script_enabled("script-1", true)
        .expect("script should enable");
    assert!(enabled.enabled);
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("enable should request reload")
    );

    let removed = store
        .remove_script("script-1")
        .expect("script should remove");
    assert_eq!(removed.id, "script-1");
    assert!(!removed.package_path.exists());
    assert!(
        store
            .list_scripts()
            .expect("scripts should list")
            .is_empty()
    );
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("removal should request reload")
    );
}

#[test]
fn update_replaces_package_and_invalidates_approval() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    let imported = import_test_script(&store, &temporary_directory);
    store
        .approve_script(ApproveScriptRequest {
            approved_permissions: vec!["file_write_limited".to_owned()],
            network_triggers: Vec::new(),
            package_hash: imported.package_hash,
            script_id: imported.id,
        })
        .expect("script should approve");
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("import and approval reload request should clear")
    );

    let updated_package_path = temporary_directory.path().join("script-updated.bbs");
    std::fs::write(&updated_package_path, b"updated bytes")
        .expect("updated package should be written");
    let updated = store
        .update_script(ImportScriptRequest {
            id: "script-1".to_owned(),
            name: "Script One Updated".to_owned(),
            package_source: updated_package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 1,
            risk_level: "high".to_owned(),
        })
        .expect("script should update");

    assert_eq!(updated.name, "Script One Updated");
    assert_eq!(updated.package_file_name, "script-updated.bbs");
    assert!(updated.package_path.exists());
    assert!(
        store
            .find_script_approval("script-1")
            .expect("approval lookup should succeed")
            .is_none()
    );
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("update should request reload")
    );
}

#[test]
fn network_trigger_auth_is_hash_only_rotatable_and_reconciled_on_update() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    let package_path = temporary_directory.path().join("network.bbs");
    std::fs::write(&package_path, b"network package").expect("package should be written");
    store
        .import_script(ImportScriptRequest {
            id: "network-script".to_owned(),
            name: "Network Script".to_owned(),
            package_source: package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 0,
            risk_level: "high".to_owned(),
        })
        .expect("network script should import");

    assert!(
        store
            .list_trigger_auth_statuses("network-script")
            .expect("unapproved auth statuses should list")
            .is_empty()
    );

    let approved = store
        .approve_script(ApproveScriptRequest {
            approved_permissions: Vec::new(),
            network_triggers: vec![
                NetworkTriggerDefinition {
                    node_id: "n-webhook".to_owned(),
                    trigger_type: NetworkTriggerType::Webhook,
                },
                NetworkTriggerDefinition {
                    node_id: "n-websocket".to_owned(),
                    trigger_type: NetworkTriggerType::Websocket,
                },
            ],
            package_hash: "initial-package-hash".to_owned(),
            script_id: "network-script".to_owned(),
        })
        .expect("network script should approve");

    assert_eq!(approved.generated_trigger_tokens.len(), 2);
    for generated in &approved.generated_trigger_tokens {
        assert_eq!(
            store
                .authenticate_trigger(
                    "network-script",
                    &generated.status.node_id,
                    generated.status.trigger_type,
                    Some(&generated.token),
                )
                .expect("generated import token should authenticate"),
            TriggerAuthentication::Authenticated
        );
    }

    let initial = store
        .list_trigger_auth_statuses("network-script")
        .expect("auth statuses should list");
    assert_eq!(initial.len(), 2);
    assert!(initial.iter().all(|status| status.auth_enabled));
    assert!(initial.iter().all(|status| status.token_preview.len() == 6));

    let rotated = store
        .rotate_trigger_auth_token("network-script", "n-webhook", NetworkTriggerType::Webhook)
        .expect("webhook token should rotate");
    assert!(rotated.token.starts_with("bbwh_"));
    assert_eq!(rotated.token.len(), 48);
    assert_eq!(
        store
            .authenticate_trigger(
                "network-script",
                "n-webhook",
                NetworkTriggerType::Webhook,
                Some(&rotated.token),
            )
            .expect("correct token should validate"),
        TriggerAuthentication::Authenticated
    );
    assert_eq!(
        store
            .authenticate_trigger(
                "network-script",
                "n-webhook",
                NetworkTriggerType::Webhook,
                Some("bbwh_wrong"),
            )
            .expect("wrong token should be handled"),
        TriggerAuthentication::InvalidToken
    );
    assert_eq!(
        store
            .authenticate_trigger(
                "network-script",
                "n-webhook",
                NetworkTriggerType::Webhook,
                None,
            )
            .expect("missing token should be handled"),
        TriggerAuthentication::MissingToken
    );

    let disabled = store
        .set_trigger_auth_enabled(
            "network-script",
            "n-webhook",
            NetworkTriggerType::Webhook,
            false,
        )
        .expect("webhook auth should disable");
    assert!(!disabled.auth_enabled);
    assert!(disabled.disabled_at_unix.is_some());
    assert_eq!(
        store
            .authenticate_trigger(
                "network-script",
                "n-webhook",
                NetworkTriggerType::Webhook,
                None,
            )
            .expect("disabled auth should be reported"),
        TriggerAuthentication::Disabled
    );

    let updated_package_path = temporary_directory.path().join("network-updated.bbs");
    std::fs::write(&updated_package_path, b"updated network package")
        .expect("updated package should be written");
    store
        .update_script(ImportScriptRequest {
            id: "network-script".to_owned(),
            name: "Network Script".to_owned(),
            package_source: updated_package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 0,
            risk_level: "high".to_owned(),
        })
        .expect("network script should update");

    let before_reapproval = store
        .list_trigger_auth_statuses("network-script")
        .expect("stale auth statuses should list before reapproval");
    assert_eq!(before_reapproval.len(), 2);
    assert!(
        before_reapproval
            .iter()
            .any(|status| status.node_id == "n-websocket")
    );

    let reapproved = store
        .approve_script(ApproveScriptRequest {
            approved_permissions: Vec::new(),
            network_triggers: vec![
                NetworkTriggerDefinition {
                    node_id: "n-webhook".to_owned(),
                    trigger_type: NetworkTriggerType::Webhook,
                },
                NetworkTriggerDefinition {
                    node_id: "n-webhook-new".to_owned(),
                    trigger_type: NetworkTriggerType::Webhook,
                },
            ],
            package_hash: "updated-package-hash".to_owned(),
            script_id: "network-script".to_owned(),
        })
        .expect("updated network script should approve");

    assert_eq!(reapproved.generated_trigger_tokens.len(), 1);
    assert_eq!(
        reapproved.generated_trigger_tokens[0].status.node_id,
        "n-webhook-new"
    );

    let updated = store
        .list_trigger_auth_statuses("network-script")
        .expect("updated auth statuses should list");
    assert_eq!(updated.len(), 2);
    let preserved = updated
        .iter()
        .find(|status| status.node_id == "n-webhook")
        .expect("unchanged webhook should remain");
    assert_eq!(preserved.token_preview, rotated.status.token_preview);
    assert!(!preserved.auth_enabled);
    assert!(updated.iter().all(|status| status.node_id != "n-websocket"));
    let added = updated
        .iter()
        .find(|status| status.node_id == "n-webhook-new")
        .expect("new webhook should receive auth state");
    assert!(added.auth_enabled);
    assert!(added.rotated_at_unix.is_none());
}

#[test]
fn stores_finds_and_revokes_approval() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    let imported = import_test_script(&store, &temporary_directory);
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("import reload request should clear")
    );
    let approval = store
        .approve_script(ApproveScriptRequest {
            approved_permissions: vec!["http_request".to_owned()],
            network_triggers: Vec::new(),
            package_hash: imported.package_hash.clone(),
            script_id: imported.id,
        })
        .expect("script should approve");
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("approval should request trigger reload")
    );

    assert_eq!(approval.approval.approved_permissions, ["http_request"]);
    assert_eq!(
        store
            .find_script_approval("Script One")
            .expect("approval lookup should succeed")
            .expect("approval should exist")
            .package_hash,
        imported.package_hash
    );
    assert!(
        store
            .revoke_script_approval("script-1")
            .expect("approval should revoke")
            .is_some()
    );
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("revocation should request trigger reload")
    );
}

#[test]
fn detects_installed_package_hash_mismatch() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    let imported = import_test_script(&store, &temporary_directory);
    std::fs::write(&imported.package_path, b"tampered bytes")
        .expect("stored package should mutate");

    assert!(matches!(
        store.verify_script_package_hash("script-1"),
        Err(StorageError::HashMismatch { .. })
    ));
}

#[test]
fn compare_and_set_variables_prevent_lost_updates() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    import_test_script(&store, &temporary_directory);

    assert!(
        store
            .compare_and_set_variable(
                StoredVariableScope::Persistent,
                "script-1",
                "counter",
                None,
                &serde_json::json!(1),
            )
            .expect("initial value should store")
    );
    let initial = store
        .load_variable(StoredVariableScope::Persistent, "script-1", "counter")
        .expect("value should load")
        .expect("value should exist");
    assert_eq!(initial.version, 1);
    assert!(
        store
            .compare_and_set_variable(
                StoredVariableScope::Persistent,
                "script-1",
                "counter",
                Some(initial.version),
                &serde_json::json!(2),
            )
            .expect("matching update should execute")
    );
    assert!(
        !store
            .compare_and_set_variable(
                StoredVariableScope::Persistent,
                "script-1",
                "counter",
                Some(initial.version),
                &serde_json::json!(99),
            )
            .expect("stale update should execute safely")
    );
    let current = store
        .load_variable(StoredVariableScope::Persistent, "script-1", "counter")
        .expect("value should load")
        .expect("value should exist");
    assert_eq!(current.value, serde_json::json!(2));
    assert_eq!(current.version, 2);
}

#[test]
fn encrypts_secret_values_and_rejects_the_wrong_key() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let database_path = temporary_directory
        .path()
        .join("runner")
        .join("runner.sqlite3");
    let key = SecretCipher::generate_key().expect("test key should generate");
    let store = SqliteRunnerStore::open(&database_path)
        .expect("store should open")
        .with_secret_cipher(SecretCipher::from_key(key));
    import_test_script(&store, &temporary_directory);
    store
        .set_secret(
            "script-1",
            "api_key",
            &serde_json::json!("super-secret-value"),
        )
        .expect("secret should store");
    assert_eq!(
        store
            .read_secret("script-1", "api_key")
            .expect("secret should decrypt"),
        Some(serde_json::json!("super-secret-value"))
    );

    let connection = rusqlite::Connection::open(&database_path).expect("database should inspect");
    let ciphertext = connection
        .query_row(
            "SELECT ciphertext FROM secret_values WHERE script_id = 'script-1' AND name = 'api_key'",
            [],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .expect("ciphertext should exist");
    assert!(
        !String::from_utf8_lossy(&ciphertext).contains("super-secret-value"),
        "plaintext must not appear in stored ciphertext"
    );

    let wrong_key = SecretCipher::generate_key().expect("second test key should generate");
    let wrong_store = SqliteRunnerStore::open(&database_path)
        .expect("store should reopen")
        .with_secret_cipher(SecretCipher::from_key(wrong_key));
    assert!(matches!(
        wrong_store.read_secret("script-1", "api_key"),
        Err(StorageError::SecretCrypto(_))
    ));
}

#[test]
fn enables_secret_access_after_the_cipher_becomes_available() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    import_test_script(&store, &temporary_directory);

    assert!(!store.has_secret_cipher());
    assert!(matches!(
        store.set_secret("script-1", "api_key", &serde_json::json!("value")),
        Err(StorageError::SecretKeyUnavailable)
    ));

    let key = SecretCipher::generate_key().expect("test key should generate");
    store.set_secret_cipher(SecretCipher::from_key(key));

    assert!(store.has_secret_cipher());
    store
        .set_secret("script-1", "api_key", &serde_json::json!("value"))
        .expect("secret should store after cipher installation");
    assert_eq!(
        store
            .read_secret("script-1", "api_key")
            .expect("secret should decrypt"),
        Some(serde_json::json!("value"))
    );
}

#[test]
fn rejects_unsafe_script_ids() {
    assert!(matches!(
        validate_script_id("../nope"),
        Err(StorageError::InvalidScriptId(_))
    ));
    assert!(matches!(
        validate_script_id("bad/slash"),
        Err(StorageError::InvalidScriptId(_))
    ));
}

#[test]
fn appends_orders_and_filters_run_records() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    import_test_script(&store, &temporary_directory);

    let now = current_test_timestamp();
    store
        .append_run_record(test_run_record("run-1", "script-1", now - 20))
        .expect("first run should append");
    store
        .append_run_record(test_run_record("run-2", "script-2", now - 10))
        .expect("second run should append");
    store
        .append_run_record(test_run_record("run-3", "script-1", now))
        .expect("third run should append");

    let all_records = store
        .list_run_records(None, Some(2))
        .expect("all records should list");
    assert_eq!(
        all_records
            .iter()
            .map(|record| record.run_id.as_str())
            .collect::<Vec<_>>(),
        ["run-3", "run-2"]
    );
    assert_eq!(
        all_records[0]
            .variable_scopes
            .get("counter")
            .map(String::as_str),
        Some("runtime")
    );
    let script_records = store
        .list_run_records(Some("Script One"), None)
        .expect("script records should list");
    assert_eq!(
        script_records
            .iter()
            .map(|record| record.run_id.as_str())
            .collect::<Vec<_>>(),
        ["run-3", "run-1"]
    );
}

#[test]
fn summarizes_all_retained_run_records() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    let now = current_test_timestamp();

    let mut completed_with_error = test_run_record("run-completed", "script-1", now - 2);
    completed_with_error.logs.push(RunLogEntry {
        action_type: Some("action.test".to_owned()),
        level: "error".to_owned(),
        message: "action reported an error".to_owned(),
        node_id: Some("n-action".to_owned()),
        timestamp_unix_ms: (now - 2) * 1_000,
    });
    let mut failed = test_run_record("run-failed", "script-1", now - 1);
    failed.status = "failed".to_owned();
    let mut cancelled = test_run_record("run-cancelled", "script-1", now);
    cancelled.status = "cancelled".to_owned();

    for record in [completed_with_error, failed, cancelled] {
        store.append_run_record(record).expect("run should append");
    }

    assert_eq!(
        store.run_statistics().expect("statistics should load"),
        RunStatistics {
            cancelled: 1,
            completed: 1,
            failed: 1,
            total: 3,
            with_errors: 2,
        }
    );

    store.clear_run_logs().expect("logs should clear");
    assert_eq!(
        store
            .run_statistics()
            .expect("statistics should update after clearing logs")
            .with_errors,
        1
    );

    store.clear_run_records().expect("history should clear");
    assert_eq!(
        store
            .run_statistics()
            .expect("empty statistics should load"),
        RunStatistics::default()
    );
}

#[test]
fn clears_run_logs_without_removing_history_and_clears_run_records() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    let now = current_test_timestamp();
    store
        .append_run_record(test_run_record("run-1", "script-1", now - 1))
        .expect("first run should append");
    store
        .append_run_record(test_run_record("run-2", "script-1", now))
        .expect("second run should append");

    assert_eq!(store.clear_run_logs().expect("run logs should clear"), 2);
    let records = store
        .list_run_records(None, None)
        .expect("run records should remain readable");
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|record| record.logs.is_empty()));
    assert_eq!(
        store
            .clear_run_logs()
            .expect("empty logs should stay clear"),
        0
    );

    assert_eq!(
        store.clear_run_records().expect("run records should clear"),
        2
    );
    assert!(
        store
            .list_run_records(None, None)
            .expect("empty run history should list")
            .is_empty()
    );
    assert_eq!(
        store
            .clear_run_records()
            .expect("empty run history should stay clear"),
        0
    );
}

#[test]
fn reads_run_logs_written_before_per_action_timestamps() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    import_test_script(&store, &temporary_directory);
    let completed_at_unix = current_test_timestamp();
    let stored_completed_at_unix =
        i64::try_from(completed_at_unix).expect("test timestamp should fit SQLite");
    let connection =
        rusqlite::Connection::open(store.path()).expect("test database should open independently");
    connection
        .execute(
            r#"
            INSERT INTO run_records (
                run_id, script_id, status, trigger_node_id, completed_at_unix,
                logs_json, variables_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            rusqlite::params![
                "pre-timestamp-run",
                "script-1",
                "completed",
                "n-trigger",
                stored_completed_at_unix,
                r#"[{"level":"info","message":"older log","node_id":"n-action"}]"#,
                "{}",
            ],
        )
        .expect("pre-timestamp run should insert");

    let records = store
        .list_run_records(None, None)
        .expect("pre-timestamp run should remain readable");

    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].logs[0].timestamp_unix_ms,
        completed_at_unix * 1_000
    );
}

#[test]
fn run_retention_prunes_count_and_age_without_touching_live_variables() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);
    import_test_script(&store, &temporary_directory);
    store
        .compare_and_set_variable(
            StoredVariableScope::Persistent,
            "script-1",
            "counter",
            None,
            &serde_json::json!(7),
        )
        .expect("persistent variable should write");
    store
        .set_run_retention_policy(RunRetentionPolicy::new(2, 30))
        .expect("retention policy should apply");

    let now = current_test_timestamp();
    for (run_id, age) in [("run-1", 3), ("run-2", 2), ("run-3", 1)] {
        store
            .append_run_record(test_run_record(run_id, "script-1", now - age))
            .expect("run should append");
    }
    assert_eq!(
        store
            .list_run_records(None, None)
            .expect("runs should list")
            .into_iter()
            .map(|record| record.run_id)
            .collect::<Vec<_>>(),
        ["run-3", "run-2"]
    );

    let deleted = store
        .set_run_retention_policy(RunRetentionPolicy::new(1, 30))
        .expect("reduced retention policy should prune immediately");
    assert_eq!(deleted, 1);
    assert_eq!(
        store
            .list_run_records(None, None)
            .expect("runs should list")
            .into_iter()
            .map(|record| record.run_id)
            .collect::<Vec<_>>(),
        ["run-3"]
    );
    assert_eq!(
        store
            .load_variable(StoredVariableScope::Persistent, "script-1", "counter")
            .expect("persistent variable should load")
            .expect("persistent variable should remain")
            .value,
        serde_json::json!(7)
    );

    store
        .set_run_retention_policy(RunRetentionPolicy::new(100, 1))
        .expect("age policy should apply");
    store
        .append_run_record(test_run_record(
            "expired-run",
            "script-1",
            now.saturating_sub(2 * 24 * 60 * 60),
        ))
        .expect("expired run insert and prune should be atomic");
    assert!(
        store
            .list_run_records(None, None)
            .expect("runs should list")
            .iter()
            .all(|record| record.run_id != "expired-run")
    );
}

#[test]
fn run_retention_rejects_unbounded_zero_limits() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = open_store(&temporary_directory);

    for policy in [
        RunRetentionPolicy::new(0, 30),
        RunRetentionPolicy::new(100, 0),
    ] {
        assert!(store.set_run_retention_policy(policy).is_err());
    }
}

fn test_run_record(run_id: &str, script_id: &str, completed_at_unix: u64) -> StoredRunRecord {
    StoredRunRecord {
        completed_at_unix,
        logs: vec![RunLogEntry {
            action_type: None,
            level: "info".to_owned(),
            message: format!("{run_id} completed"),
            node_id: None,
            timestamp_unix_ms: completed_at_unix * 1_000,
        }],
        run_id: run_id.to_owned(),
        script_id: script_id.to_owned(),
        status: "completed".to_owned(),
        trigger_node_id: "n-trigger".to_owned(),
        variable_scopes: BTreeMap::from([("counter".to_owned(), "runtime".to_owned())]),
        variables: BTreeMap::from([("counter".to_owned(), serde_json::json!(1))]),
    }
}

fn current_test_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should follow Unix epoch")
        .as_secs()
}
