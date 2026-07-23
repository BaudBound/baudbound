use std::{
    fs,
    io::{Cursor, Write},
    path::Path,
    process::{Command, Output},
};

#[cfg(windows)]
use std::process::Stdio;

use serde_json::Value;
#[cfg(windows)]
use serde_json::json;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

#[test]
fn desktop_cli_initializes_runner_config_template() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let config_path = runner_home.join("config.toml");

    let printed_path = run_desktop(&runner_home, ["config", "path"]);
    assert_success_like(&printed_path);
    assert_eq!(
        String::from_utf8_lossy(&printed_path.stdout).trim(),
        config_path.display().to_string()
    );

    let printed_template = run_desktop(&runner_home, ["config", "print"]);
    assert_success_like(&printed_template);
    assert!(String::from_utf8_lossy(&printed_template.stdout).contains("[websockets]"));

    assert_success(run_desktop(&runner_home, ["config", "init"]));
    let config = fs::read_to_string(&config_path).expect("config template should be written");
    assert!(config.contains("webhooks_enabled = false"));

    let refused_overwrite = run_desktop(&runner_home, ["config", "init"]);
    assert!(
        !refused_overwrite.status.success(),
        "second init should refuse overwrite\n{}",
        command_output(&refused_overwrite)
    );

    assert_success(run_desktop(&runner_home, ["config", "init", "--force"]));
}

#[test]
fn desktop_startup_creates_runner_config_automatically() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let config_path = runner_home.join("config.toml");

    assert_success(run_desktop(&runner_home, ["status"]));

    let config = fs::read_to_string(config_path).expect("config should be created on startup");
    assert!(config.contains("[websockets]"));
}

#[test]
fn desktop_cli_reads_and_updates_shared_config() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");

    assert_success(run_desktop(
        &runner_home,
        ["config", "set", "display.time-format", "12-hour"],
    ));
    let config = fs::read_to_string(runner_home.join("config.toml"))
        .expect("runner config should be readable");
    assert!(config.contains("time_format = \"12-hour\""));
}

#[test]
fn desktop_cli_manages_script_storage_against_isolated_home() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let package_path = temporary_directory.path().join("desktop-script.bbs");
    fs::write(&package_path, create_desktop_test_package("Desktop Script"))
        .expect("test package should be written");

    let status = command_json(run_desktop(&runner_home, ["status", "--json"]));
    assert_eq!(status["desktop"]["action_adapter"], "system");
    assert_eq!(status["desktop"]["native_tray"], false);
    assert!(status["desktop"]["storage_root"].is_string());
    assert!(
        status["desktop"]["supported_target_runtimes"]
            .as_array()
            .expect("supported target runtimes should be an array")
            .iter()
            .any(|target_runtime| target_runtime.as_str() == Some("Generic Desktop"))
    );
    assert!(
        status["runner"]["supported_target_runtimes"]
            .as_array()
            .expect("runner supported target runtimes should be an array")
            .iter()
            .any(|target_runtime| target_runtime.as_str() == Some("Generic Desktop"))
    );

    let doctor = command_json(run_desktop(&runner_home, ["doctor", "--json"]));
    assert!(doctor["healthy"].is_boolean());
    assert!(doctor["os"].is_string());
    assert!(
        doctor["checks"]
            .as_array()
            .expect("doctor checks should be a JSON array")
            .iter()
            .all(|check| check["available"].is_boolean()
                && check["label"].is_string()
                && check["note"].is_string())
    );

    assert_success(run_desktop(
        &runner_home,
        [
            "script",
            "import",
            package_path.to_str().expect("path should be UTF-8"),
        ],
    ));

    let scripts = command_json(run_desktop(&runner_home, ["script", "list", "--json"]));
    let scripts = scripts
        .as_array()
        .expect("installed scripts should be a JSON array");
    assert_eq!(scripts.len(), 1);
    assert_eq!(scripts[0]["id"], "desktop-script");
    assert_eq!(scripts[0]["enabled"], true);

    assert_success(run_desktop(
        &runner_home,
        ["script", "disable", "desktop-script"],
    ));
    let disabled = command_json(run_desktop(
        &runner_home,
        ["script", "inspect", "desktop-script", "--json"],
    ));
    assert_eq!(disabled["enabled"], false);

    assert_success(run_desktop(
        &runner_home,
        ["script", "enable", "desktop-script"],
    ));
    assert_success(run_desktop(
        &runner_home,
        ["script", "approve", "desktop-script"],
    ));
    let run_output = run_desktop(&runner_home, ["script", "run", "desktop-script"]);
    assert_success(run_output);
    let run_logs = command_json(run_desktop(
        &runner_home,
        ["script", "logs", "--script", "desktop-script", "--json"],
    ));
    let run_logs = run_logs
        .as_array()
        .expect("run logs should be a JSON array");
    assert_eq!(run_logs.len(), 1);
    assert_eq!(run_logs[0]["script_id"], "desktop-script");
    assert_eq!(run_logs[0]["trigger_node_id"], "n-manual");

    let approval = command_json(run_desktop(
        &runner_home,
        ["script", "approval", "desktop-script", "--json"],
    ));
    assert_eq!(approval["script_id"], "desktop-script");
    assert!(
        approval["approved_permissions"]
            .as_array()
            .expect("approved permissions should be an array")
            .is_empty()
    );

    assert_success(run_desktop(
        &runner_home,
        ["script", "revoke-approval", "desktop-script"],
    ));
    assert_eq!(
        command_json(run_desktop(
            &runner_home,
            ["script", "approval", "desktop-script", "--json"],
        )),
        Value::Null
    );

    assert_success(run_desktop(
        &runner_home,
        ["script", "remove", "desktop-script"],
    ));
    assert!(
        command_json(run_desktop(&runner_home, ["script", "list", "--json"]))
            .as_array()
            .expect("installed scripts should be a JSON array")
            .is_empty()
    );
}

#[test]
fn desktop_cli_supports_package_and_script_commands() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let package_path = temporary_directory.path().join("desktop-script.bbs");
    fs::write(&package_path, create_desktop_test_package("Desktop Script"))
        .expect("test package should be written");
    let package = package_path.to_str().expect("path should be UTF-8");

    assert_success(run_desktop(&runner_home, ["validate", package]));

    let package_inspection =
        command_json(run_desktop(&runner_home, ["inspect", package, "--json"]));
    assert_eq!(
        package_inspection["summary"]["script_name"],
        "Desktop Script"
    );

    assert_success(run_desktop(&runner_home, ["script", "import", package]));

    let scripts = command_json(run_desktop(&runner_home, ["script", "list", "--json"]));
    assert_eq!(
        scripts
            .as_array()
            .expect("installed scripts should be a JSON array")
            .len(),
        1
    );

    let installed = command_json(run_desktop(
        &runner_home,
        ["script", "inspect", "desktop-script", "--json"],
    ));
    assert_eq!(installed["id"], "desktop-script");
    assert_success(run_desktop(
        &runner_home,
        ["script", "approve", "desktop-script"],
    ));

    let triggers = command_json(run_desktop(&runner_home, ["script", "triggers", "--json"]));
    assert!(
        triggers
            .as_array()
            .expect("trigger registrations should be a JSON array")
            .iter()
            .any(|trigger| trigger["node_id"] == "n-manual")
    );

    assert_success(run_desktop(
        &runner_home,
        ["script", "dispatch-trigger", "desktop-script", "n-manual"],
    ));
    let run_logs = command_json(run_desktop(
        &runner_home,
        ["script", "logs", "--script", "desktop-script", "--json"],
    ));
    assert_eq!(
        run_logs
            .as_array()
            .expect("run logs should be a JSON array")
            .len(),
        1
    );

    let approval = command_json(run_desktop(
        &runner_home,
        ["script", "approval", "desktop-script", "--json"],
    ));
    assert_eq!(approval["script_id"], "desktop-script");

    assert_success(run_desktop(
        &runner_home,
        ["script", "revoke-approval", "desktop-script"],
    ));
    assert_eq!(
        command_json(run_desktop(
            &runner_home,
            ["script", "approval", "desktop-script", "--json"],
        )),
        Value::Null
    );

    assert_success(run_desktop(
        &runner_home,
        ["script", "disable", "desktop-script"],
    ));
    assert_eq!(
        command_json(run_desktop(
            &runner_home,
            ["script", "inspect", "desktop-script", "--json"],
        ))["enabled"],
        false
    );

    assert_success(run_desktop(
        &runner_home,
        ["script", "enable", "desktop-script"],
    ));
    assert_success(run_desktop(
        &runner_home,
        ["script", "remove", "desktop-script"],
    ));
}

#[test]
fn desktop_cli_supports_script_group_aliases() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let package_path = temporary_directory.path().join("desktop-script.bbs");
    fs::write(&package_path, create_desktop_test_package("Desktop Script"))
        .expect("test package should be written");

    assert_success(run_desktop(
        &runner_home,
        [
            "script",
            "import",
            package_path.to_str().expect("path should be UTF-8"),
        ],
    ));
    let scripts = command_json(run_desktop(&runner_home, ["script", "list", "--json"]));
    assert_eq!(
        scripts
            .as_array()
            .expect("installed scripts should be a JSON array")
            .len(),
        1
    );
    let status = command_json(run_desktop(&runner_home, ["script", "status", "--json"]));
    assert_eq!(status["total_script_count"], 1);
    assert_eq!(status["enabled_script_count"], 1);

    assert_success(run_desktop(
        &runner_home,
        ["script", "approve", "desktop-script"],
    ));
    assert_success(run_desktop(
        &runner_home,
        ["script", "run", "desktop-script"],
    ));
    let triggers = command_json(run_desktop(
        &runner_home,
        ["script", "triggers", "desktop-script", "--json"],
    ));
    assert!(
        triggers
            .as_array()
            .expect("trigger registrations should be a JSON array")
            .iter()
            .any(|trigger| trigger["node_id"] == "n-manual")
    );
    assert_success(run_desktop(
        &runner_home,
        ["script", "dispatch-trigger", "desktop-script", "n-manual"],
    ));
    let logs = command_json(run_desktop(
        &runner_home,
        ["script", "logs", "--script", "desktop-script", "--json"],
    ));
    assert_eq!(
        logs.as_array().expect("logs should be a JSON array").len(),
        2
    );

    assert_success(run_desktop(
        &runner_home,
        ["script", "remove", "desktop-script"],
    ));
}

#[cfg(windows)]
#[test]
fn desktop_cli_dispatches_hotkey_triggers() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let package_path = temporary_directory.path().join("hotkey-script.bbs");
    fs::write(
        &package_path,
        create_desktop_hotkey_test_package("Hotkey Script"),
    )
    .expect("hotkey test package should be written");

    assert_success(run_desktop(
        &runner_home,
        [
            "script",
            "import",
            package_path.to_str().expect("path should be UTF-8"),
        ],
    ));
    assert_success(run_desktop(
        &runner_home,
        ["script", "approve", "hotkey-script"],
    ));

    let hotkeys = command_json(run_desktop(&runner_home, ["hotkey", "list", "--json"]));
    assert_eq!(hotkeys["count"], 1);
    assert_eq!(hotkeys["hotkeys"], json!(["Ctrl+Alt+B"]));

    let reports = command_json(run_desktop(
        &runner_home,
        ["hotkey", "dispatch", "control-alt-b", "--json"],
    ));
    let reports = reports
        .as_array()
        .expect("hotkey dispatch should return report array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["identity"]["script_id"], "hotkey-script");
    assert_eq!(reports[0]["identity"]["trigger_node_id"], "n-hotkey");
    assert_eq!(reports[0]["variables"]["n-hotkey.key"], "Ctrl+Alt+B");

    let run_logs = command_json(run_desktop(
        &runner_home,
        ["script", "logs", "--script", "hotkey-script", "--json"],
    ));
    let run_logs = run_logs
        .as_array()
        .expect("run logs should be a JSON array");
    assert_eq!(run_logs.len(), 1);
    assert_eq!(run_logs[0]["trigger_node_id"], "n-hotkey");
}

#[cfg(windows)]
#[test]
fn desktop_cli_listens_for_stdin_hotkey_triggers() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let package_path = temporary_directory.path().join("hotkey-script.bbs");
    fs::write(
        &package_path,
        create_desktop_hotkey_test_package("Hotkey Script"),
    )
    .expect("hotkey test package should be written");

    assert_success(run_desktop(
        &runner_home,
        [
            "script",
            "import",
            package_path.to_str().expect("path should be UTF-8"),
        ],
    ));
    assert_success(run_desktop(
        &runner_home,
        ["script", "approve", "hotkey-script"],
    ));

    let output = run_desktop_with_stdin(
        &runner_home,
        ["hotkey", "listen", "--stdin", "--json"],
        "control-alt-b\nCtrl+Alt+C\n\n",
    );
    assert_success_like(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let events = stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("listener line should be JSON"))
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["key"], "control-alt-b");
    assert_eq!(events[0]["matched"], 1);
    assert_eq!(events[1]["key"], "Ctrl+Alt+C");
    assert_eq!(events[1]["matched"], 0);

    let run_logs = command_json(run_desktop(
        &runner_home,
        ["script", "logs", "--script", "hotkey-script", "--json"],
    ));
    let run_logs = run_logs
        .as_array()
        .expect("run logs should be a JSON array");
    assert_eq!(run_logs.len(), 1);
    assert_eq!(run_logs[0]["trigger_node_id"], "n-hotkey");
}

#[cfg(windows)]
#[test]
fn desktop_cli_serves_hotkey_stdin_once() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let package_path = temporary_directory.path().join("hotkey-script.bbs");
    fs::write(
        &package_path,
        create_desktop_hotkey_test_package("Hotkey Script"),
    )
    .expect("hotkey test package should be written");

    assert_success(run_desktop(
        &runner_home,
        [
            "script",
            "import",
            package_path.to_str().expect("path should be UTF-8"),
        ],
    ));
    assert_success(run_desktop(
        &runner_home,
        ["script", "approve", "hotkey-script"],
    ));

    let preflight = command_json(run_desktop(
        &runner_home,
        ["serve", "--dry-run", "--json", "--hotkey-stdin"],
    ));
    assert_eq!(preflight["active_service_count"], 2);
    assert!(
        preflight["services"]
            .as_array()
            .expect("serve services should be an array")
            .iter()
            .any(|service| service["name"] == "hotkey"
                && service["active"] == true
                && service["registrations"] == 1)
    );
    assert!(
        preflight["services"]
            .as_array()
            .expect("serve services should be an array")
            .iter()
            .any(|service| service["name"] == "hotkey_stdin"
                && service["active"] == true
                && service["registrations"] == 1)
    );

    let output = run_desktop_with_stdin(
        &runner_home,
        ["serve", "--hotkey-stdin", "--once"],
        "control-alt-b\n",
    );
    assert_success_like(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Serving 1 desktop hotkey trigger"),
        "serve stdout should describe active hotkey listener\n{}",
        command_output(&output)
    );

    let status = command_json(run_desktop(&runner_home, ["status", "--json"]));
    assert_eq!(status["service"]["state"], "stopped");
    assert_eq!(status["service"]["active_service_count"], 0);
    assert!(
        status["service"]["services"]
            .as_array()
            .expect("stopped service rows should be an array")
            .iter()
            .all(|service| service["active"] == false && service["details"] == json!({}))
    );

    let run_logs = command_json(run_desktop(
        &runner_home,
        ["script", "logs", "--script", "hotkey-script", "--json"],
    ));
    let run_logs = run_logs
        .as_array()
        .expect("run logs should be a JSON array");
    assert_eq!(run_logs.len(), 1);
    assert_eq!(run_logs[0]["trigger_node_id"], "n-hotkey");
}

fn run_desktop<const N: usize>(runner_home: &Path, args: [&str; N]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_baudbound"))
        .args(args)
        .env("BAUDBOUND_HOME", runner_home)
        .env_remove("BAUDBOUND_CONFIG")
        .output()
        .expect("baudbound command should run")
}

#[cfg(windows)]
fn run_desktop_with_stdin<const N: usize>(
    runner_home: &Path,
    args: [&str; N],
    stdin: &str,
) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_baudbound"))
        .args(args)
        .env("BAUDBOUND_HOME", runner_home)
        .env_remove("BAUDBOUND_CONFIG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("baudbound command should spawn");

    child
        .stdin
        .take()
        .expect("child stdin should be piped")
        .write_all(stdin.as_bytes())
        .expect("stdin should be written");

    child
        .wait_with_output()
        .expect("baudbound command should finish")
}

fn assert_success(output: Output) {
    assert_success_like(&output);
}

fn assert_success_like(output: &Output) {
    assert!(
        output.status.success(),
        "command should succeed\n{}",
        command_output(output)
    );
}

fn command_json(output: Output) -> Value {
    assert!(
        output.status.success(),
        "command should succeed\n{}",
        command_output(&output)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "command stdout should be valid JSON: {error}\n{}",
            command_output(&output)
        )
    })
}

fn command_output(output: &Output) -> String {
    format!(
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn create_desktop_test_package(script_name: &str) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    let manifest = format!(
        r#"{{
            "format_version": 1,
            "script_language_version": 1,
            "id": "desktop-script",
            "name": "{script_name}",
            "created_with": "BaudBound Desktop Test",
            "created_at": "2026-01-01T00:00:00.000Z",
            "minimum_runner_version": "0.1.0"
        }}"#
    );

    for (path, content) in [
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
                    "program": {
                        "type": "block",
                        "execution_model": "directed_graph",
                        "runtime_context": {
                            "expression_reference": "{{node-id.data_name}}",
                            "template_reference": "{{node-id.data_name}}",
                            "variables": [],
                            "built_in_variables": {"syntax": "{{variable_name}}", "variables": []},
                            "node_outputs": []
                        },
                        "steps": [],
                        "edges": []
                    }
                }
            }"#,
        ),
        (
            "permissions.json",
            r#"{"declared_permissions": [], "risk_level": "low"}"#,
        ),
        (
            "capabilities.json",
            r#"{"required_capabilities": ["trigger.manual"], "target_runtime": "Generic Desktop"}"#,
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

#[cfg(windows)]
fn create_desktop_hotkey_test_package(script_name: &str) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    let manifest = format!(
        r#"{{
            "format_version": 1,
            "script_language_version": 1,
            "id": "hotkey-script",
            "name": "{script_name}",
            "created_with": "BaudBound Desktop Test",
            "created_at": "2026-01-01T00:00:00.000Z",
            "minimum_runner_version": "0.1.0"
        }}"#
    );

    for (path, content) in [
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
                        },
                        {
                            "id": "n-hotkey",
                            "action_type": "trigger.hotkey",
                            "type": "hotkey",
                            "config": {"key": "Ctrl+Alt+B"},
                            "runtime_outputs": []
                        }
                    ],
                    "program": {
                        "type": "block",
                        "execution_model": "directed_graph",
                        "runtime_context": {
                            "expression_reference": "{{node-id.data_name}}",
                            "template_reference": "{{node-id.data_name}}",
                            "variables": [],
                            "built_in_variables": {"syntax": "{{variable_name}}", "variables": []},
                            "node_outputs": []
                        },
                        "steps": [],
                        "edges": []
                    }
                }
            }"#,
        ),
        (
            "permissions.json",
            r#"{"declared_permissions": [], "risk_level": "low"}"#,
        ),
        (
            "capabilities.json",
            r#"{"required_capabilities": ["trigger.hotkey", "trigger.manual"], "target_runtime": "Windows Desktop"}"#,
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
