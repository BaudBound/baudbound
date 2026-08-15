use super::*;

#[test]
fn missing_sub_script_routes_failure_and_persists_parent_errors() {
    let temporary_directory = tempfile::tempdir().expect("temporary storage should be created");
    let parent_package_path = temporary_directory.path().join("missing-child-parent.bbs");
    fs::write(
        &parent_package_path,
        create_sub_script_parent_package(MISSING_CHILD_PARENT_ID, MISSING_CHILD_ID),
    )
    .expect("parent test package should be written");

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &parent_package_path)
        .expect("parent package should import");
    core.approve_installed(&store, MISSING_CHILD_PARENT_ID)
        .expect("parent package should approve");

    let report = core
        .run_installed(&store, MISSING_CHILD_PARENT_ID)
        .expect("missing child failure should remain available to the parent graph");

    let message = report
        .variables
        .get("n-sub.error")
        .and_then(Value::as_object)
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .expect("sub-script failure should expose a structured message");
    assert!(message.contains(MISSING_CHILD_ID), "{message}");
    assert!(message.contains("is not installed"), "{message}");
    let parent_runs = store
        .list_run_records(Some(MISSING_CHILD_PARENT_ID), None)
        .expect("parent run records should list");
    assert_eq!(parent_runs.len(), 1);
    assert_eq!(parent_runs[0].status, "completed");
    assert!(parent_runs[0].logs.iter().any(|log| log.level == "error"));
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
        create_sub_script_parent_package(APPROVAL_PARENT_ID, NETWORK_TRIGGER_ID),
    )
    .expect("parent package should be written");

    let store = test_store(&temporary_directory);
    let core = RunnerCore::default();
    core.import_package(&store, &child_package_path)
        .expect("child package should import");
    core.import_package(&store, &parent_package_path)
        .expect("parent package should import");
    core.approve_installed(&store, APPROVAL_PARENT_ID)
        .expect("parent package should approve");

    let parent_failure_report = core
        .run_installed(&store, APPROVAL_PARENT_ID)
        .expect("unapproved child failure should remain available to the parent graph");
    let error_message = parent_failure_report
        .variables
        .get("n-sub.error")
        .and_then(Value::as_object)
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .expect("sub-script failure should expose a structured message");
    assert!(
        error_message.contains("is not approved for its current package"),
        "{error_message}"
    );

    let failed_child_runs = store
        .list_run_records(Some(NETWORK_TRIGGER_ID), None)
        .expect("child run records should list");
    assert_eq!(failed_child_runs.len(), 1);
    assert_eq!(failed_child_runs[0].status, "failed");

    core.approve_installed(&store, NETWORK_TRIGGER_ID)
        .expect("child package should approve independently");
    let parent_report = core
        .run_installed(&store, APPROVAL_PARENT_ID)
        .expect("approved child should run through its parent");
    let child_run_id = parent_report
        .variables
        .get("n-sub.run_id")
        .and_then(Value::as_str)
        .expect("parent output should expose the child run id");

    let child_runs = store
        .list_run_records(Some(NETWORK_TRIGGER_ID), None)
        .expect("child run records should list");
    assert_eq!(child_runs.len(), 2);
    assert!(
        child_runs
            .iter()
            .any(|run| run.status == "completed" && run.run_id == child_run_id),
        "parent output must link to the persisted child run"
    );
}
