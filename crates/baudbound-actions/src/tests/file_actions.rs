use std::fs;

use serde_json::json;

use super::{TestHttpServer, execute};

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
    assert!(missing_error.to_string().contains("failed to inspect"));

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
    let status_error = execute(
        "action.file.download",
        json!({
            "url": failed_server.url("/missing"),
            "destinationPath": destination,
            "overwrite": false,
            "timeoutSeconds": 2
        }),
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
    execute(
        "action.file.download",
        json!({
            "url": success_server.url("/file"),
            "destinationPath": destination,
            "overwrite": true,
            "timeoutSeconds": 2
        }),
    )
    .expect("download overwrite should succeed");
    success_server.join();
    assert_eq!(fs::read_to_string(&destination).unwrap(), "new");
}
