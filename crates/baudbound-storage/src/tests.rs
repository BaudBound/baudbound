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
            package_hash: imported.package_hash.clone(),
            script_id: imported.id,
        })
        .expect("script should approve");
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("approval should request trigger reload")
    );

    assert_eq!(approval.approved_permissions, ["http_request"]);
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
            level: "info".to_owned(),
            message: format!("{run_id} completed"),
            node_id: None,
            timestamp_unix_ms: completed_at_unix * 1_000,
        }],
        run_id: run_id.to_owned(),
        script_id: script_id.to_owned(),
        status: "completed".to_owned(),
        trigger_node_id: "n-trigger".to_owned(),
        variables: BTreeMap::new(),
    }
}

fn current_test_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should follow Unix epoch")
        .as_secs()
}
