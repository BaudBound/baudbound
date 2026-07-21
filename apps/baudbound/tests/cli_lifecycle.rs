use std::{
    fs,
    io::{BufRead, BufReader, Cursor, Write},
    net::TcpStream,
    path::Path,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

#[test]
fn cli_initializes_runner_config_template() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let config_path = runner_home.join("config.toml");

    let printed_path = run_baudbound(&runner_home, ["config", "path"]);
    assert_success_like(&printed_path);
    assert_eq!(
        String::from_utf8_lossy(&printed_path.stdout).trim(),
        config_path.display().to_string()
    );

    let printed_template = run_baudbound(&runner_home, ["config", "print"]);
    assert_success_like(&printed_template);
    assert!(String::from_utf8_lossy(&printed_template.stdout).contains("[triggers]"));

    assert_success(run_baudbound(&runner_home, ["config", "init"]));
    let config = fs::read_to_string(&config_path).expect("config template should be written");
    assert!(config.contains("[serial.devices.main_controller]"));

    let refused_overwrite = run_baudbound(&runner_home, ["config", "init"]);
    assert!(
        !refused_overwrite.status.success(),
        "second init should refuse overwrite\n{}",
        command_output(&refused_overwrite)
    );

    assert_success(run_baudbound(&runner_home, ["config", "init", "--force"]));
}

#[test]
fn cli_startup_creates_runner_config_automatically() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let config_path = runner_home.join("config.toml");

    assert_success(run_baudbound(&runner_home, ["script", "status"]));

    let config = fs::read_to_string(config_path).expect("config should be created on startup");
    assert!(config.contains("[webhooks]"));
}

#[test]
fn cli_runs_installed_package_lifecycle_against_isolated_home() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let package_path = temporary_directory.path().join("cli-lifecycle.bbs");
    let updated_package_path = temporary_directory.path().join("cli-lifecycle-updated.bbs");

    fs::write(
        &package_path,
        create_test_package("CLI Lifecycle", "hook", "initial"),
    )
    .expect("test package should be written");
    fs::write(
        &updated_package_path,
        create_test_package("CLI Lifecycle Updated", "updated-hook", "updated"),
    )
    .expect("updated test package should be written");

    assert_success(run_baudbound(
        &runner_home,
        [
            "validate",
            package_path.to_str().expect("path should be UTF-8"),
        ],
    ));
    let import_output = run_baudbound(
        &runner_home,
        [
            "script",
            "import",
            package_path.to_str().expect("path should be UTF-8"),
        ],
    );
    assert_success_like(&import_output);
    assert!(
        !String::from_utf8_lossy(&import_output.stdout).contains("bbwh_"),
        "import must not create or print network trigger credentials\n{}",
        command_output(&import_output)
    );
    let imported = command_json(run_baudbound(
        &runner_home,
        ["script", "inspect", "cli-lifecycle", "--json"],
    ));
    let imported_hash = imported["package_hash"]
        .as_str()
        .expect("imported package hash should be a string")
        .to_owned();
    assert_eq!(imported["id"], "cli-lifecycle");

    let failed_run = run_baudbound(&runner_home, ["script", "run", "cli-lifecycle"]);
    assert!(
        !failed_run.status.success(),
        "unapproved package should fail before approval\n{}",
        command_output(&failed_run)
    );

    let approval_output = run_baudbound(&runner_home, ["script", "approve", "cli-lifecycle"]);
    assert_success_like(&approval_output);
    assert!(
        String::from_utf8_lossy(&approval_output.stdout).contains("bbwh_"),
        "approval should print a newly generated Webhook token once\n{}",
        command_output(&approval_output)
    );
    assert_success(run_baudbound(
        &runner_home,
        ["script", "run", "cli-lifecycle"],
    ));

    let logs = command_json(run_baudbound(
        &runner_home,
        ["script", "logs", "--script", "cli-lifecycle", "--json"],
    ));
    let records = logs.as_array().expect("logs should be a JSON array");
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["status"], "completed");
    assert_eq!(records[1]["status"], "failed");

    assert_success(run_baudbound(
        &runner_home,
        [
            "script",
            "update",
            updated_package_path.to_str().expect("path should be UTF-8"),
        ],
    ));
    let updated = command_json(run_baudbound(
        &runner_home,
        ["script", "inspect", "cli-lifecycle", "--json"],
    ));
    assert_eq!(updated["id"], "cli-lifecycle");
    assert_ne!(updated["package_hash"], imported_hash);
    assert_eq!(
        command_json(run_baudbound(&runner_home, ["script", "list", "--json"]))
            .as_array()
            .expect("installed scripts should be a JSON array")
            .len(),
        1
    );
    assert_eq!(
        command_json(run_baudbound(
            &runner_home,
            ["script", "approval", "cli-lifecycle", "--json"],
        )),
        Value::Null
    );

    let triggers = command_json(run_baudbound(
        &runner_home,
        ["script", "triggers", "cli-lifecycle", "--json"],
    ));
    let webhook = triggers
        .as_array()
        .expect("triggers should be a JSON array")
        .iter()
        .find(|trigger| trigger["node_id"] == "n-webhook")
        .expect("webhook trigger should be listed");
    assert_eq!(webhook["config"]["hookName"], "updated-hook");

    assert_success(run_baudbound(
        &runner_home,
        ["script", "remove", "cli-lifecycle"],
    ));
    let installed_scripts = command_json(run_baudbound(&runner_home, ["script", "list", "--json"]));
    assert!(
        installed_scripts
            .as_array()
            .expect("installed scripts should be a JSON array")
            .is_empty()
    );
}

#[test]
fn cli_supports_script_group_aliases() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let package_path = temporary_directory.path().join("cli-script-group.bbs");

    fs::write(
        &package_path,
        create_test_package("CLI Script Group", "group-hook", "group"),
    )
    .expect("test package should be written");

    assert_success(run_baudbound(
        &runner_home,
        [
            "script",
            "import",
            package_path.to_str().expect("path should be UTF-8"),
        ],
    ));
    let scripts = command_json(run_baudbound(&runner_home, ["script", "list", "--json"]));
    assert_eq!(
        scripts
            .as_array()
            .expect("installed scripts should be a JSON array")
            .len(),
        1
    );
    let status = command_json(run_baudbound(&runner_home, ["script", "status", "--json"]));
    assert_eq!(status["total_script_count"], 1);
    assert_eq!(status["enabled_script_count"], 1);

    assert_success(run_baudbound(
        &runner_home,
        ["script", "approve", "CLI Script Group"],
    ));
    assert_success(run_baudbound(
        &runner_home,
        ["script", "run", "CLI Script Group"],
    ));
    let triggers = command_json(run_baudbound(
        &runner_home,
        ["script", "triggers", "CLI Script Group", "--json"],
    ));
    assert!(
        triggers
            .as_array()
            .expect("triggers should be a JSON array")
            .iter()
            .any(|trigger| trigger["node_id"] == "n-webhook")
    );
    assert_success(run_baudbound(
        &runner_home,
        ["script", "dispatch-trigger", "CLI Script Group", "n-manual"],
    ));
    let logs = command_json(run_baudbound(
        &runner_home,
        ["script", "logs", "--script", "CLI Script Group", "--json"],
    ));
    assert_eq!(
        logs.as_array().expect("logs should be a JSON array").len(),
        2
    );

    assert_success(run_baudbound(
        &runner_home,
        ["script", "remove", "CLI Script Group"],
    ));
}

#[test]
fn cli_status_reports_service_health() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    assert_success(run_baudbound(&runner_home, ["status"]));
    let service_status = serde_json::json!({
        "active_service_count": 1,
        "last_heartbeat_unix": 1,
        "pid": 1234,
        "reload_interval_seconds": 2,
        "services": [],
        "state": "running"
    });
    let connection = rusqlite::Connection::open(runner_home.join("runner.sqlite3"))
        .expect("runner database should open");
    connection
        .execute(
            "INSERT INTO service_status (id, status_json, updated_at_unix) VALUES (1, ?1, 1)",
            [serde_json::to_string(&service_status).expect("service status should serialize")],
        )
        .expect("service status should be inserted");

    let status = command_json(run_baudbound(&runner_home, ["status", "--json"]));

    assert_eq!(status["service_health"]["health"], "stale");
    assert_eq!(status["service_health"]["stale"], true);
    assert_eq!(status["service_health"]["stale_after_seconds"], 15);
}

#[test]
fn cli_reports_serve_preflight_without_opening_services() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let package_path = temporary_directory.path().join("cli-lifecycle.bbs");
    fs::write(
        &package_path,
        create_test_package("CLI Lifecycle", "hook", "preflight"),
    )
    .expect("test package should be written");

    assert_success(run_baudbound(
        &runner_home,
        [
            "script",
            "import",
            package_path.to_str().expect("path should be UTF-8"),
        ],
    ));
    assert_success(run_baudbound(
        &runner_home,
        ["script", "approve", "cli-lifecycle"],
    ));

    let preflight = command_json(run_baudbound(
        &runner_home,
        [
            "serve",
            "--dry-run",
            "--json",
            "--webhooks",
            "--webhook-port",
            "8123",
            "--reload-interval-seconds",
            "2",
        ],
    ));
    assert_eq!(preflight["active_service_count"], 1);
    assert_eq!(preflight["idle"], false);
    assert_eq!(preflight["reload_interval_seconds"], 2);
    assert_eq!(preflight["trigger_registration_count"], 2);
    let webhook = service_row(&preflight, "webhook");
    assert_eq!(webhook["active"], true);
    assert_eq!(webhook["enabled"], true);
    assert_eq!(webhook["registrations"], 1);
    assert_eq!(webhook["target"], "127.0.0.1:8123");

    assert_success(run_baudbound(
        &runner_home,
        ["script", "disable", "cli-lifecycle"],
    ));
    let disabled_preflight = command_json(run_baudbound(
        &runner_home,
        ["serve", "--dry-run", "--json", "--webhooks"],
    ));
    assert_eq!(disabled_preflight["active_service_count"], 0);
    assert_eq!(disabled_preflight["idle"], true);
    assert_eq!(disabled_preflight["trigger_registration_count"], 0);
}

#[test]
fn cli_serve_once_dispatches_due_schedule_and_persists_status() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let package_path = temporary_directory.path().join("scheduled-log.bbs");
    fs::write(&package_path, create_schedule_package())
        .expect("schedule test package should be written");

    assert_success(run_baudbound(
        &runner_home,
        [
            "script",
            "import",
            package_path.to_str().expect("path should be UTF-8"),
        ],
    ));
    assert_success(run_baudbound(
        &runner_home,
        ["script", "approve", "scheduled-log"],
    ));

    let preflight = command_json(run_baudbound(
        &runner_home,
        ["serve", "--dry-run", "--json"],
    ));
    assert_eq!(preflight["active_service_count"], 1);
    assert_eq!(preflight["trigger_registration_count"], 1);
    let schedule = service_row(&preflight, "schedule");
    assert_eq!(schedule["active"], true);
    assert_eq!(schedule["registrations"], 1);

    assert_success(run_baudbound(
        &runner_home,
        [
            "serve",
            "--once",
            "--run-schedules-immediately",
            "--reload-interval-seconds",
            "1",
        ],
    ));

    let logs = command_json(run_baudbound(
        &runner_home,
        ["script", "logs", "--script", "scheduled-log", "--json"],
    ));
    let records = logs.as_array().expect("logs should be a JSON array");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["status"], "completed");
    assert_eq!(records[0]["trigger_node_id"], "n-schedule");
    assert_eq!(
        records[0]["variables"]["n-schedule.schedule"]["unit"],
        "seconds"
    );
    assert_eq!(records[0]["variables"]["n-schedule.schedule"]["every"], 30);

    let service_status =
        read_service_status(&runner_home).expect("serve should persist service status in SQLite");
    assert_eq!(service_status["state"], "stopped");
    assert_eq!(service_status["active_service_count"], 0);
    assert_eq!(service_status["idle"], true);
    assert_eq!(service_row(&service_status, "schedule")["registrations"], 1);
    assert_eq!(service_row(&service_status, "schedule")["active"], false);
    assert_eq!(
        service_row(&service_status, "schedule")["details"],
        json!({})
    );
}

#[test]
fn cli_serve_reloads_triggers_after_import_and_stops_through_ipc() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let package_path = temporary_directory.path().join("scheduled-log.bbs");
    fs::write(&package_path, create_schedule_package())
        .expect("schedule test package should be written");

    let serve = spawn_baudbound(&runner_home, ["serve", "--reload-interval-seconds", "1"]);

    let initial_status = wait_for_service_status(&runner_home, Duration::from_secs(8), |status| {
        status["state"] == "running"
    });
    assert_eq!(initial_status["active_service_count"], 0);
    assert_eq!(initial_status["idle"], true);
    let public_status = command_json(run_baudbound(&runner_home, ["status", "--json"]));
    assert_eq!(
        public_status["service"]["control"]["protocol"],
        "baudbound-control-v1"
    );
    assert!(
        public_status["service"]["control"].get("token").is_none(),
        "status output must not expose the IPC authentication token"
    );

    assert_success(run_baudbound(
        &runner_home,
        [
            "script",
            "import",
            package_path.to_str().expect("path should be UTF-8"),
        ],
    ));
    assert_success(run_baudbound(
        &runner_home,
        ["script", "approve", "scheduled-log"],
    ));

    let reloaded_status = wait_for_service_status(&runner_home, Duration::from_secs(8), |status| {
        status["state"] == "running" && status["active_service_count"] == 1
    });
    let schedule = service_row(&reloaded_status, "schedule");
    assert_eq!(schedule["active"], true);
    assert_eq!(schedule["registrations"], 1);

    assert_success(run_baudbound(
        &runner_home,
        ["script", "disable", "scheduled-log"],
    ));
    let disabled_status = wait_for_service_status(&runner_home, Duration::from_secs(8), |status| {
        status["state"] == "running" && status["active_service_count"] == 0
    });
    assert_eq!(disabled_status["idle"], true);

    assert_success(run_baudbound(
        &runner_home,
        ["script", "enable", "scheduled-log"],
    ));
    wait_for_service_status(&runner_home, Duration::from_secs(8), |status| {
        status["state"] == "running" && status["active_service_count"] == 1
    });

    assert_success(run_baudbound(
        &runner_home,
        ["script", "revoke-approval", "scheduled-log"],
    ));
    let revoked_status = wait_for_service_status(&runner_home, Duration::from_secs(8), |status| {
        status["state"] == "running" && status["active_service_count"] == 0
    });
    assert_eq!(revoked_status["idle"], true);

    assert_success(run_baudbound(
        &runner_home,
        ["script", "approve", "scheduled-log"],
    ));
    let reapproved_status =
        wait_for_service_status(&runner_home, Duration::from_secs(8), |status| {
            status["state"] == "running" && status["active_service_count"] == 1
        });

    request_service_control(&reapproved_status, "stop");
    assert_child_exits_successfully(serve, Duration::from_secs(8));

    let stopped_status = wait_for_service_status(&runner_home, Duration::from_secs(4), |status| {
        status["state"] == "stopped"
    });
    assert_eq!(stopped_status["state"], "stopped");
}

#[test]
fn cli_status_reports_tampered_installed_package() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner-home");
    let package_path = temporary_directory.path().join("cli-lifecycle.bbs");
    fs::write(
        &package_path,
        create_test_package("CLI Lifecycle", "hook", "tamper"),
    )
    .expect("test package should be written");

    assert_success(run_baudbound(
        &runner_home,
        [
            "script",
            "import",
            package_path.to_str().expect("path should be UTF-8"),
        ],
    ));
    fs::write(
        runner_home.join("scripts").join("cli-lifecycle.bbs"),
        b"tampered package bytes",
    )
    .expect("installed package should be tampered");

    let runner_status = command_json(run_baudbound(&runner_home, ["script", "status", "--json"]));
    assert_eq!(runner_status["problem_count"], 1);
    assert_eq!(
        runner_status["scripts"][0]["package_hash_status"]["state"],
        "mismatch"
    );
    assert!(runner_status["scripts"][0]["package_hash_status"]["expected"].is_string());
    assert!(runner_status["scripts"][0]["package_hash_status"]["actual"].is_string());
}

fn run_baudbound<const N: usize>(runner_home: &Path, args: [&str; N]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_baudbound"))
        .args(args)
        .env("BAUDBOUND_HOME", runner_home)
        .env_remove("BAUDBOUND_CONFIG")
        .output()
        .expect("baudbound command should run")
}

fn spawn_baudbound<const N: usize>(runner_home: &Path, args: [&str; N]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_baudbound"))
        .args(args)
        .env("BAUDBOUND_HOME", runner_home)
        .env_remove("BAUDBOUND_CONFIG")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("baudbound command should spawn")
}

fn wait_for_service_status(
    runner_home: &Path,
    timeout: Duration,
    predicate: impl Fn(&Value) -> bool,
) -> Value {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = read_service_status(runner_home)
            && predicate(&status)
        {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for service status in {}",
            runner_home.join("runner.sqlite3").display()
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn read_service_status(runner_home: &Path) -> Option<Value> {
    let connection = rusqlite::Connection::open(runner_home.join("runner.sqlite3")).ok()?;
    let status = connection
        .query_row(
            "SELECT status_json FROM service_status WHERE id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()?;
    serde_json::from_str(&status).ok()
}

fn request_service_control(status: &Value, command: &str) {
    let control = &status["control"];
    let address = control["address"]
        .as_str()
        .expect("IPC address should be present");
    let token = control["token"]
        .as_str()
        .expect("IPC token should be present");
    let protocol = control["protocol"]
        .as_str()
        .expect("IPC protocol should be present");
    let mut stream = TcpStream::connect(address).expect("IPC client should connect");
    serde_json::to_writer(
        &mut stream,
        &serde_json::json!({
            "command": command,
            "protocol": protocol,
            "token": token,
        }),
    )
    .expect("IPC request should serialize");
    stream.write_all(b"\n").expect("IPC request should write");
    stream.flush().expect("IPC request should flush");

    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .expect("IPC response should read");
    let response: Value = serde_json::from_str(&response).expect("IPC response should be JSON");
    assert_eq!(response["accepted"], true, "IPC command should be accepted");
}

fn assert_child_exits_successfully(mut child: Child, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait().expect("child status should be readable") {
            Some(status) => {
                assert!(status.success(), "serve child should exit successfully");
                return;
            }
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            None => {
                child.kill().expect("hung child should be killed");
                let output = child.wait_with_output().expect("child output should read");
                panic!(
                    "serve child did not exit before timeout\n{}",
                    command_output(&output)
                );
            }
        }
    }
}

fn service_row<'a>(document: &'a Value, name: &str) -> &'a Value {
    document["services"]
        .as_array()
        .expect("services should be a JSON array")
        .iter()
        .find(|service| service["name"] == name)
        .unwrap_or_else(|| panic!("service row {name:?} should exist"))
}

fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "command should succeed\n{}",
        command_output(&output)
    );
}

fn command_json(output: Output) -> Value {
    assert_success_like(&output);
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "command stdout should be valid JSON: {error}\n{}",
            command_output(&output)
        )
    })
}

fn assert_success_like(output: &Output) {
    assert!(
        output.status.success(),
        "command should succeed\n{}",
        command_output(output)
    );
}

fn command_output(output: &Output) -> String {
    format!(
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn create_test_package(script_name: &str, hook_name: &str, marker: &str) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    let manifest = format!(
        r#"{{
            "format_version": 1,
            "script_language_version": 1,
            "id": "cli-lifecycle",
            "name": "{script_name}",
            "created_with": "BaudBound CLI Test",
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
                "program": {{
                    "type": "block",
                    "execution_model": "directed_graph",
                    "runtime_context": {{
                        "expression_reference": "{{{{node-id.data_name}}}}",
                        "template_reference": "{{{{node-id.data_name}}}}",
                        "variables": [],
                        "built_in_variables": {{"syntax": "{{{{variable_name}}}}", "variables": []}},
                        "node_outputs": []
                    }},
                    "steps": [
                        {{
                            "id": "n-log",
                            "action_type": "action.log",
                            "type": "action",
                            "action": "log",
                            "config": {{"level": "info", "message": "cli lifecycle {marker}"}},
                            "runtime_outputs": []
                        }}
                    ],
                    "edges": [
                        {{
                            "execution_order": 0,
                            "source": "n-manual",
                            "source_handle": "out",
                            "target": "n-log",
                            "target_handle": "input"
                        }}
                    ]
                }}
            }}
        }}"#
    );

    for (path, content) in [
        ("manifest.json", manifest.as_str()),
        ("program.json", program.as_str()),
        (
            "permissions.json",
            r#"{"declared_permissions": ["log", "webhook_public_bind"], "risk_level": "high"}"#,
        ),
        (
            "capabilities.json",
            r#"{"required_capabilities": ["action.log", "trigger.manual", "trigger.webhook"], "target_runtime": "Generic Desktop"}"#,
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

fn create_schedule_package() -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for (path, content) in [
        (
            "manifest.json",
            r#"{
                "format_version": 1,
                "script_language_version": 1,
                "id": "scheduled-log",
                "name": "Scheduled Log",
                "created_with": "BaudBound CLI Test",
                "created_at": "2026-01-01T00:00:00.000Z",
                "minimum_runner_version": "0.1.0"
            }"#,
        ),
        (
            "program.json",
            r#"{
                "entry": {
                    "trigger": {
                        "id": "n-schedule",
                        "action_type": "trigger.schedule",
                        "type": "schedule",
                        "config": {"every": 30, "unit": "seconds"},
                        "runtime_outputs": []
                    },
                    "triggers": [
                        {
                            "id": "n-schedule",
                            "action_type": "trigger.schedule",
                            "type": "schedule",
                            "config": {"every": 30, "unit": "seconds"},
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
                        "steps": [
                            {
                                "id": "n-log",
                                "action_type": "action.log",
                                "type": "action",
                                "action": "log",
                                "config": {
                                    "level": "info",
                                    "message": "schedule fired every {{n-schedule.schedule.every}} {{n-schedule.schedule.unit}}"
                                },
                                "runtime_outputs": []
                            }
                        ],
                        "edges": [
                            {
                                "execution_order": 0,
                                "source": "n-schedule",
                                "source_handle": "out",
                                "target": "n-log",
                                "target_handle": "input"
                            }
                        ]
                    }
                }
            }"#,
        ),
        (
            "permissions.json",
            r#"{"declared_permissions": ["log"], "risk_level": "low"}"#,
        ),
        (
            "capabilities.json",
            r#"{"required_capabilities": ["action.log", "trigger.schedule"], "target_runtime": "Generic Desktop"}"#,
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
