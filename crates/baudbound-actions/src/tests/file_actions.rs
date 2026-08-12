use std::fs;

use serde_json::json;

use super::{
    TestHttpServer, execute, execute_with_handler, execute_with_workspace, private_network_handler,
};
use crate::{ActionLimits, HeadlessActionHandler};

/// Unit-level check that a bounded path stays inside the workspace it is given.
///
/// This covers the resolver only. Which workspace the execution path actually
/// supplies is covered by an integration test in `baudbound-core`, because that
/// is the wiring this test cannot observe.
#[test]
fn limited_relative_paths_use_the_script_workspace() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let workspace = directory.path().join("workspaces").join("script-1");

    execute_with_workspace(
        "action.file.write",
        json!({"path": "output/result.txt", "content": "workspace"}),
        workspace.clone(),
    )
    .expect("limited write should succeed");

    assert_eq!(
        fs::read_to_string(workspace.join("output").join("result.txt"))
            .expect("workspace output should exist"),
        "workspace"
    );
}

#[cfg(unix)]
#[test]
fn limited_paths_reject_symbolic_link_workspace_escapes() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let workspace = directory.path().join("workspaces").join("script-1");
    let outside = directory.path().join("outside");
    fs::create_dir_all(&workspace).expect("workspace should be created");
    fs::create_dir_all(&outside).expect("outside directory should be created");
    symlink(&outside, workspace.join("escape")).expect("symlink should be created");

    let error = execute_with_workspace(
        "action.file.write",
        json!({"path": "escape/result.txt", "content": "blocked"}),
        workspace,
    )
    .expect_err("workspace symlink escape must fail");

    assert!(
        matches!(
            &error,
            baudbound_runtime::RuntimeActionError::Failed {
                action_type,
                message,
            } if action_type == "action.file.write"
                && message.contains("failed to create parent directory")
        ),
        "unexpected symlink rejection: {error}"
    );
    assert!(!outside.join("result.txt").exists());
}

/// The workspace root itself must not be a symbolic link.
///
/// `cap-std` confines everything below the directory handle, but the handle is
/// obtained with ambient authority. A symlink at the root would relocate the
/// entire sandbox before `cap-std` is involved, so the previous test cannot
/// catch it.
#[cfg(unix)]
#[test]
fn limited_paths_reject_a_symbolic_link_workspace_root() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let workspace = directory.path().join("workspaces").join("script-1");
    let outside = directory.path().join("attacker-controlled");
    fs::create_dir_all(workspace.parent().expect("workspace has a parent"))
        .expect("workspace parent should be created");
    fs::create_dir_all(&outside).expect("outside directory should be created");
    symlink(&outside, &workspace).expect("workspace root symlink should be created");

    let error = execute_with_workspace(
        "action.file.write",
        json!({"path": "result.txt", "content": "blocked"}),
        workspace,
    )
    .expect_err("a symlinked workspace root must fail");

    assert!(
        format!("{error}").contains("symbolic link"),
        "unexpected workspace root rejection: {error}"
    );
    assert!(
        !outside.join("result.txt").exists(),
        "a symlinked workspace root must not redirect writes outside the runner home"
    );
}

/// A bounded relative path with no workspace must fail rather than fall back to
/// ambient authority, which would resolve it against the process directory.
#[test]
fn limited_paths_without_a_workspace_fail_closed() {
    let error = execute(
        "action.file.write",
        json!({"path": "result.txt", "content": "blocked"}),
    )
    .expect_err("a bounded path without a workspace must fail");

    assert!(
        format!("{error}").contains("no script workspace is available"),
        "unexpected missing-workspace rejection: {error}"
    );
}

#[test]
fn read_file_rejects_invalid_encoding_and_invalid_utf8() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let path = directory.path().join("invalid.bin");
    fs::write(&path, [0xff, 0xfe]).expect("fixture should be written");

    let encoding_error = execute(
        "action.file.read",
        json!({"path": path, "encoding": "latin-1"}),
    )
    .expect_err("unsupported encoding must fail");
    assert!(
        encoding_error
            .to_string()
            .contains("unsupported file encoding")
    );

    let utf8_error = execute(
        "action.file.read",
        json!({"path": path, "encoding": "utf-8"}),
    )
    .expect_err("invalid UTF-8 must fail");
    assert!(utf8_error.to_string().contains("not valid UTF-8"));
}

#[test]
fn read_file_rejects_files_over_the_configured_limit() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let path = directory.path().join("large.txt");
    fs::write(&path, "12345").expect("fixture should be written");
    let handler = HeadlessActionHandler::default().with_limits(ActionLimits {
        max_file_read_bytes: baudbound_runtime::ResourceLimit::limited(4),
        ..ActionLimits::default()
    });

    let error = execute_with_handler(
        &handler,
        "action.file.read",
        json!({"path": path, "encoding": "utf-8"}),
        serde_json::Value::Null,
    )
    .expect_err("oversized file read must fail");

    assert!(
        error
            .to_string()
            .contains("configured read limit of 4 bytes")
    );
}

#[test]
fn write_file_rejects_invalid_modes_and_directory_targets() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let path = directory.path().join("output.txt");

    let mode_error = execute(
        "action.file.write",
        json!({"path": path, "mode": "replace", "content": "data"}),
    )
    .expect_err("unsupported write mode must fail");
    assert!(
        mode_error
            .to_string()
            .contains("unsupported file write mode")
    );

    let directory_error = execute(
        "action.file.write",
        json!({"path": directory.path(), "mode": "overwrite", "content": "data"}),
    )
    .expect_err("writing to a directory must fail");
    assert!(directory_error.to_string().contains("failed to open"));
}

#[test]
fn repeated_file_writes_respect_the_exact_cumulative_run_budget() {
    const WRITES: usize = 1_000;

    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let path = directory.path().join("repeated.txt");
    let handler = HeadlessActionHandler::default().with_limits(ActionLimits {
        max_file_write_bytes_per_run: baudbound_runtime::ResourceLimit::limited(WRITES as u64),
        ..ActionLimits::default()
    });

    for _ in 0..WRITES {
        execute_with_handler(
            &handler,
            "action.file.write",
            json!({"path": path, "mode": "append", "content": "x"}),
            serde_json::Value::Null,
        )
        .expect("a write within the cumulative run budget should succeed");
    }
    let error = execute_with_handler(
        &handler,
        "action.file.write",
        json!({"path": path, "mode": "append", "content": "x"}),
        serde_json::Value::Null,
    )
    .expect_err("the first byte beyond the cumulative run budget must fail");

    assert!(error.to_string().contains("1000 byte file-write limit"));
    assert_eq!(
        fs::metadata(path).expect("output should exist").len(),
        WRITES as u64
    );
}

#[test]
fn copy_file_overwrites_only_when_requested() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = directory.path().join("source.txt");
    let destination = directory.path().join("destination.txt");
    fs::write(&source, "new content").expect("source should be written");
    fs::write(&destination, "old content").expect("destination should be written");

    let blocked = execute(
        "action.file.copy",
        json!({
            "sourcePath": source,
            "destinationPath": destination,
            "overwrite": false
        }),
    )
    .expect_err("copy without overwrite must fail");
    assert!(blocked.to_string().contains("overwrite is disabled"));
    assert_eq!(fs::read_to_string(&destination).unwrap(), "old content");

    execute(
        "action.file.copy",
        json!({
            "sourcePath": source,
            "destinationPath": destination,
            "overwrite": true
        }),
    )
    .expect("copy with overwrite should succeed");
    assert_eq!(fs::read_to_string(&destination).unwrap(), "new content");
}

#[test]
fn copy_and_move_reject_the_same_source_and_destination_without_data_loss() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = directory.path().join("source.txt");
    fs::write(&source, "preserve me").expect("source should be written");
    let equivalent_path = directory.path().join(".").join("source.txt");

    for action_type in ["action.file.copy", "action.file.move"] {
        let error = execute(
            action_type,
            json!({
                "sourcePath": source,
                "destinationPath": equivalent_path,
                "overwrite": true
            }),
        )
        .expect_err("same-file transfer must fail");
        assert!(error.to_string().contains("same file"));
        assert_eq!(fs::read_to_string(&source).unwrap(), "preserve me");
    }
}

#[test]
fn move_file_overwrites_only_when_requested() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = directory.path().join("source.txt");
    let destination = directory.path().join("destination.txt");
    fs::write(&source, "new content").expect("source should be written");
    fs::write(&destination, "old content").expect("destination should be written");

    let blocked = execute(
        "action.file.move",
        json!({
            "sourcePath": source,
            "destinationPath": destination,
            "overwrite": false
        }),
    )
    .expect_err("move without overwrite must fail");
    assert!(blocked.to_string().contains("overwrite is disabled"));
    assert!(source.exists());

    execute(
        "action.file.move",
        json!({
            "sourcePath": source,
            "destinationPath": destination,
            "overwrite": true
        }),
    )
    .expect("move with overwrite should succeed");
    assert!(!source.exists());
    assert_eq!(fs::read_to_string(&destination).unwrap(), "new content");
}

#[test]
fn file_transfers_reject_missing_sources_and_directory_destinations() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let missing = directory.path().join("missing.txt");
    let destination = directory.path().join("destination.txt");

    for action_type in ["action.file.copy", "action.file.move"] {
        let missing_error = execute(
            action_type,
            json!({
                "sourcePath": missing,
                "destinationPath": destination,
                "overwrite": false
            }),
        )
        .expect_err("missing source must fail");
        assert!(missing_error.to_string().contains("failed to"));

        let source = directory.path().join(format!("{action_type}.txt"));
        fs::write(&source, "content").expect("source should be written");
        let directory_error = execute(
            action_type,
            json!({
                "sourcePath": source,
                "destinationPath": directory.path(),
                "overwrite": true
            }),
        )
        .expect_err("directory destination must fail");
        assert!(directory_error.to_string().contains("not a regular file"));
    }
}

#[test]
fn delete_file_rejects_missing_paths_and_directories() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let missing_error = execute(
        "action.file.delete",
        json!({"path": directory.path().join("missing.txt")}),
    )
    .expect_err("missing delete target must fail");
    assert!(matches!(
        missing_error,
        baudbound_runtime::RuntimeActionError::ExpectedOutcome {
            action_type,
            output,
            ..
        } if action_type == "action.file.delete" && output == "not_found"
    ));

    let directory_error = execute("action.file.delete", json!({"path": directory.path()}))
        .expect_err("directory delete target must fail");
    assert!(directory_error.to_string().contains("not a regular file"));
}

#[test]
fn download_rejects_http_failures_and_respects_overwrite() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let destination = directory.path().join("download.txt");

    let failed_server = TestHttpServer::start(
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let status_error = execute_with_handler(
        &private_network_handler(),
        "action.file.download",
        json!({
            "url": failed_server.url("/missing"),
            "destinationPath": destination,
            "overwrite": false,
            "timeoutSeconds": 2
        }),
        serde_json::Value::Null,
    )
    .expect_err("non-success download status must fail");
    assert!(status_error.to_string().contains("returned 404"));
    failed_server.join();
    assert!(!destination.exists());

    fs::write(&destination, "existing").expect("destination should be written");
    let overwrite_error = execute(
        "action.file.download",
        json!({
            "url": "http://127.0.0.1:1/not-requested",
            "destinationPath": destination,
            "overwrite": false,
            "timeoutSeconds": 1
        }),
    )
    .expect_err("existing download target must be protected");
    assert!(
        overwrite_error
            .to_string()
            .contains("overwrite is disabled")
    );
    assert_eq!(fs::read_to_string(&destination).unwrap(), "existing");

    let success_server = TestHttpServer::start(
        "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nnew",
    );
    execute_with_handler(
        &private_network_handler(),
        "action.file.download",
        json!({
            "url": success_server.url("/file"),
            "destinationPath": destination,
            "overwrite": true,
            "timeoutSeconds": 2
        }),
        serde_json::Value::Null,
    )
    .expect("download overwrite should succeed");
    success_server.join();
    assert_eq!(fs::read_to_string(&destination).unwrap(), "new");
}

#[test]
fn download_blocks_private_destinations_by_default() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let destination = directory.path().join("download.txt");

    let error = execute(
        "action.file.download",
        json!({
            "url": "http://127.0.0.1:1/private",
            "destinationPath": destination,
            "timeoutSeconds": 1
        }),
    )
    .expect_err("private downloads must be blocked by default");

    assert!(
        error
            .to_string()
            .contains("allow_private_http_requests is false")
    );
    assert!(!destination.exists());
}

#[test]
fn oversized_download_preserves_destination_and_removes_temporary_file() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let destination = directory.path().join("download.txt");
    fs::write(&destination, "existing").expect("destination should be written");
    let server =
        TestHttpServer::start("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nresponse-too-large");
    let handler = private_network_handler().with_limits(ActionLimits {
        max_file_download_bytes: baudbound_runtime::ResourceLimit::limited(4),
        ..ActionLimits::default()
    });

    let error = execute_with_handler(
        &handler,
        "action.file.download",
        json!({
            "url": server.url("/large"),
            "destinationPath": destination,
            "overwrite": true,
            "timeoutSeconds": 2
        }),
        serde_json::Value::Null,
    )
    .expect_err("oversized download must fail");
    server.join();

    assert!(error.to_string().contains("configured limit of 4 bytes"));
    assert_eq!(fs::read_to_string(&destination).unwrap(), "existing");
    assert_eq!(
        fs::read_dir(directory.path()).unwrap().count(),
        1,
        "temporary download file must be removed"
    );
}
