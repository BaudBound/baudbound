//! Guards the on-disk permissions of everything the storage layer creates.
//!
//! The database holds the runner control token in cleartext along with run
//! records and script settings. Restricting only the main database file leaves
//! the WAL and shared-memory sidecars readable, and those carry the same
//! recently written pages.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;

use super::*;

fn mode_of(path: &std::path::Path) -> u32 {
    std::fs::metadata(path)
        .unwrap_or_else(|error| panic!("{} should be readable: {error}", path.display()))
        .permissions()
        .mode()
        & 0o777
}

#[test]
fn storage_files_are_not_readable_by_other_users() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let store = open_store(&temporary_directory);

    // Force a write so SQLite materializes its write-ahead log sidecars.
    store
        .write_service_status(&serde_json::json!({
            "control": {"token": "sensitive-control-token"}
        }))
        .expect("service status should write");

    let database = temporary_directory
        .path()
        .join("runner")
        .join("runner.sqlite3");
    assert!(database.is_file(), "database should exist after opening");

    let mut offenders = Vec::new();
    for suffix in ["", "-wal", "-shm"] {
        let path = if suffix.is_empty() {
            database.clone()
        } else {
            let mut name = database.clone().into_os_string();
            name.push(suffix);
            std::path::PathBuf::from(name)
        };
        if !path.exists() {
            continue;
        }
        let mode = mode_of(&path);
        if mode & 0o077 != 0 {
            offenders.push(format!("{} has mode {mode:o}", path.display()));
        }
    }

    assert!(
        offenders.is_empty(),
        "storage files must not grant group or other access: {}",
        offenders.join("; ")
    );
}

#[test]
fn runner_home_directory_is_not_traversable_by_other_users() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let _store = open_store(&temporary_directory);

    let runner_home = temporary_directory.path().join("runner");
    let mode = mode_of(&runner_home);
    assert_eq!(
        mode & 0o077,
        0,
        "the runner home must not grant group or other access, found mode {mode:o}"
    );
}
