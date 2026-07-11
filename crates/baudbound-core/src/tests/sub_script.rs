use super::*;

#[test]
fn missing_sub_script_fails_parent_and_persists_the_failure() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let parent_package_path = temporary_directory.path().join("missing-child-parent.bbs");
    fs::write(
        &parent_package_path,
        create_sub_script_parent_package("missing-child-parent", "missing-child"),
    )
    .expect("parent test package should be written");

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &parent_package_path)
        .expect("parent package should import");
    core.approve_installed(&store, "missing-child-parent")
        .expect("parent package should approve");

    let error = core
        .run_installed(&store, "missing-child-parent")
        .expect_err("missing child script must fail the parent run");

    assert!(error.to_string().contains("missing-child"), "{error}");
    assert!(error.to_string().contains("is not installed"), "{error}");
    let parent_runs = store
        .list_run_records(Some("missing-child-parent"), None)
        .expect("parent run records should list");
    assert_eq!(parent_runs.len(), 1);
    assert_eq!(parent_runs[0].status, "failed");
}

#[test]
fn parent_approval_cannot_bypass_child_script_approval() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let child_package_path = temporary_directory.path().join("network-trigger.bbs");
    let parent_package_path = temporary_directory.path().join("approval-parent.bbs");
    fs::write(&child_package_path, create_policy_test_package())
        .expect("child policy package should be written");
    fs::write(
        &parent_package_path,
        create_sub_script_parent_package("approval-parent", "network-trigger"),
    )
    .expect("parent package should be written");

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &child_package_path)
        .expect("child package should import");
    core.import_package(&store, &parent_package_path)
        .expect("parent package should import");
    core.approve_installed(&store, "approval-parent")
        .expect("parent package should approve");

    let error = core
        .run_installed(&store, "approval-parent")
        .expect_err("unapproved child permissions must block the parent run");
    assert!(
        error
            .to_string()
            .contains("runner policy blocks permission webhook_public_bind"),
        "{error}"
    );

    let failed_child_runs = store
        .list_run_records(Some("network-trigger"), None)
        .expect("child run records should list");
    assert_eq!(failed_child_runs.len(), 1);
    assert_eq!(failed_child_runs[0].status, "failed");

    core.approve_installed(&store, "network-trigger")
        .expect("child package should approve independently");
    let parent_report = core
        .run_installed(&store, "approval-parent")
        .expect("approved child should run through its parent");
    let child_run_id = parent_report
        .variables
        .get("n-sub.run_id")
        .and_then(Value::as_str)
        .expect("parent output should expose the child run id");

    let child_runs = store
        .list_run_records(Some("network-trigger"), None)
        .expect("child run records should list");
    assert_eq!(child_runs.len(), 2);
    assert!(
        child_runs
            .iter()
            .any(|run| run.status == "completed" && run.run_id == child_run_id),
        "parent output must link to the persisted child run"
    );
}
