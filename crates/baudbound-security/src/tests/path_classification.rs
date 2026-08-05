//! Guards path risk classification against forms that escape the workspace.
//!
//! Classification decides whether a file action declares a bounded permission
//! such as `file.write.limited` or the Dangerous `file.write.any`. A path that
//! is not provably confined to the script workspace must classify as unbounded
//! so the user sees a Dangerous prompt.
//!
//! Classification must not be platform gated. A package authored on Linux can be
//! installed on Windows, so a Windows-specific escape form has to classify the
//! same way on both platforms or the risk shown at approval time depends on
//! which machine happened to build the package.

use serde::Deserialize;

use super::*;

#[derive(Deserialize)]
struct PathConformance {
    cases: Vec<PathCase>,
    version: u32,
}

#[derive(Deserialize)]
struct PathCase {
    path: String,
    reason: String,
    unbounded: bool,
}

/// The editor and the runner must classify every shared case identically.
///
/// The editor shows the risk at approval time and the runner recalculates it
/// before running. A disagreement makes a package fail to import, or worse,
/// shows the user a lower risk than the one that is enforced.
#[test]
fn shared_path_classification_fixtures_conform() {
    let conformance: PathConformance = serde_json::from_str(include_str!(
        "../../../../contracts/path-classification-conformance.json"
    ))
    .expect("shared path classification fixtures should parse");
    assert_eq!(conformance.version, 1);

    for case in conformance.cases {
        assert_eq!(
            is_unbounded_path(&case.path),
            case.unbounded,
            "{:?} should be {}: {}",
            case.path,
            if case.unbounded {
                "unbounded"
            } else {
                "bounded"
            },
            case.reason
        );
    }
}

#[test]
fn bounded_relative_paths_classify_as_limited() {
    for path in [
        "output/result.txt",
        "notes.txt",
        "nested/directory/file.log",
        "./relative.txt",
    ] {
        assert!(
            !is_unbounded_path(path),
            "{path} is confined to the workspace and should classify as bounded"
        );
    }
}

#[test]
fn traversal_and_absolute_paths_classify_as_unbounded() {
    for path in [
        "../escape.txt",
        "nested/../../escape.txt",
        "/etc/passwd",
        "~/.ssh/id_rsa",
        "C:/Windows/System32/drivers/etc/hosts",
        "\\Windows\\System32\\config\\SAM",
        "{{n-node.output}}",
    ] {
        assert!(
            is_unbounded_path(path),
            "{path} escapes the workspace and must classify as unbounded"
        );
    }
}

#[test]
fn windows_drive_relative_paths_classify_as_unbounded() {
    // `C:file.txt` has a drive prefix but no root, so it resolves against the
    // current directory of that drive rather than the script workspace.
    for path in ["C:file.txt", "c:nested/file.txt", "Z:data.log"] {
        assert!(
            is_unbounded_path(path),
            "{path} is drive relative and does not resolve inside the workspace"
        );
    }
}

#[test]
fn windows_alternate_data_streams_classify_as_unbounded() {
    // An alternate data stream writes to a hidden stream of another file rather
    // than to the named path, so it cannot be shown to the user as a bounded
    // workspace write.
    for path in ["notes.txt:hidden", "output/report.txt:$DATA"] {
        assert!(
            is_unbounded_path(path),
            "{path} targets an alternate data stream and must classify as unbounded"
        );
    }
}

#[test]
fn windows_reserved_device_names_classify_as_unbounded() {
    // These names resolve to devices regardless of the directory they appear in,
    // so they never stay inside the workspace.
    for path in ["CON", "NUL", "COM1", "LPT1", "output/CON", "aux.txt"] {
        assert!(
            is_unbounded_path(path),
            "{path} resolves to a reserved device and must classify as unbounded"
        );
    }
}
