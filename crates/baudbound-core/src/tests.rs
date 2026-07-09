use std::{
    fs,
    io::{Cursor, Write},
    sync::Mutex,
};

use baudbound_runtime::{
    RuntimeActionError, RuntimeActionHandler, RuntimeActionRequest, RuntimeActionResult,
    RuntimeContext,
};
use baudbound_script::{Capabilities, Manifest, Permissions, RiskLevel};
use baudbound_storage::FilesystemScriptStore;
use serde_json::{Map, Value, json};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use super::*;

#[derive(Default)]
struct RecordingActionHandler {
    actions: Mutex<Vec<String>>,
}

impl RuntimeActionHandler for RecordingActionHandler {
    fn execute_action(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        self.actions
            .lock()
            .expect("recording action lock should not be poisoned")
            .push(request.action_type.clone());
        Ok(RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), Value::Bool(true))]),
        })
    }
}

#[test]
fn creates_failed_run_record_with_package_identity() {
    let package = ScriptPackage {
        capabilities: Capabilities {
            required_capabilities: Vec::new(),
            target_runtime: "Generic Desktop".to_owned(),
        },
        editor: None,
        entries: Vec::new(),
        manifest: Manifest {
            format_version: 1,
            script_language_version: 1,
            id: "script-1".to_owned(),
            name: "Script One".to_owned(),
            description: String::new(),
            author: String::new(),
            website: String::new(),
            repository: String::new(),
            created_with: "test".to_owned(),
            created_at: "2026-01-01T00:00:00.000Z".to_owned(),
            updated_at: String::new(),
            tags: Vec::new(),
            minimum_runner_version: "0.1.0".to_owned(),
            assets: Vec::new(),
        },
        permissions: Permissions {
            declared_permissions: Vec::new(),
            risk_level: RiskLevel::Low,
        },
        program: json!({
            "entry": {
                "trigger": {
                    "id": "n-trigger",
                    "action_type": "trigger.manual",
                    "type": "manual",
                    "config": {},
                    "runtime_outputs": []
                },
                "triggers": [],
                "program": {
                    "steps": [],
                    "edges": []
                }
            }
        }),
    };

    let record = run_records::failed_run_record(&package, None, "permission denied".to_owned());

    assert_eq!(record.script_id, "script-1");
    assert_eq!(record.trigger_node_id, "n-trigger");
    assert_eq!(record.status, "failed");
    assert!(record.run_id.starts_with("script-1:n-trigger:"));
    assert_eq!(record.logs[0].message, "permission denied");
}

#[test]
fn current_script_approval_allows_policy_blocked_permissions() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("package should import");

    let unapproved = core
        .run_installed(&store, "network-trigger")
        .expect_err("unapproved network trigger should be blocked");
    assert!(matches!(unapproved, CoreError::Security(_)));

    let approval = core
        .approve_installed(&store, "network-trigger")
        .expect("package should approve");
    assert_eq!(approval.approved_permissions, ["webhook_public_bind"]);

    let report = core
        .run_installed(&store, "network-trigger")
        .expect("approved package should run");
    assert_eq!(report.identity.script_id, "network-trigger");
}

#[test]
fn installed_package_lifecycle_uses_real_bbs_packages() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    let updated_package_path = temporary_directory
        .path()
        .join("network-trigger-updated.bbs");
    fs::write(
        &package_path,
        create_policy_test_package_with_webhook("network-trigger", "hook"),
    )
    .expect("test package should be written");
    fs::write(
        &updated_package_path,
        create_policy_test_package_with_webhook("network-trigger-updated", "updated-hook"),
    )
    .expect("updated test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let core = RunnerCore::default();

    let imported = core
        .import_package(&store, &package_path)
        .expect("package should import");
    assert_eq!(imported.id, "network-trigger");
    assert_eq!(imported.package_file_name, "network-trigger.bbs");
    assert!(imported.package_path.exists());
    assert!(store.verify_script_package_hash("network-trigger").is_ok());

    let blocked = core
        .run_installed(&store, "network-trigger")
        .expect_err("unapproved high-risk package should be blocked");
    assert!(matches!(blocked, CoreError::Security(_)));
    assert_eq!(
        store
            .list_run_records(Some("network-trigger"), None)
            .expect("failed run record should list")
            .first()
            .expect("failed run record should exist")
            .status,
        "failed"
    );

    core.approve_installed(&store, "network-trigger")
        .expect("package should approve");
    let report = core
        .dispatch_trigger_event(
            &store,
            TriggerEvent {
                node_id: "n-webhook".to_owned(),
                payload: json!({"body": "hello from lifecycle test"}),
                script_id: "network-trigger".to_owned(),
            },
        )
        .expect("approved trigger should run");
    assert_eq!(report.identity.trigger_node_id, "n-webhook");
    assert_eq!(
        report.variables.get("n-webhook.body"),
        Some(&json!("hello from lifecycle test"))
    );

    let run_records = store
        .list_run_records(Some("network-trigger"), None)
        .expect("run records should list");
    assert_eq!(
        run_records
            .iter()
            .map(|record| record.status.as_str())
            .collect::<Vec<_>>(),
        ["completed", "failed"]
    );

    let updated = core
        .update_package(&store, &updated_package_path)
        .expect("installed package should update");
    assert_eq!(updated.id, "network-trigger");
    assert_eq!(updated.name, "network-trigger-updated");
    assert_eq!(updated.package_file_name, "network-trigger-updated.bbs");
    assert!(!imported.package_path.exists());
    assert!(updated.package_path.exists());
    assert!(
        store
            .find_script_approval("network-trigger")
            .expect("approval lookup should succeed")
            .is_none()
    );

    let registrations = core
        .list_trigger_registrations(&store, Some("network-trigger"))
        .expect("updated trigger registrations should list");
    let webhook = registrations
        .iter()
        .find(|registration| registration.node_id == "n-webhook")
        .expect("webhook registration should exist");
    assert_eq!(webhook.config["hookName"], "updated-hook");

    core.remove_installed(&store, "network-trigger")
        .expect("installed package should remove");
    assert!(
        core.list_installed(&store)
            .expect("installed scripts should list")
            .is_empty()
    );
    assert!(!updated.package_path.exists());
}

#[test]
fn custom_action_handler_is_used_for_script_execution() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("action-handler-test.bbs");
    fs::write(&package_path, create_action_handler_test_package())
        .expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let handler = Arc::new(RecordingActionHandler::default());
    let core = RunnerCore::default().with_action_handler(handler.clone());
    core.import_package(&store, &package_path)
        .expect("package should import");
    let report = core
        .run_installed(&store, "action-handler-test")
        .expect("script should run with injected action handler");

    assert_eq!(
        report.variables.get("n-format.handled"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        handler
            .actions
            .lock()
            .expect("recording action lock should not be poisoned")
            .as_slice(),
        &["action.text.format".to_owned()]
    );
}

#[test]
fn import_rejects_desktop_actions_for_headless_target_runtime() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("headless-notification.bbs");
    fs::write(
        &package_path,
        create_target_runtime_test_package(
            "headless-notification",
            "Generic Headless",
            "action.notification",
        ),
    )
    .expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let error = RunnerCore::default()
        .import_package(&store, &package_path)
        .expect_err("desktop-only action should not import into headless target");

    assert!(
        error
            .to_string()
            .contains("requires a desktop target runtime"),
        "{error}"
    );
}

#[test]
fn import_rejects_windows_only_actions_for_non_windows_desktop_target_runtime() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("linux-pixel.bbs");
    fs::write(
        &package_path,
        create_target_runtime_test_package("linux-pixel", "Linux Desktop", "action.pixel.get"),
    )
    .expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let error = RunnerCore::default()
        .import_package(&store, &package_path)
        .expect_err("Windows-only action should not import into Linux Desktop target");

    assert!(
        error.to_string().contains("requires Windows Desktop"),
        "{error}"
    );
}

#[test]
fn import_rejects_removed_target_runtime() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("unsupported-target.bbs");
    fs::write(
        &package_path,
        create_target_runtime_test_package(
            "unsupported-target",
            &format!("{} Desktop", ["mac", "OS"].join("")),
            "action.text.format",
        ),
    )
    .expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let error = RunnerCore::default()
        .import_package(&store, &package_path)
        .expect_err("removed target runtime should not import");

    assert!(
        error.to_string().contains("unsupported target runtime"),
        "{error}"
    );
}

#[test]
fn validate_rejects_packages_that_require_newer_runner() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("future-runner.bbs");
    fs::write(
        &package_path,
        create_minimum_runner_version_test_package("future-runner", "999.0.0"),
    )
    .expect("test package should be written");

    let error = RunnerCore::default()
        .validate_package(&package_path)
        .expect_err("package requiring a newer runner should fail validation");

    assert!(matches!(error, CoreError::Version(_)), "{error}");
    assert!(
        error
            .to_string()
            .contains("requires runner version 999.0.0")
    );
}

#[test]
fn import_rejects_packages_that_require_newer_runner() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("future-runner.bbs");
    fs::write(
        &package_path,
        create_minimum_runner_version_test_package("future-runner", "999.0.0"),
    )
    .expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let error = RunnerCore::default()
        .import_package(&store, &package_path)
        .expect_err("package requiring a newer runner should not import");

    assert!(matches!(error, CoreError::Version(_)), "{error}");
}

#[test]
fn run_rejects_installed_package_that_requires_newer_runner() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("future-runner.bbs");
    fs::write(
        &package_path,
        create_minimum_runner_version_test_package("future-runner", "999.0.0"),
    )
    .expect("test package should be written");

    let package = load_script_package(&package_path).expect("test package should load");
    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    store
        .import_script(import_request_from_package(&package_path, package))
        .expect("test package should be inserted into storage");

    let error = RunnerCore::default()
        .run_installed(&store, "future-runner")
        .expect_err("installed package requiring a newer runner should not run");

    assert!(matches!(error, CoreError::Version(_)), "{error}");
    let records = store
        .list_run_records(Some("future-runner"), None)
        .expect("failed run record should list");
    assert_eq!(records.len(), 1);
    assert!(
        records[0]
            .logs
            .iter()
            .any(|log| log.message.contains("requires runner version 999.0.0")),
        "{records:?}"
    );
}

#[test]
fn trigger_registration_rejects_installed_package_that_requires_newer_runner() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("future-runner.bbs");
    fs::write(
        &package_path,
        create_minimum_runner_version_test_package("future-runner", "999.0.0"),
    )
    .expect("test package should be written");

    let package = load_script_package(&package_path).expect("test package should load");
    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    store
        .import_script(import_request_from_package(&package_path, package))
        .expect("test package should be inserted into storage");

    let error = RunnerCore::default()
        .list_trigger_registrations(&store, None)
        .expect_err("incompatible installed package should not register triggers");

    assert!(matches!(error, CoreError::Version(_)), "{error}");
}

#[test]
fn status_reports_installed_package_that_requires_newer_runner() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("future-runner.bbs");
    fs::write(
        &package_path,
        create_minimum_runner_version_test_package("future-runner", "999.0.0"),
    )
    .expect("test package should be written");

    let package = load_script_package(&package_path).expect("test package should load");
    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    store
        .import_script(import_request_from_package(&package_path, package))
        .expect("test package should be inserted into storage");

    let status = RunnerCore::default()
        .status(&store)
        .expect("status should still build");

    assert_eq!(status.scripts.len(), 1);
    assert!(
        status.scripts[0]
            .package_error
            .as_deref()
            .is_some_and(|message| message.contains("requires runner version 999.0.0")),
        "{status:?}"
    );
}

#[test]
fn configured_runner_target_runtimes_reject_other_package_targets() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("desktop-text.bbs");
    fs::write(
        &package_path,
        create_target_runtime_test_package("desktop-text", "Generic Desktop", "action.text.format"),
    )
    .expect("test package should be written");
    let config = RunnerConfig {
        runner: RunnerSettings {
            name: Some("Headless Test Runner".to_owned()),
            target_runtimes: vec!["Generic Headless".to_owned()],
            trigger_reload_seconds: DEFAULT_TRIGGER_RELOAD_SECONDS,
        },
        ..RunnerConfig::default()
    };

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let error = RunnerCore::from_config(&config)
        .import_package(&store, &package_path)
        .expect_err("headless-only runner should reject desktop package target");

    assert!(
        error
            .to_string()
            .contains("this runner supports only Generic Headless"),
        "{error}"
    );
}

#[test]
fn sub_script_action_runs_installed_manual_script() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let child_package_path = temporary_directory.path().join("child-script.bbs");
    let parent_package_path = temporary_directory.path().join("parent-script.bbs");
    fs::write(&child_package_path, create_action_handler_test_package())
        .expect("child test package should be written");
    fs::write(
        &parent_package_path,
        create_sub_script_parent_package("parent-script", "action-handler-test"),
    )
    .expect("parent test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let core = RunnerCore::default();
    core.import_package(&store, &child_package_path)
        .expect("child package should import");
    core.import_package(&store, &parent_package_path)
        .expect("parent package should import");
    core.approve_installed(&store, "parent-script")
        .expect("sub-script parent should approve");

    let report = core
        .run_installed(&store, "parent-script")
        .expect("parent should run sub-script");

    assert_eq!(
        report.variables.get("n-sub.status"),
        Some(&json!("completed"))
    );
    assert_eq!(report.variables.get("n-sub.exit_code"), Some(&json!(0)));
    assert_eq!(
        report.variables.get("n-sub.script_id"),
        Some(&json!("action-handler-test"))
    );

    let child_runs = store
        .list_run_records(Some("action-handler-test"), None)
        .expect("child run records should list");
    assert_eq!(child_runs.len(), 1);
    assert_eq!(child_runs[0].status, "completed");
}

#[test]
fn sub_script_action_rejects_recursive_cycle() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("recursive-script.bbs");
    fs::write(
        &package_path,
        create_sub_script_parent_package("recursive-script", "recursive-script"),
    )
    .expect("recursive test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("recursive package should import");
    core.approve_installed(&store, "recursive-script")
        .expect("recursive package should approve");

    let error = core
        .run_installed(&store, "recursive-script")
        .expect_err("recursive sub-script should fail");

    assert!(error.to_string().contains("sub-script cycle detected"));
    let runs = store
        .list_run_records(Some("recursive-script"), None)
        .expect("failed run record should list");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "failed");
}

#[test]
fn lists_trigger_registrations_for_installed_scripts() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("package should import");

    let registrations = core
        .list_trigger_registrations(&store, Some("network-trigger"))
        .expect("trigger registrations should list");

    assert_eq!(registrations.len(), 2);
    assert!(
        registrations
            .iter()
            .any(|registration| registration.node_id == "n-manual"
                && registration.action_type == "trigger.manual")
    );
    let webhook = registrations
        .iter()
        .find(|registration| registration.node_id == "n-webhook")
        .expect("webhook trigger should be registered");
    assert_eq!(webhook.runner_type, "webhook");
    assert_eq!(webhook.config["hookName"], "hook");
}

#[test]
fn disabled_scripts_are_omitted_from_global_trigger_registrations() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("package should import");

    assert!(
        !core
            .list_trigger_registrations(&store, None)
            .expect("enabled trigger registrations should list")
            .is_empty()
    );

    let disabled = core
        .set_installed_enabled(&store, "network-trigger", false)
        .expect("script should disable");
    assert!(!disabled.enabled);

    assert!(
        core.list_trigger_registrations(&store, None)
            .expect("global trigger registrations should list")
            .is_empty()
    );
    assert!(
        !core
            .list_trigger_registrations(&store, Some("network-trigger"))
            .expect("direct trigger registrations should list")
            .is_empty()
    );
}

#[test]
fn status_reports_script_health_and_approval_state() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("package should import");

    let status = core.status(&store).expect("status should build");
    assert_eq!(status.runner_name, "BaudBound Runner");
    assert!(
        status
            .supported_target_runtimes
            .contains(&"Generic Desktop".to_owned())
    );
    assert_eq!(status.total_script_count, 1);
    assert_eq!(status.enabled_script_count, 1);
    assert_eq!(status.disabled_script_count, 0);
    assert_eq!(status.problem_count, 0);
    assert_eq!(status.trigger_count, 2);
    assert!(matches!(
        status.scripts[0].package_hash_status,
        PackageHashStatus::Valid
    ));
    assert!(matches!(
        status.scripts[0].approval_status,
        ApprovalStatus::Missing
    ));
    assert_eq!(
        status.scripts[0].declared_permissions,
        ["webhook_public_bind"]
    );

    core.approve_installed(&store, "network-trigger")
        .expect("package should approve");
    core.set_installed_enabled(&store, "network-trigger", false)
        .expect("script should disable");

    let status = core.status(&store).expect("status should build");
    assert_eq!(status.enabled_script_count, 0);
    assert_eq!(status.disabled_script_count, 1);
    assert_eq!(status.trigger_count, 0);
    assert!(matches!(
        status.scripts[0].approval_status,
        ApprovalStatus::Current
    ));
}

#[test]
fn dispatches_trigger_event_through_core_dispatcher() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");

    let store = FilesystemScriptStore::new(temporary_directory.path().join("store"));
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("package should import");
    core.approve_installed(&store, "network-trigger")
        .expect("package should approve");

    let report = core
        .trigger_dispatcher(&store)
        .dispatch(TriggerEvent {
            node_id: "n-webhook".to_owned(),
            payload: json!({"body": "hello"}),
            script_id: "network-trigger".to_owned(),
        })
        .expect("trigger event should dispatch");

    assert_eq!(report.identity.script_id, "network-trigger");
    assert_eq!(report.identity.trigger_node_id, "n-webhook");
    assert_eq!(
        report.variables.get("n-webhook.body"),
        Some(&json!("hello"))
    );
}

fn create_policy_test_package() -> Vec<u8> {
    create_policy_test_package_with_webhook("network-trigger", "hook")
}

fn create_policy_test_package_with_webhook(script_name: &str, hook_name: &str) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    let manifest = format!(
        r#"{{
                    "format_version": 1,
                    "script_language_version": 1,
                    "id": "network-trigger",
                    "name": "{script_name}",
                    "created_with": "BaudBound Test",
                    "created_at": "2026-01-01T00:00:00.000Z",
                    "minimum_runner_version": "0.1.0"
                }}"#
    );
    let program = format!(
        r#"{{
                    "entry": {{
                        "trigger": {{
                            "id": "n-manual",
                            "action_type": "trigger.manual",
                            "type": "manual",
                            "config": {{}},
                            "runtime_outputs": []
                        }},
                        "triggers": [
                            {{
                                "id": "n-manual",
                                "action_type": "trigger.manual",
                                "type": "manual",
                                "config": {{}},
                                "runtime_outputs": []
                            }},
                            {{
                                "id": "n-webhook",
                                "action_type": "trigger.webhook",
                                "type": "webhook",
                                "config": {{"method": "POST", "hookName": "{hook_name}"}},
                                "runtime_outputs": []
                            }}
                        ],
                        "program": {{"type": "block", "steps": [], "edges": []}}
                    }}
                }}"#
    );

    for (path, content) in [
        ("manifest.json", manifest.as_str()),
        ("program.json", program.as_str()),
        (
            "permissions.json",
            r#"{"declared_permissions": ["webhook_public_bind"], "risk_level": "high"}"#,
        ),
        (
            "capabilities.json",
            r#"{"required_capabilities": [], "target_runtime": "Generic Desktop"}"#,
        ),
    ] {
        writer
            .start_file(path, options)
            .expect("test zip file should start");
        writer
            .write_all(content.as_bytes())
            .expect("test zip content should write");
    }

    writer
        .finish()
        .expect("test zip should finish")
        .into_inner()
}

fn create_sub_script_parent_package(script_id: &str, target_script: &str) -> Vec<u8> {
    let manifest = format!(
        r#"{{
                "format_version": 1,
                "script_language_version": 1,
                "id": "{script_id}",
                "name": "{script_id}",
                "created_with": "BaudBound Test",
                "created_at": "2026-01-01T00:00:00.000Z",
                "minimum_runner_version": "0.1.0"
            }}"#
    );
    let program = format!(
        r#"{{
                "entry": {{
                    "trigger": {{
                        "id": "n-manual",
                        "action_type": "trigger.manual",
                        "type": "manual",
                        "config": {{}},
                        "runtime_outputs": []
                    }},
                    "triggers": [
                        {{
                            "id": "n-manual",
                            "action_type": "trigger.manual",
                            "type": "manual",
                            "config": {{}},
                            "runtime_outputs": []
                        }}
                    ],
                    "program": {{
                        "type": "block",
                        "steps": [
                            {{
                                "id": "n-sub",
                                "action_type": "action.script.run",
                                "type": "action",
                                "action": "run_sub_script",
                                "config": {{
                                    "script": "{target_script}"
                                }},
                                "runtime_outputs": []
                            }}
                        ],
                        "edges": [
                            {{
                                "source": "n-manual",
                                "source_handle": "out",
                                "target": "n-sub",
                                "target_handle": "input"
                            }}
                        ]
                    }}
                }}
            }}"#
    );

    create_test_package([
        ("manifest.json", manifest.as_str()),
        ("program.json", program.as_str()),
        (
            "permissions.json",
            r#"{"declared_permissions": ["sub_script_run"], "risk_level": "high"}"#,
        ),
        (
            "capabilities.json",
            r#"{"required_capabilities": [], "target_runtime": "Generic Desktop"}"#,
        ),
    ])
}

fn create_action_handler_test_package() -> Vec<u8> {
    create_test_package([
        (
            "manifest.json",
            r#"{
                    "format_version": 1,
                    "script_language_version": 1,
                    "id": "action-handler-test",
                    "name": "action-handler-test",
                    "created_with": "BaudBound Test",
                    "created_at": "2026-01-01T00:00:00.000Z",
                    "minimum_runner_version": "0.1.0"
                }"#,
        ),
        (
            "program.json",
            r#"{
                    "entry": {
                        "trigger": {
                            "id": "n-manual",
                            "action_type": "trigger.manual",
                            "type": "manual",
                            "config": {},
                            "runtime_outputs": []
                        },
                        "triggers": [
                            {
                                "id": "n-manual",
                                "action_type": "trigger.manual",
                                "type": "manual",
                                "config": {},
                                "runtime_outputs": []
                            }
                        ],
                        "program": {
                            "type": "block",
                            "steps": [
                                {
                                    "id": "n-format",
                                    "action_type": "action.text.format",
                                    "type": "action",
                                    "action": "format_text",
                                    "config": {
                                        "operation": "uppercase",
                                        "input": "hello"
                                    },
                                    "runtime_outputs": []
                                }
                            ],
                            "edges": [
                                {
                                    "source": "n-manual",
                                    "source_handle": "out",
                                    "target": "n-format",
                                    "target_handle": "input"
                                }
                            ]
                        }
                    }
                }"#,
        ),
        (
            "permissions.json",
            r#"{"declared_permissions": ["text_transform"], "risk_level": "low"}"#,
        ),
        (
            "capabilities.json",
            r#"{"required_capabilities": [], "target_runtime": "Generic Desktop"}"#,
        ),
    ])
}

fn create_target_runtime_test_package(
    script_id: &str,
    target_runtime: &str,
    action_type: &str,
) -> Vec<u8> {
    create_target_runtime_test_package_with_action_config(
        script_id,
        target_runtime,
        action_type,
        "{}",
    )
}

fn create_target_runtime_test_package_with_action_config(
    script_id: &str,
    target_runtime: &str,
    action_type: &str,
    action_config: &str,
) -> Vec<u8> {
    let manifest = format!(
        r#"{{
                "format_version": 1,
                "script_language_version": 1,
                "id": "{script_id}",
                "name": "{script_id}",
                "created_with": "BaudBound Test",
                "created_at": "2026-01-01T00:00:00.000Z",
                "minimum_runner_version": "0.1.0"
            }}"#
    );
    let program = format!(
        r#"{{
                "entry": {{
                    "trigger": {{
                        "id": "n-manual",
                        "action_type": "trigger.manual",
                        "type": "manual",
                        "config": {{}},
                        "runtime_outputs": []
                    }},
                    "triggers": [
                        {{
                            "id": "n-manual",
                            "action_type": "trigger.manual",
                            "type": "manual",
                            "config": {{}},
                            "runtime_outputs": []
                        }}
                    ],
                    "program": {{
                        "type": "block",
                        "steps": [
                            {{
                                "id": "n-native",
                                "action_type": "{action_type}",
                                "type": "action",
                                "config": {action_config},
                                "runtime_outputs": []
                            }}
                        ],
                        "edges": []
                    }}
                }}
            }}"#
    );
    let capabilities =
        format!(r#"{{"required_capabilities": [], "target_runtime": "{target_runtime}"}}"#);

    create_test_package([
        ("manifest.json", manifest.as_str()),
        ("program.json", program.as_str()),
        (
            "permissions.json",
            r#"{"declared_permissions": [], "risk_level": "low"}"#,
        ),
        ("capabilities.json", capabilities.as_str()),
    ])
}

fn create_minimum_runner_version_test_package(script_id: &str, minimum_version: &str) -> Vec<u8> {
    let manifest = format!(
        r#"{{
                "format_version": 1,
                "script_language_version": 1,
                "id": "{script_id}",
                "name": "{script_id}",
                "created_with": "BaudBound Test",
                "created_at": "2026-01-01T00:00:00.000Z",
                "minimum_runner_version": "{minimum_version}"
            }}"#
    );

    create_test_package([
        ("manifest.json", manifest.as_str()),
        (
            "program.json",
            r#"{
                    "entry": {
                        "trigger": {
                            "id": "n-manual",
                            "action_type": "trigger.manual",
                            "type": "manual",
                            "config": {},
                            "runtime_outputs": []
                        },
                        "triggers": [
                            {
                                "id": "n-manual",
                                "action_type": "trigger.manual",
                                "type": "manual",
                                "config": {},
                                "runtime_outputs": []
                            }
                        ],
                        "program": {"type": "block", "steps": [], "edges": []}
                    }
                }"#,
        ),
        (
            "permissions.json",
            r#"{"declared_permissions": [], "risk_level": "low"}"#,
        ),
        (
            "capabilities.json",
            r#"{"required_capabilities": [], "target_runtime": "Generic Desktop"}"#,
        ),
    ])
}

fn create_test_package<const N: usize>(files: [(&str, &str); N]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for (path, content) in files {
        writer
            .start_file(path, options)
            .expect("test zip file should start");
        writer
            .write_all(content.as_bytes())
            .expect("test zip content should write");
    }

    writer
        .finish()
        .expect("test zip should finish")
        .into_inner()
}
