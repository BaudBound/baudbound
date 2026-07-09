use super::*;

#[test]
fn imports_lists_finds_and_removes_script() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("script.bbs");
    fs::write(&package_path, b"package bytes").expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let imported = store
        .import_script(ImportScriptRequest {
            id: "script-1".to_owned(),
            name: "Script One".to_owned(),
            package_source: package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 0,
            risk_level: "low".to_owned(),
        })
        .expect("script should import");

    assert!(imported.package_path.exists());
    assert_eq!(imported.package_file_name, "script.bbs");
    assert_eq!(store.list_scripts().expect("scripts should list").len(), 1);
    assert_eq!(
        store
            .find_script("Script One")
            .expect("script should be found by name")
            .id,
        "script-1"
    );

    let removed = store
        .remove_script("script-1")
        .expect("script should be removed");
    assert_eq!(removed.id, "script-1");
    assert!(
        store
            .list_scripts()
            .expect("scripts should list")
            .is_empty()
    );
    assert!(!store.root().join("scripts").join("script.bbs").exists());
}

#[test]
fn updates_existing_script_package() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("script.bbs");
    let updated_package_path = temporary_directory.path().join("script-updated.bbs");
    fs::write(&package_path, b"package bytes").expect("test package should be written");
    fs::write(&updated_package_path, b"updated bytes").expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    store
        .import_script(ImportScriptRequest {
            id: "script-1".to_owned(),
            name: "Script One".to_owned(),
            package_source: package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 0,
            risk_level: "low".to_owned(),
        })
        .expect("script should import");

    let updated = store
        .update_script(ImportScriptRequest {
            id: "script-1".to_owned(),
            name: "Script One Updated".to_owned(),
            package_source: updated_package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 0,
            risk_level: "medium".to_owned(),
        })
        .expect("script should update");

    assert_eq!(updated.name, "Script One Updated");
    assert_eq!(updated.risk_level, "medium");
    assert_eq!(updated.package_file_name, "script-updated.bbs");
    assert!(updated.package_path.exists());
    assert!(!store.root().join("scripts").join("script.bbs").exists());
    assert!(store.verify_script_package_hash("script-1").is_ok());
}

#[test]
fn toggles_installed_script_enabled_state() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("script.bbs");
    fs::write(&package_path, b"package bytes").expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    store
        .import_script(ImportScriptRequest {
            id: "script-1".to_owned(),
            name: "Script One".to_owned(),
            package_source: package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 0,
            risk_level: "low".to_owned(),
        })
        .expect("script should import");

    let disabled = store
        .set_script_enabled("Script One", false)
        .expect("script should disable");
    assert!(!disabled.enabled);
    assert!(
        !store
            .find_script("script-1")
            .expect("script should exist")
            .enabled
    );

    let enabled = store
        .set_script_enabled("script-1", true)
        .expect("script should enable");
    assert!(enabled.enabled);
    assert!(
        store
            .find_script("Script One")
            .expect("script should exist")
            .enabled
    );
}

#[test]
fn installed_script_changes_request_trigger_reload() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("script.bbs");
    fs::write(&package_path, b"package bytes").expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    assert!(
        !store
            .consume_trigger_reload_request()
            .expect("missing reload signal should be consumed cleanly")
    );

    store
        .import_script(ImportScriptRequest {
            id: "script-1".to_owned(),
            name: "Script One".to_owned(),
            package_source: package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 0,
            risk_level: "low".to_owned(),
        })
        .expect("script should import");

    assert!(
        store
            .consume_trigger_reload_request()
            .expect("import should request reload")
    );
    assert!(
        !store
            .consume_trigger_reload_request()
            .expect("reload signal should only be consumed once")
    );

    store
        .set_script_enabled("script-1", false)
        .expect("script should disable");
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("enable state change should request reload")
    );

    store
        .remove_script("script-1")
        .expect("script should remove");
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("remove should request reload")
    );
}

#[test]
fn reads_and_writes_service_status() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));

    assert!(
        store
            .read_service_status()
            .expect("missing service status should read cleanly")
            .is_none()
    );

    let status = serde_json::json!({
        "active_service_count": 1,
        "last_heartbeat_unix": 123,
        "pid": 42,
        "services": [
            {
                "active": true,
                "enabled": true,
                "name": "schedule",
                "registrations": 1,
                "target": "internal timer"
            }
        ],
        "state": "running"
    });
    store
        .write_service_status(&status)
        .expect("service status should write");

    assert_eq!(
        store
            .read_service_status()
            .expect("service status should read"),
        Some(status)
    );

    assert!(
        store
            .clear_service_status()
            .expect("service status should clear")
    );
    assert!(
        store
            .read_service_status()
            .expect("cleared service status should read cleanly")
            .is_none()
    );
    assert!(
        !store
            .clear_service_status()
            .expect("missing service status should clear cleanly")
    );
}

#[test]
fn sqlite_runner_store_initializes_versioned_schema() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let database_path = temporary_directory.path().join("runner").join("runner.db");

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
fn sqlite_runner_store_round_trips_service_status() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = SqliteRunnerStore::open(temporary_directory.path().join("runner.db"))
        .expect("SQLite store should open");

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
        store
            .read_service_status()
            .expect("cleared status should read cleanly")
            .is_none()
    );
}

#[test]
fn sqlite_runner_store_trigger_reload_signal_is_one_shot() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = SqliteRunnerStore::open(temporary_directory.path().join("runner.db"))
        .expect("SQLite store should open");

    assert!(
        !store
            .consume_trigger_reload_request()
            .expect("missing reload request should read cleanly")
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
fn sqlite_runner_store_supports_script_lifecycle() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("script.bbs");
    fs::write(&package_path, b"package bytes").expect("test package should be written");

    let store = SqliteRunnerStore::open(temporary_directory.path().join("store").join("runner.db"))
        .expect("SQLite store should open");

    let imported = store
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
        .expect("script should import");
    assert!(imported.package_path.exists());
    assert!(
        store
            .consume_trigger_reload_request()
            .expect("import should request reload")
    );
    assert_eq!(
        store
            .find_script("Script One")
            .expect("script should resolve by name")
            .id,
        "script-1"
    );
    assert!(store.verify_script_package_hash("script-1").is_ok());

    let approval = store
        .approve_script(ApproveScriptRequest {
            approved_permissions: vec!["http_request".to_owned()],
            package_hash: imported.package_hash,
            script_id: imported.id,
        })
        .expect("script should approve");
    assert_eq!(approval.approved_permissions, ["http_request"]);
    assert_eq!(
        store
            .find_script_approval("Script One")
            .expect("approval lookup should succeed")
            .expect("approval should exist")
            .script_id,
        "script-1"
    );

    store
        .append_run_record(test_run_record("run-1", "script-1", 123))
        .expect("run record should append");
    assert_eq!(
        store
            .list_run_records(Some("script-1"), Some(1))
            .expect("run records should list")[0]
            .run_id,
        "run-1"
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
}

#[test]
fn sqlite_runner_store_migrates_filesystem_store() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("script.bbs");
    fs::write(&package_path, b"package bytes").expect("test package should be written");

    let filesystem_store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let imported = filesystem_store
        .import_script(ImportScriptRequest {
            id: "script-1".to_owned(),
            name: "Script One".to_owned(),
            package_source: package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 2,
            risk_level: "high".to_owned(),
        })
        .expect("script should import");
    filesystem_store
        .approve_script(ApproveScriptRequest {
            approved_permissions: vec!["file_write_limited".to_owned()],
            package_hash: imported.package_hash,
            script_id: imported.id,
        })
        .expect("script should approve");
    filesystem_store
        .append_run_record(test_run_record("run-1", "script-1", 123))
        .expect("run record should append");
    filesystem_store
        .write_service_status(&serde_json::json!({
            "pid": 1234,
            "state": "running"
        }))
        .expect("service status should write");

    let sqlite_store =
        SqliteRunnerStore::open(temporary_directory.path().join("store").join("runner.db"))
            .expect("SQLite store should open");
    sqlite_store
        .migrate_from_filesystem(&filesystem_store)
        .expect("filesystem store should migrate");
    sqlite_store
        .migrate_from_filesystem(&filesystem_store)
        .expect("filesystem migration should be idempotent");

    let migrated = sqlite_store
        .find_script("Script One")
        .expect("migrated script should resolve");
    assert_eq!(migrated.id, "script-1");
    assert_eq!(migrated.asset_count, 2);
    assert!(sqlite_store.verify_script_package_hash("script-1").is_ok());
    assert_eq!(
        sqlite_store
            .find_script_approval("script-1")
            .expect("approval lookup should succeed")
            .expect("approval should exist")
            .approved_permissions,
        ["file_write_limited"]
    );
    assert_eq!(
        sqlite_store
            .list_run_records(None, None)
            .expect("run records should list")
            .len(),
        1
    );
    assert_eq!(
        sqlite_store
            .read_service_status()
            .expect("service status should read")
            .expect("service status should exist")["state"],
        "running"
    );
}

#[test]
fn sqlite_runner_store_rejects_migration_when_package_hash_is_tampered() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("script.bbs");
    fs::write(&package_path, b"package bytes").expect("test package should be written");

    let filesystem_store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let imported = filesystem_store
        .import_script(ImportScriptRequest {
            id: "script-1".to_owned(),
            name: "Script One".to_owned(),
            package_source: package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 0,
            risk_level: "low".to_owned(),
        })
        .expect("script should import");
    fs::write(&imported.package_path, b"tampered bytes").expect("package should be tampered");

    let sqlite_store =
        SqliteRunnerStore::open(temporary_directory.path().join("store").join("runner.db"))
            .expect("SQLite store should open");

    assert!(matches!(
        sqlite_store.migrate_from_filesystem(&filesystem_store),
        Err(StorageError::HashMismatch { .. })
    ));
    assert!(
        sqlite_store
            .list_scripts()
            .expect("SQLite scripts should list")
            .is_empty()
    );
}

#[test]
fn service_control_request_is_one_shot() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));

    assert!(
        store
            .consume_service_control_request()
            .expect("missing service control should read cleanly")
            .is_none()
    );

    let request = serde_json::json!({
        "command": "reload",
        "requested_at_unix": 123,
        "target_pid": 42
    });
    store
        .write_service_control_request(&request)
        .expect("service control should write");

    assert_eq!(
        store
            .consume_service_control_request()
            .expect("service control should read"),
        Some(request)
    );
    assert!(
        store
            .consume_service_control_request()
            .expect("service control should only be consumed once")
            .is_none()
    );
}

#[test]
fn typed_service_control_targets_running_service_pid() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));

    store
        .write_service_status(&serde_json::json!({
            "pid": 1234,
            "state": "running"
        }))
        .expect("service status should write");
    assert_eq!(
        store
            .running_service_pid()
            .expect("running service pid should parse"),
        1234
    );

    store
        .request_service_control(ServiceControlCommand::Stop, 5678)
        .expect("service stop should be requested");
    assert_eq!(
        store
            .consume_service_control_request_for_pid(9999)
            .expect("wrong pid request should be consumed"),
        Some(ConsumedServiceControl::Ignored {
            reason: "request targets pid 1234, but this process is 9999".to_owned()
        })
    );

    store
        .request_service_control(ServiceControlCommand::Reload, 5678)
        .expect("service reload should be requested");
    assert_eq!(
        store
            .consume_service_control_request_for_pid(1234)
            .expect("targeted request should be consumed"),
        Some(ConsumedServiceControl::Command(
            ServiceControlCommand::Reload
        ))
    );
}

#[test]
fn stores_finds_and_revokes_script_approvals() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("script.bbs");
    fs::write(&package_path, b"package bytes").expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let imported = store
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
        .expect("script should import");

    let approval = store
        .approve_script(ApproveScriptRequest {
            approved_permissions: vec!["http_request".to_owned()],
            package_hash: imported.package_hash.clone(),
            script_id: imported.id.clone(),
        })
        .expect("script should approve");

    assert_eq!(approval.script_id, "script-1");
    assert_eq!(approval.package_hash, imported.package_hash);
    assert_eq!(approval.approved_permissions, ["http_request"]);
    assert_eq!(
        store
            .find_script_approval("Script One")
            .expect("approval lookup should succeed")
            .expect("approval should exist")
            .script_id,
        "script-1"
    );

    let revoked = store
        .revoke_script_approval("script-1")
        .expect("approval should revoke");
    assert!(revoked.is_some());
    assert!(
        store
            .find_script_approval("script-1")
            .expect("approval lookup should succeed")
            .is_none()
    );
}

#[test]
fn update_and_remove_clear_script_approval() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("script.bbs");
    let updated_package_path = temporary_directory.path().join("script-updated.bbs");
    fs::write(&package_path, b"package bytes").expect("test package should be written");
    fs::write(&updated_package_path, b"updated bytes").expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let imported = store
        .import_script(ImportScriptRequest {
            id: "script-1".to_owned(),
            name: "Script One".to_owned(),
            package_source: package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 0,
            risk_level: "high".to_owned(),
        })
        .expect("script should import");

    store
        .approve_script(ApproveScriptRequest {
            approved_permissions: vec!["file_write_limited".to_owned()],
            package_hash: imported.package_hash,
            script_id: "script-1".to_owned(),
        })
        .expect("script should approve");

    store
        .update_script(ImportScriptRequest {
            id: "script-1".to_owned(),
            name: "Script One Updated".to_owned(),
            package_source: updated_package_path.clone(),
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 0,
            risk_level: "high".to_owned(),
        })
        .expect("script should update");
    assert!(
        store
            .find_script_approval("script-1")
            .expect("approval lookup should succeed")
            .is_none()
    );

    let updated = store.find_script("script-1").expect("script should exist");
    store
        .approve_script(ApproveScriptRequest {
            approved_permissions: vec!["file_write_limited".to_owned()],
            package_hash: updated.package_hash,
            script_id: "script-1".to_owned(),
        })
        .expect("script should approve again");

    store
        .remove_script("script-1")
        .expect("script should remove");
    assert!(matches!(
        store.find_script_approval("script-1"),
        Err(StorageError::NotFound(_))
    ));
}

#[test]
fn detects_hash_mismatch() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("script.bbs");
    fs::write(&package_path, b"package bytes").expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let imported = store
        .import_script(ImportScriptRequest {
            id: "script-1".to_owned(),
            name: "Script One".to_owned(),
            package_source: package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 0,
            risk_level: "low".to_owned(),
        })
        .expect("script should import");

    fs::write(&imported.package_path, b"tampered bytes").expect("stored package should mutate");

    assert!(matches!(
        store.verify_script_package_hash("script-1"),
        Err(StorageError::HashMismatch { .. })
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
fn appends_and_filters_run_records() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("script.bbs");
    fs::write(&package_path, b"package bytes").expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    store
        .import_script(ImportScriptRequest {
            id: "script-1".to_owned(),
            name: "Script One".to_owned(),
            package_source: package_path,
            package_format_version: 1,
            script_language_version: 1,
            target_runtime: "Generic Desktop".to_owned(),
            asset_count: 0,
            risk_level: "low".to_owned(),
        })
        .expect("script should import");

    store
        .append_run_record(test_run_record("run-1", "script-1", 10))
        .expect("first run should append");
    store
        .append_run_record(test_run_record("run-2", "script-2", 20))
        .expect("second run should append");
    store
        .append_run_record(test_run_record("run-3", "script-1", 30))
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

fn test_run_record(run_id: &str, script_id: &str, completed_at_unix: u64) -> StoredRunRecord {
    StoredRunRecord {
        completed_at_unix,
        logs: vec![RunLogEntry {
            level: "info".to_owned(),
            message: format!("{run_id} completed"),
            node_id: None,
        }],
        run_id: run_id.to_owned(),
        script_id: script_id.to_owned(),
        status: "completed".to_owned(),
        trigger_node_id: "n-trigger".to_owned(),
        variables: BTreeMap::new(),
    }
}
