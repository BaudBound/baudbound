use std::{
    fs,
    io::{Cursor, Write},
    path::Path,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

use baudbound_runtime::{
    RunIdentity, RuntimeActionError, RuntimeActionHandler, RuntimeActionRequest,
    RuntimeActionResult, RuntimeCancellationToken, RuntimeContext, RuntimeLogEntry,
    RuntimeOutputLimits, RuntimeRunObserver,
};
use baudbound_script::{Capabilities, Manifest, Permissions, RiskLevel};
use baudbound_storage::SqliteRunnerStore;
use serde_json::{Map, Value, json};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use super::*;

#[path = "tests/sub_script.rs"]
mod sub_script;

#[cfg(windows)]
fn test_headless_runtime() -> &'static str {
    "Windows Headless"
}

#[cfg(unix)]
fn test_headless_runtime() -> &'static str {
    "Linux Headless"
}

#[cfg(windows)]
fn test_desktop_runtime() -> &'static str {
    "Windows Desktop"
}

#[cfg(unix)]
fn test_desktop_runtime() -> &'static str {
    "Linux Desktop"
}

fn test_store(temporary_directory: &tempfile::TempDir) -> SqliteRunnerStore {
    SqliteRunnerStore::open(
        temporary_directory
            .path()
            .join("store")
            .join("runner.sqlite3"),
    )
    .expect("SQLite test store should open")
}

#[test]
fn staged_packages_are_independent_of_the_selected_source_file() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = directory.path().join("selected.bbs");
    fs::write(&source, b"validated package bytes").expect("source package should be written");

    let staged = StagedPackage::copy_from(&source).expect("package should be staged");
    fs::write(&source, b"replacement bytes").expect("source package should be replaced");

    assert_eq!(
        fs::read(&staged.path).expect("staged package should remain readable"),
        b"validated package bytes"
    );
}

#[test]
fn installed_package_trust_boundaries_reject_replaced_bytes() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");
    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    let installed = core
        .import_package(&store, &package_path)
        .expect("package should import");

    fs::write(&installed.package_path, b"attacker-controlled replacement")
        .expect("installed package should be replaceable for the test");

    assert!(matches!(
        core.approve_installed(&store, &installed.id),
        Err(CoreError::Storage(StorageError::HashMismatch { .. }))
    ));
    assert!(matches!(
        core.list_trigger_registrations(&store, Some(&installed.id)),
        Err(CoreError::Storage(StorageError::HashMismatch { .. }))
    ));
    assert!(matches!(
        core.run_installed(&store, &installed.id),
        Err(CoreError::Storage(StorageError::HashMismatch { .. }))
    ));
}

#[test]
fn package_bytes_changed_during_snapshot_cannot_validate_as_approved() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");
    let store = test_store(&temporary_directory);
    let installed = RunnerCore::default()
        .import_package(&store, &package_path)
        .expect("package should import");
    let mut mixed_bytes = fs::read(&installed.package_path).expect("installed bytes should read");
    let changed_index = mixed_bytes.len() / 2;
    mixed_bytes[changed_index] ^= 0xff;

    let error = StagedPackage::verified_from_bytes(&installed, Arc::from(mixed_bytes))
        .err()
        .expect("a snapshot containing changed bytes must be rejected");

    assert!(matches!(
        error,
        CoreError::Storage(StorageError::HashMismatch { .. })
    ));
}

struct FirstRunBlockingActionHandler {
    entered: Mutex<Option<mpsc::Sender<()>>>,
    release: Mutex<mpsc::Receiver<()>>,
}

impl RuntimeActionHandler for FirstRunBlockingActionHandler {
    fn execute_action(
        &self,
        _request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        if let Some(entered) = self
            .entered
            .lock()
            .expect("entry signal lock should not be poisoned")
            .take()
        {
            entered
                .send(())
                .expect("first run entry signal should be observed");
            self.release
                .lock()
                .expect("release signal lock should not be poisoned")
                .recv()
                .expect("first run should be released");
        }
        Ok(RuntimeActionResult {
            output_data: Map::from_iter([("handled".to_owned(), Value::Bool(true))]),
            sensitive_output_keys: Default::default(),
        })
    }
}

#[test]
fn queued_run_snapshots_package_only_after_acquiring_its_execution_permit() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("action-handler-test.bbs");
    fs::write(&package_path, create_action_handler_test_package())
        .expect("test package should be written");
    let store = test_store(&temporary_directory);
    let (entered_sender, entered_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let handler = Arc::new(FirstRunBlockingActionHandler {
        entered: Mutex::new(Some(entered_sender)),
        release: Mutex::new(release_receiver),
    });
    let core = RunnerCore::default().with_action_handler(handler);
    let installed = core
        .import_package(&store, &package_path)
        .expect("package should import");
    core.approve_installed(&store, &installed.id)
        .expect("package should approve");

    let first = {
        let core = core.clone();
        let store = store.clone();
        let script_id = installed.id.clone();
        thread::spawn(move || core.run_installed(&store, &script_id))
    };
    entered_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("first run should reach its action");

    let (second_started_sender, second_started_receiver) = mpsc::channel();
    let second = {
        let core = core.clone();
        let store = store.clone();
        let script_id = installed.id.clone();
        thread::spawn(move || {
            second_started_sender
                .send(())
                .expect("second run start should be observed");
            core.run_installed(&store, &script_id)
        })
    };
    second_started_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("second run should start acquiring its permit");
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while core.execution_queue.waiting_count(&installed.id) == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "second run did not enter the execution queue"
        );
        thread::yield_now();
    }

    fs::write(&installed.package_path, b"replacement after first snapshot")
        .expect("installed package should be replaced for the race test");
    release_sender
        .send(())
        .expect("first run release should be delivered");

    first
        .join()
        .expect("first run thread should finish")
        .expect("the first run must execute its immutable approved snapshot");
    assert!(matches!(
        second.join().expect("second run thread should finish"),
        Err(CoreError::Storage(StorageError::HashMismatch { .. }))
    ));
}

#[derive(Default)]
struct RecordingActionHandler {
    actions: Mutex<Vec<String>>,
}

struct RecordingRunObserver {
    observed_record_counts: Mutex<Vec<usize>>,
    store: SqliteRunnerStore,
}

impl RuntimeRunObserver for RecordingRunObserver {
    fn run_started(&self, _identity: &RunIdentity, _cancellation: RuntimeCancellationToken) {}

    fn log_emitted(&self, _identity: &RunIdentity, _entry: &RuntimeLogEntry) {}

    fn run_finished(&self, _identity: &RunIdentity) {}

    fn run_recorded(&self) {
        let count = self
            .store
            .list_run_records(None, None)
            .expect("committed run records should be readable")
            .len();
        self.observed_record_counts
            .lock()
            .expect("observer count lock should not be poisoned")
            .push(count);
    }
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
            sensitive_output_keys: Default::default(),
        })
    }
}

#[test]
fn creates_failed_run_record_with_package_identity() {
    let package = ScriptPackage {
        capabilities: Capabilities {
            required_capabilities: Vec::new(),
            target_runtimes: vec![test_headless_runtime().to_owned()],
        },
        editor: None,
        entries: Vec::new(),
        manifest: Manifest {
            variables: Vec::new(),
            settings: Vec::new(),
            format_version: 1,
            script_language_version: 1,
            id: "script-1".to_owned(),
            name: "Script One".to_owned(),
            description: String::new(),
            author: String::new(),
            website: String::new(),
            source: String::new(),
            created_with: "test".to_owned(),
            created_at: "2026-01-01T00:00:00.000Z".to_owned(),
            updated_at: String::new(),
            tags: Vec::new(),
            minimum_runner_version: "0.1.0".to_owned(),
            version: "1.0.0".to_owned(),
            repository_url: String::new(),
            assets: Vec::new(),
            secrets: Vec::new(),
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

    let record = run_records::failed_run_record(
        &package,
        None,
        "permission denied".to_owned(),
        RuntimeOutputLimits::default(),
    );

    assert_eq!(record.script_id, "script-1");
    assert_eq!(record.trigger_node_id, "n-trigger");
    assert_eq!(record.status, "failed");
    assert!(record.run_id.starts_with("script-1:n-trigger:"));
    assert_eq!(record.logs[0].message, "permission denied");
    assert!(record.logs[0].timestamp_unix_ms > 0);
}

#[test]
fn cancelled_execution_is_persisted_as_cancelled() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("cancellable.bbs");
    fs::write(&package_path, create_cancellable_test_package())
        .expect("cancellable package should be written");
    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("cancellable package should import");
    core.approve_installed(&store, "cancellable")
        .expect("cancellable package should approve");

    let cancellation = RuntimeCancellationToken::new();
    let thread_cancellation = cancellation.clone();
    let cancel_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(25));
        thread_cancellation.cancel();
    });
    let error = core
        .run_installed_with_trigger_and_cancellation(
            &store,
            "cancellable",
            None,
            Value::Null,
            cancellation,
        )
        .expect_err("cancelled package should stop");
    cancel_thread
        .join()
        .expect("cancellation thread should join");

    assert!(matches!(
        error,
        CoreError::Runtime(baudbound_runtime::RuntimeError::Cancelled)
    ));
    let records = store
        .list_run_records(Some("cancellable"), None)
        .expect("cancelled run history should load");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].status, "cancelled");
    assert_eq!(records[0].logs[0].level, "warning");
    assert!(records[0].logs[0].timestamp_unix_ms > 0);
}

#[test]
fn current_script_approval_allows_policy_blocked_permissions() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("package should import");

    let unapproved = core
        .run_installed(&store, "network-trigger")
        .expect_err("unapproved network trigger should be blocked");
    assert!(matches!(unapproved, CoreError::ApprovalRequired(_)));

    let approval = core
        .approve_installed(&store, "network-trigger")
        .expect("package should approve");
    assert_eq!(approval.approval.approved_permissions, ["network.webhook"]);

    let report = core
        .run_installed(&store, "network-trigger")
        .expect("approved package should run");
    assert_eq!(report.identity.script_id, "network-trigger");
}

#[test]
fn public_listener_policy_does_not_block_approved_network_trigger_packages() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");
    let store = test_store(&temporary_directory);
    let permissive_core = RunnerCore::default();
    permissive_core
        .import_package(&store, &package_path)
        .expect("package should import before policy is restricted");
    permissive_core
        .approve_installed(&store, "network-trigger")
        .expect("package should be approved before policy is restricted");

    let loopback_only_core = core_with_policy(true, true, false);
    let registrations = loopback_only_core
        .list_trigger_registrations(&store, None)
        .expect("listener exposure is enforced when a listener starts");
    assert!(
        registrations
            .iter()
            .any(|registration| registration.action_type == "trigger.webhook")
    );
    loopback_only_core
        .run_installed(&store, "network-trigger")
        .expect("listener exposure policy must not change package execution approval");
    let status = loopback_only_core
        .status(&store)
        .expect("status should build");
    assert!(status.scripts[0].package_error.is_none(), "{status:?}");
}

#[test]
fn configured_policy_blocks_approved_dangerous_permissions() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("dangerous-process.bbs");
    fs::write(
        &package_path,
        create_action_policy_test_package(
            "dangerous-process",
            "action.process.run",
            "run_process",
            r#"{"arguments":[],"executable":"unused","workingDirectory":""}"#,
            "process.run",
            "dangerous",
        ),
    )
    .expect("test package should be written");
    let store = test_store(&temporary_directory);
    let permissive_core = RunnerCore::default();
    permissive_core
        .import_package(&store, &package_path)
        .expect("package should import before policy is restricted");
    permissive_core
        .approve_installed(&store, "dangerous-process")
        .expect("package should be approved before policy is restricted");

    let restricted_core = core_with_policy(true, false, true);
    let error = restricted_core
        .run_installed(&store, "dangerous-process")
        .expect_err("Dangerous permission must be blocked");
    assert!(
        error
            .to_string()
            .contains("security.policy.allow_dangerous_permissions"),
        "{error}"
    );
}

#[test]
fn configured_policy_blocks_approved_shell_commands_independently() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("shell-command.bbs");
    fs::write(
        &package_path,
        create_action_policy_test_package(
            "shell-command",
            "action.shell",
            "run_shell_command",
            r#"{"command":"unused"}"#,
            "process.shell",
            "dangerous",
        ),
    )
    .expect("test package should be written");
    let store = test_store(&temporary_directory);
    let permissive_core = RunnerCore::default();
    permissive_core
        .import_package(&store, &package_path)
        .expect("package should import before policy is restricted");
    permissive_core
        .approve_installed(&store, "shell-command")
        .expect("package should be approved before policy is restricted");

    let restricted_core = core_with_policy(false, true, true);
    let error = restricted_core
        .run_installed(&store, "shell-command")
        .expect_err("shell command must be blocked independently");
    assert!(
        error
            .to_string()
            .contains("security.policy.allow_shell_commands"),
        "{error}"
    );
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

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();

    let imported = core
        .import_package(&store, &package_path)
        .expect("package should import");
    assert_eq!(imported.id, "network-trigger");
    assert_eq!(imported.package_file_name, "network-trigger.bbs");
    assert!(imported.package_path.exists());
    assert!(store.verify_script_package_hash("network-trigger").is_ok());
    let initial_auth = core
        .list_trigger_auth(&store, "network-trigger")
        .expect("webhook auth should list");
    assert!(initial_auth.is_empty());

    let blocked = core
        .run_installed(&store, "network-trigger")
        .expect_err("unapproved high-risk package should be blocked");
    assert!(matches!(blocked, CoreError::ApprovalRequired(_)));
    assert_eq!(
        store
            .list_run_records(Some("network-trigger"), None)
            .expect("failed run record should list")
            .first()
            .expect("failed run record should exist")
            .status,
        "failed"
    );

    let approval = core
        .approve_installed(&store, "network-trigger")
        .expect("package should approve");
    assert_eq!(approval.generated_trigger_tokens.len(), 1);
    assert_eq!(
        approval.generated_trigger_tokens[0].status.node_id,
        "n-webhook"
    );
    let approved_auth = core
        .list_trigger_auth(&store, "network-trigger")
        .expect("approved webhook auth should list");
    assert_eq!(approved_auth.len(), 1);
    assert!(approved_auth[0].auth_enabled);
    let rotated_auth = core
        .rotate_trigger_token(
            &store,
            "network-trigger",
            "n-webhook",
            NetworkTriggerType::Webhook,
        )
        .expect("webhook token should rotate");
    assert!(rotated_auth.token.starts_with("bbwh_"));
    let report = core
        .dispatch_trigger_event(
            &store,
            TriggerEvent {
                action_type: "trigger.webhook".to_owned(),
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
    let updated_auth = core
        .list_trigger_auth(&store, "network-trigger")
        .expect("updated webhook auth should list");
    assert_eq!(updated_auth.len(), 1);
    assert_eq!(
        updated_auth[0].token_preview,
        rotated_auth.status.token_preview
    );
    assert!(
        store
            .find_script_approval("network-trigger")
            .expect("approval lookup should succeed")
            .is_none()
    );

    let reapproval = core
        .approve_installed(&store, "network-trigger")
        .expect("updated package should approve");
    assert!(reapproval.generated_trigger_tokens.is_empty());

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
fn observation_permissions_follow_resolution_approval_reload_and_update_lifecycle() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let initial_path = temporary_directory.path().join("observation-initial.bbs");
    let updated_path = temporary_directory.path().join("observation-updated.bbs");
    let variable_path = temporary_directory.path().join("observation-variable.bbs");
    let process_path = temporary_directory.path().join("observation-process.bbs");
    let absolute_watch_path = temporary_directory.path().join("host-watch");
    fs::create_dir_all(&absolute_watch_path).expect("absolute watch directory should exist");

    fs::write(
        &initial_path,
        create_observation_trigger_package(
            "observation-watch",
            "trigger.file_watch",
            "file_watch",
            json!({"path": "incoming", "recursive": false}),
            &["file.watch.limited"],
            "medium",
            json!([]),
        ),
    )
    .expect("limited observation package should be written");
    fs::write(
        &updated_path,
        create_observation_trigger_package(
            "observation-watch",
            "trigger.file_watch",
            "file_watch",
            json!({"path": absolute_watch_path, "recursive": false}),
            &["file.watch.any"],
            "dangerous",
            json!([]),
        ),
    )
    .expect("host observation package should be written");
    fs::write(
        &variable_path,
        create_observation_trigger_package(
            "observation-variable",
            "trigger.file_watch",
            "file_watch",
            json!({"path": "{{watchPath}}", "recursive": false}),
            &["file.watch.any", "variable.local.set"],
            "dangerous",
            json!([{
                "name": "watchPath",
                "scope": "runtime",
                "type": "file_path",
                "value": absolute_watch_path
            }]),
        ),
    )
    .expect("variable observation package should be written");
    fs::write(
        &process_path,
        create_observation_trigger_package(
            "observation-process",
            "trigger.process_started",
            "process_started",
            json!({"matchMode": "process_name", "target": "baudbound-test-process"}),
            &["process.observe"],
            "medium",
            json!([]),
        ),
    )
    .expect("process observation package should be written");

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &initial_path)
        .expect("limited observation package should import");
    assert!(
        core.list_trigger_registrations(&store, None)
            .expect("unapproved registrations should list")
            .is_empty(),
        "unapproved observation triggers must not register"
    );

    let approval = core
        .approve_installed(&store, "observation-watch")
        .expect("limited observation package should approve");
    assert_eq!(
        approval.approval.approved_permissions,
        ["file.watch.limited"]
    );
    let limited = core
        .list_trigger_registrations(&store, None)
        .expect("approved limited registration should list");
    let limited = limited
        .iter()
        .find(|registration| registration.action_type == "trigger.file_watch")
        .expect("limited file-watch registration should exist");
    let limited_path = Path::new(
        limited.config["path"]
            .as_str()
            .expect("limited watch path should resolve to text"),
    );
    assert!(limited_path.is_absolute());
    let expected_workspace = store
        .root()
        .join("workspaces/observation-watch")
        .canonicalize()
        .expect("script workspace should resolve");
    assert!(limited_path.starts_with(expected_workspace));

    core.revoke_approval(&store, "observation-watch")
        .expect("approval should revoke")
        .expect("approval should exist");
    assert!(
        core.list_trigger_registrations(&store, None)
            .expect("registrations after revocation should list")
            .is_empty(),
        "revocation must remove observation triggers on the next registration reload"
    );
    core.approve_installed(&store, "observation-watch")
        .expect("limited observation package should reapprove");

    core.update_package(&store, &updated_path)
        .expect("observation package should update");
    assert!(
        core.list_trigger_registrations(&store, None)
            .expect("registrations after update should list")
            .is_empty(),
        "a package update must invalidate the previous observation approval"
    );
    let updated_approval = core
        .approve_installed(&store, "observation-watch")
        .expect("updated observation package should approve");
    assert_eq!(
        updated_approval.approval.approved_permissions,
        ["file.watch.any"]
    );
    let updated = core
        .list_trigger_registrations(&store, None)
        .expect("updated observation registration should list");
    let updated = updated
        .iter()
        .find(|registration| registration.action_type == "trigger.file_watch")
        .expect("updated file-watch registration should exist");
    assert_eq!(
        Path::new(updated.config["path"].as_str().unwrap()),
        absolute_watch_path
    );

    core.import_package(&store, &variable_path)
        .expect("variable observation package should import");
    core.approve_installed(&store, "observation-variable")
        .expect("variable observation package should approve");
    let variable_registration = core
        .list_trigger_registrations(&store, Some("observation-variable"))
        .expect("variable observation registration should list");
    let variable_registration = variable_registration
        .iter()
        .find(|registration| registration.action_type == "trigger.file_watch")
        .expect("variable file-watch registration should exist");
    assert_eq!(
        Path::new(variable_registration.config["path"].as_str().unwrap()),
        absolute_watch_path,
        "pre-trigger variables must resolve before file-watch service validation"
    );

    core.import_package(&store, &process_path)
        .expect("process observation package should import");
    let process_approval = core
        .approve_installed(&store, "observation-process")
        .expect("process observation package should approve");
    assert_eq!(
        process_approval.approval.approved_permissions,
        ["process.observe"]
    );
    let process_registration = core
        .list_trigger_registrations(&store, Some("observation-process"))
        .expect("process observation registration should list");
    let process_registration = process_registration
        .iter()
        .find(|registration| registration.action_type == "trigger.process_started")
        .expect("process observation registration should exist");
    assert_eq!(process_registration.config["matchMode"], "process_name");
    assert_eq!(
        process_registration.config["target"],
        "baudbound-test-process"
    );
}

#[test]
fn run_observer_is_notified_after_terminal_records_are_committed() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("action-handler-test.bbs");
    fs::write(&package_path, create_action_handler_test_package())
        .expect("test package should be written");

    let store = test_store(&temporary_directory);
    let observer = Arc::new(RecordingRunObserver {
        observed_record_counts: Mutex::new(Vec::new()),
        store: store.clone(),
    });
    let core = RunnerCore::default().with_run_observer(observer.clone());
    core.import_package(&store, &package_path)
        .expect("package should import");

    core.run_installed(&store, "action-handler-test")
        .expect_err("unapproved package should create a failed record");
    core.approve_installed(&store, "action-handler-test")
        .expect("package should approve");
    core.run_installed(&store, "action-handler-test")
        .expect("approved package should complete");

    assert_eq!(
        observer
            .observed_record_counts
            .lock()
            .expect("observer count lock should not be poisoned")
            .as_slice(),
        &[1, 2]
    );
}

#[test]
fn custom_action_handler_is_used_for_script_execution() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("action-handler-test.bbs");
    fs::write(&package_path, create_action_handler_test_package())
        .expect("test package should be written");

    let store = test_store(&temporary_directory);
    let handler = Arc::new(RecordingActionHandler::default());
    let core = RunnerCore::default().with_action_handler(handler.clone());
    core.import_package(&store, &package_path)
        .expect("package should import");
    core.approve_installed(&store, "action-handler-test")
        .expect("package should approve");
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
fn import_rejects_tampered_capability_declarations() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("tampered-capabilities.bbs");
    fs::write(
        &package_path,
        create_action_handler_test_package_with_capabilities(Some(&["trigger.manual"])),
    )
    .expect("test package should be written");

    let store = test_store(&temporary_directory);
    let error = RunnerCore::default()
        .import_package(&store, &package_path)
        .expect_err("package hiding its action capability must fail import");

    assert!(
        error
            .to_string()
            .contains("missing declared capability action.text")
    );
    assert!(
        store
            .list_scripts()
            .expect("installed scripts should list")
            .is_empty()
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
            "Linux Headless",
            "action.notification",
        ),
    )
    .expect("test package should be written");

    let store = test_store(&temporary_directory);
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

    let store = test_store(&temporary_directory);
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

    let store = test_store(&temporary_directory);
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

    let store = test_store(&temporary_directory);
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
    let store = test_store(&temporary_directory);
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
    let store = test_store(&temporary_directory);
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
    let store = test_store(&temporary_directory);
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
        create_target_runtime_test_package(
            "desktop-text",
            test_desktop_runtime(),
            "action.text.format",
        ),
    )
    .expect("test package should be written");
    let config = RunnerConfig {
        runner: RunnerSettings {
            target_runtimes: vec![test_headless_runtime().to_owned()],
            trigger_reload_seconds: DEFAULT_TRIGGER_RELOAD_SECONDS,
            ..RunnerSettings::default()
        },
        ..RunnerConfig::default()
    };

    let store = test_store(&temporary_directory);
    let error = RunnerCore::from_config(&config)
        .import_package(&store, &package_path)
        .expect_err("headless-only runner should reject desktop package target");

    assert!(
        error.to_string().contains(&format!(
            "this runner is active as {}",
            test_headless_runtime()
        )),
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

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &child_package_path)
        .expect("child package should import");
    core.import_package(&store, &parent_package_path)
        .expect("parent package should import");
    core.approve_installed(&store, "parent-script")
        .expect("sub-script parent should approve");
    core.approve_installed(&store, "action-handler-test")
        .expect("child package should approve");

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
fn sub_script_action_routes_recursive_cycle_to_failed_output() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("recursive-script.bbs");
    fs::write(
        &package_path,
        create_sub_script_parent_package("recursive-script", "recursive-script"),
    )
    .expect("recursive test package should be written");

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("recursive package should import");
    core.approve_installed(&store, "recursive-script")
        .expect("recursive package should approve");

    let report = core
        .run_installed(&store, "recursive-script")
        .expect("recursive sub-script failure should remain available to the parent graph");

    assert!(
        report
            .variables
            .get("n-sub.error")
            .and_then(Value::as_object)
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .is_some_and(|message| message.contains("sub-script cycle detected"))
    );
    assert!(
        report.logs.iter().any(|log| {
            log.level == "error" && log.message.contains("sub-script cycle detected")
        })
    );
    let runs = store
        .list_run_records(Some("recursive-script"), None)
        .expect("completed run record with errors should list");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "completed");
    assert!(runs[0].logs.iter().any(|log| log.level == "error"));
}

#[test]
fn lists_trigger_registrations_for_installed_scripts() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");

    let store = test_store(&temporary_directory);
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

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("package should import");
    core.approve_installed(&store, "network-trigger")
        .expect("package should approve");

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
fn disabled_scripts_cannot_execute_from_stale_trigger_events() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("package should import");
    core.approve_installed(&store, "network-trigger")
        .expect("package should approve");
    core.set_installed_enabled(&store, "network-trigger", false)
        .expect("script should disable");

    let error = core
        .dispatch_trigger_event(
            &store,
            TriggerEvent {
                action_type: "trigger.webhook".to_owned(),
                node_id: "n-webhook".to_owned(),
                payload: json!({"body": "stale event"}),
                script_id: "network-trigger".to_owned(),
            },
        )
        .expect_err("a stale trigger event must not execute a disabled script");

    assert!(matches!(error, CoreError::ScriptDisabled(_)));
}

#[test]
fn unapproved_scripts_are_not_registered_or_executed() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("package should import");

    assert!(
        core.list_trigger_registrations(&store, None)
            .expect("active trigger registrations should list")
            .is_empty()
    );

    let error = core
        .dispatch_trigger_event(
            &store,
            TriggerEvent {
                action_type: "trigger.webhook".to_owned(),
                node_id: "n-webhook".to_owned(),
                payload: json!({"body": "unapproved event"}),
                script_id: "network-trigger".to_owned(),
            },
        )
        .expect_err("an unapproved script must not execute");

    assert!(matches!(error, CoreError::ApprovalRequired(_)));
}

#[test]
fn unapproved_low_risk_scripts_cannot_execute() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("action-handler-test.bbs");
    fs::write(&package_path, create_action_handler_test_package())
        .expect("low-risk test package should be written");

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("low-risk package should import");

    let error = core
        .run_installed(&store, "action-handler-test")
        .expect_err("an unapproved low-risk script must not execute");

    assert!(matches!(error, CoreError::ApprovalRequired(_)));
}

#[test]
fn status_reports_script_health_and_approval_state() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("package should import");

    let status = core.status(&store).expect("status should build");
    assert!(
        status
            .supported_target_runtimes
            .contains(&test_headless_runtime().to_owned())
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
    assert_eq!(status.scripts[0].declared_permissions, ["network.webhook"]);
    let metadata = status.scripts[0]
        .metadata
        .as_ref()
        .expect("verified package metadata should be available");
    assert_eq!(metadata.created_with, "BaudBound Test");
    assert_eq!(metadata.minimum_runner_version, "0.1.0");

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

    let installed_package_path = status.scripts[0].installed.package_path.clone();
    fs::write(&installed_package_path, b"tampered package")
        .expect("installed package should be changed for the status check");
    let disabled_problem = core.status(&store).expect("disabled status should build");
    assert!(disabled_problem.scripts[0].has_problem());
    assert_eq!(disabled_problem.problem_count, 0);
    fs::copy(&package_path, &installed_package_path)
        .expect("installed package should be restored after the status check");

    let revoked = core
        .revoke_approval(&store, "network-trigger")
        .expect("approval should revoke")
        .expect("stored approval should be returned");
    assert_eq!(revoked.script_id, "network-trigger");
    let status = core
        .status(&store)
        .expect("status should build after revoke");
    assert!(matches!(
        status.scripts[0].approval_status,
        ApprovalStatus::Missing
    ));
    core.set_installed_enabled(&store, "network-trigger", true)
        .expect("script should re-enable");
    let blocked = core
        .run_installed(&store, "network-trigger")
        .expect_err("revoked script should be blocked");
    assert!(matches!(blocked, CoreError::ApprovalRequired(_)));
}

#[test]
fn dispatches_trigger_event_through_core_dispatcher() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let package_path = temporary_directory.path().join("network-trigger.bbs");
    fs::write(&package_path, create_policy_test_package()).expect("test package should be written");

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("package should import");
    core.approve_installed(&store, "network-trigger")
        .expect("package should approve");

    let report = core
        .trigger_dispatcher(&store)
        .dispatch(TriggerEvent {
            action_type: "trigger.webhook".to_owned(),
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

#[test]
fn saving_script_settings_validates_the_entire_batch_before_writing() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let package_path = temporary_directory.path().join("settings.bbs");
    fs::write(&package_path, create_script_settings_test_package())
        .expect("settings package should be written");
    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &package_path)
        .expect("settings package should import");
    core.set_installed_script_setting_from_text(&store, "settings-test", "Endpoint", "saved")
        .expect("initial setting should store");

    let invalid = BTreeMap::from([
        ("Endpoint".to_owned(), "changed".to_owned()),
        ("Retries".to_owned(), "not-a-number".to_owned()),
    ]);
    core.save_installed_script_settings_from_text(&store, "settings-test", &invalid)
        .expect_err("an invalid setting must reject the entire batch");

    let unchanged = core
        .list_installed_script_settings(&store, "settings-test")
        .expect("settings should remain readable");
    assert_eq!(
        unchanged
            .iter()
            .find(|setting| setting.name == "Endpoint")
            .and_then(|setting| setting.configured_value.as_ref()),
        Some(&json!("saved"))
    );
    assert!(
        unchanged
            .iter()
            .find(|setting| setting.name == "Retries")
            .is_some_and(|setting| !setting.configured)
    );

    let replacement = BTreeMap::from([("Retries".to_owned(), "3".to_owned())]);
    let statuses = core
        .save_installed_script_settings_from_text(&store, "settings-test", &replacement)
        .expect("valid settings should replace the configured set");
    assert!(
        statuses
            .iter()
            .find(|setting| setting.name == "Endpoint")
            .is_some_and(|setting| !setting.configured)
    );
    assert_eq!(
        statuses
            .iter()
            .find(|setting| setting.name == "Retries")
            .and_then(|setting| setting.configured_value.as_ref()),
        Some(&json!(3.0))
    );
}

fn create_policy_test_package() -> Vec<u8> {
    create_policy_test_package_with_webhook("network-trigger", "hook")
}

fn create_observation_trigger_package(
    script_id: &str,
    action_type: &str,
    trigger_type: &str,
    config: Value,
    permissions: &[&str],
    risk: &str,
    variables: Value,
) -> Vec<u8> {
    let has_variables = variables
        .as_array()
        .is_some_and(|variables| !variables.is_empty());
    let manifest = json!({
        "format_version": 1,
        "script_language_version": 1,
        "id": script_id,
        "name": script_id,
        "created_with": "BaudBound Test",
        "created_at": "2026-01-01T00:00:00.000Z",
        "minimum_runner_version": "0.1.0",
        "version": "1.0.0",
        "variables": variables
    })
    .to_string();
    let program = complete_test_program_contract(
        &json!({
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
                    },
                    {
                        "id": "n-observation",
                        "action_type": action_type,
                        "type": trigger_type,
                        "config": config,
                        "runtime_outputs": []
                    }
                ],
                "program": {"type": "block", "steps": [], "edges": []}
            }
        })
        .to_string(),
    );
    let mut capabilities: Value =
        serde_json::from_str(&capabilities_json(&program, test_headless_runtime()))
            .expect("observation capabilities should parse");
    if has_variables {
        capabilities["required_capabilities"]
            .as_array_mut()
            .expect("required capabilities should be an array")
            .push(json!("runtime.variables"));
    }
    let capabilities = capabilities.to_string();
    let permissions = json!({
        "declared_permissions": permissions,
        "risk_level": risk
    })
    .to_string();
    create_test_package([
        ("manifest.json", manifest.as_str()),
        ("program.json", program.as_str()),
        ("permissions.json", permissions.as_str()),
        ("capabilities.json", capabilities.as_str()),
    ])
}

fn create_script_settings_test_package() -> Vec<u8> {
    let manifest = r#"{
        "format_version": 1,
        "script_language_version": 1,
        "id": "settings-test",
        "name": "Settings Test",
        "created_with": "BaudBound Test",
        "created_at": "2026-01-01T00:00:00.000Z",
        "minimum_runner_version": "0.1.0",
        "version": "1.0.0",
        "settings": [
            {
                "name": "Endpoint",
                "type": "string",
                "description": "Endpoint setting",
                "required": false,
                "default_value": "package"
            },
            {
                "name": "Retries",
                "type": "number",
                "description": "Retry count",
                "required": false
            }
        ]
    }"#;
    let program = r#"{
        "entry": {
            "trigger": {
                "id": "n-manual",
                "action_type": "trigger.manual",
                "type": "manual",
                "config": {},
                "runtime_outputs": []
            },
            "triggers": [],
            "program": {
                "type": "block",
                "steps": [],
                "edges": []
            }
        }
    }"#;
    let capabilities = capabilities_json(program, test_headless_runtime());
    create_test_package([
        ("manifest.json", manifest),
        ("program.json", program),
        (
            "permissions.json",
            r#"{"declared_permissions":[],"risk_level":"low"}"#,
        ),
        ("capabilities.json", capabilities.as_str()),
    ])
}

fn core_with_policy(
    allow_shell_commands: bool,
    allow_dangerous_permissions: bool,
    allow_public_network_listeners: bool,
) -> RunnerCore {
    let mut config = RunnerConfig::default();
    config.security.policy = SecurityPolicySettings {
        allow_dangerous_permissions,
        allow_private_http_requests: false,
        allow_public_network_listeners,
        allow_shell_commands,
    };
    RunnerCore::from_config(&config)
}

fn create_action_policy_test_package(
    script_id: &str,
    action_type: &str,
    action: &str,
    action_config: &str,
    permission: &str,
    risk: &str,
) -> Vec<u8> {
    let manifest = format!(
        r#"{{
            "format_version": 1,
            "script_language_version": 1,
            "id": "{script_id}",
            "name": "{script_id}",
            "created_with": "BaudBound Test",
            "created_at": "2026-01-01T00:00:00.000Z",
            "minimum_runner_version": "0.1.0",
            "version": "1.0.0"
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
                "triggers": [{{
                    "id": "n-manual",
                    "action_type": "trigger.manual",
                    "type": "manual",
                    "config": {{}},
                    "runtime_outputs": []
                }}],
                "program": {{
                    "type": "block",
                    "steps": [{{
                        "id": "n-action",
                        "action_type": "{action_type}",
                        "type": "action",
                        "action": "{action}",
                        "config": {action_config},
                        "runtime_outputs": []
                    }}],
                    "edges": []
                }}
            }}
        }}"#
    );
    let capabilities = capabilities_json(&program, test_headless_runtime());
    let permissions =
        format!(r#"{{"declared_permissions":["{permission}"],"risk_level":"{risk}"}}"#);
    create_test_package([
        ("manifest.json", manifest.as_str()),
        ("program.json", program.as_str()),
        ("permissions.json", permissions.as_str()),
        ("capabilities.json", capabilities.as_str()),
    ])
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
                    "minimum_runner_version": "0.1.0",
                    "version": "1.0.0"
                }}"#
    );
    let program = complete_test_program_contract(&format!(
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
    ));
    let capabilities = capabilities_json(&program, test_headless_runtime());

    for (path, content) in [
        ("manifest.json", manifest.as_str()),
        ("program.json", program.as_str()),
        (
            "permissions.json",
            r#"{"declared_permissions": ["network.webhook"], "risk_level": "high"}"#,
        ),
        ("capabilities.json", capabilities.as_str()),
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
                "minimum_runner_version": "0.1.0",
                "version": "1.0.0"
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
                                "execution_order": 0,
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
    let capabilities = capabilities_json(&program, test_headless_runtime());

    create_test_package([
        ("manifest.json", manifest.as_str()),
        ("program.json", program.as_str()),
        (
            "permissions.json",
            r#"{"declared_permissions": ["script.run"], "risk_level": "high"}"#,
        ),
        ("capabilities.json", capabilities.as_str()),
    ])
}

fn create_action_handler_test_package() -> Vec<u8> {
    create_action_handler_test_package_with_capabilities(None)
}

fn create_cancellable_test_package() -> Vec<u8> {
    let program = r#"{
        "entry": {
            "trigger": {
                "id": "n-manual",
                "action_type": "trigger.manual",
                "type": "manual",
                "config": {},
                "runtime_outputs": []
            },
            "triggers": [],
            "program": {
                "type": "block",
                "steps": [{
                    "id": "n-delay",
                    "action_type": "action.delay",
                    "type": "action",
                    "action": "delay",
                    "config": {"amount": 30, "unit": "seconds"},
                    "runtime_outputs": []
                }],
                "edges": [{
                    "execution_order": 0,
                    "source": "n-manual",
                    "source_handle": "out",
                    "target": "n-delay",
                    "target_handle": "input"
                }]
            }
        }
    }"#;
    let capabilities = capabilities_json(program, test_headless_runtime());
    create_test_package([
        (
            "manifest.json",
            r#"{
                "format_version": 1,
                "script_language_version": 1,
                "id": "cancellable",
                "name": "cancellable",
                "created_with": "BaudBound Test",
                "created_at": "2026-01-01T00:00:00.000Z",
                "minimum_runner_version": "0.1.0",
                "version": "1.0.0"
            }"#,
        ),
        ("program.json", program),
        (
            "permissions.json",
            r#"{"declared_permissions": ["delay"], "risk_level": "low"}"#,
        ),
        ("capabilities.json", capabilities.as_str()),
    ])
}

fn create_action_handler_test_package_with_capabilities(
    capability_override: Option<&[&str]>,
) -> Vec<u8> {
    let program = r#"{
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
                                        "input": "hello",
                                        "operations": [
                                            {
                                                "id": "uppercase",
                                                "operation": "uppercase"
                                            }
                                        ]
                                    },
                                    "runtime_outputs": []
                                }
                            ],
                            "edges": [
                                {
                                    "execution_order": 0,
                                    "source": "n-manual",
                                    "source_handle": "out",
                                    "target": "n-format",
                                    "target_handle": "input"
                                }
                            ]
                        }
                    }
                }"#;
    let capabilities = capability_override.map_or_else(
        || capabilities_json(program, test_headless_runtime()),
        |capabilities| {
            serde_json::json!({
                "required_capabilities": capabilities,
                "target_runtimes": [test_headless_runtime()]
            })
            .to_string()
        },
    );
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
                    "minimum_runner_version": "0.1.0",
                    "version": "1.0.0"
                }"#,
        ),
        ("program.json", program),
        (
            "permissions.json",
            r#"{"declared_permissions": ["text.transform"], "risk_level": "low"}"#,
        ),
        ("capabilities.json", capabilities.as_str()),
    ])
}

fn create_target_runtime_test_package(
    script_id: &str,
    target_runtime: &str,
    action_type: &str,
) -> Vec<u8> {
    let (action, config) = match action_type {
        "action.notification" => ("show_notification", r#"{"title":"Test","message":"Test"}"#),
        "action.pixel.get" => ("get_pixel_color", r#"{"x":0,"y":0}"#),
        "action.text.format" => (
            "format_text",
            r#"{"input":"test","operations":[{"id":"uppercase","operation":"uppercase"}]}"#,
        ),
        unsupported => panic!("missing schema-complete target-runtime fixture for {unsupported}"),
    };
    create_target_runtime_test_package_with_action_config(
        script_id,
        target_runtime,
        action_type,
        action,
        config,
    )
}

fn create_target_runtime_test_package_with_action_config(
    script_id: &str,
    target_runtime: &str,
    action_type: &str,
    action: &str,
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
                "minimum_runner_version": "0.1.0",
                "version": "1.0.0"
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
                                "action": "{action}",
                                "config": {action_config},
                                "runtime_outputs": []
                            }}
                        ],
                        "edges": []
                    }}
                }}
            }}"#
    );
    let capabilities = capabilities_json(&program, target_runtime);

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
                "minimum_runner_version": "{minimum_version}",
                "version": "1.0.0"
            }}"#
    );

    let program = r#"{
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
                }"#;
    let capabilities = capabilities_json(program, test_headless_runtime());

    create_test_package([
        ("manifest.json", manifest.as_str()),
        ("program.json", program),
        (
            "permissions.json",
            r#"{"declared_permissions": [], "risk_level": "low"}"#,
        ),
        ("capabilities.json", capabilities.as_str()),
    ])
}

fn create_test_package<const N: usize>(files: [(&str, &str); N]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for (path, content) in files {
        let normalized_program;
        let content = if path == "program.json" {
            normalized_program = complete_test_program_contract(content);
            normalized_program.as_str()
        } else {
            content
        };
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

fn complete_test_program_contract(program: &str) -> String {
    let mut program = serde_json::from_str::<Value>(program).expect("test program should be JSON");
    let block = program
        .get_mut("entry")
        .and_then(|entry| entry.get_mut("program"))
        .and_then(Value::as_object_mut)
        .expect("test program should contain entry.program");
    block
        .entry("execution_model")
        .or_insert_with(|| json!("directed_graph"));
    block.entry("runtime_context").or_insert_with(|| {
        json!({
            "expression_reference": "{{node-id.data_name}}",
            "template_reference": "{{node-id.data_name}}",
            "variables": [],
            "built_in_variables": {
                "syntax": "{{variable_name}}",
                "variables": []
            },
            "node_outputs": []
        })
    });
    serde_json::to_string(&program).expect("completed test program should serialize")
}

fn capabilities_json(program: &str, target_runtime: &str) -> String {
    let program = serde_json::from_str(program).expect("test program should be valid JSON");
    let report = baudbound_security::calculate_program_capabilities(&program)
        .expect("test program capabilities should calculate");
    let required_capabilities = report
        .required_capabilities
        .into_iter()
        .map(|capability| capability.name)
        .collect::<Vec<_>>();
    serde_json::json!({
        "required_capabilities": required_capabilities,
        "target_runtimes": [target_runtime]
    })
    .to_string()
}
