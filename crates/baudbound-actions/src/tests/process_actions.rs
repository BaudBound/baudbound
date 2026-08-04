use std::{
    thread,
    time::{Duration, Instant},
};

use baudbound_runtime::{ResourceLimit, RuntimeActionError, RuntimeCancellationToken};
use serde_json::{Value, json};

use super::{execute, execute_with_cancellation, execute_with_handler};
use crate::{ActionLimits, HeadlessActionHandler};

#[test]
fn run_process_rejects_string_arguments() {
    let error = execute(
        "action.process.run",
        json!({
            "executable": "baudbound-definitely-missing-executable",
            "arguments": "--old string form",
            "workingDirectory": ""
        }),
    )
    .expect_err("string arguments must be rejected before starting the process");

    assert!(error.to_string().contains("arguments must be an array"));
}

#[test]
fn run_process_captures_working_directory_stdout_stderr_and_nonzero_exit() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let (executable, arguments) = failing_process_command();
    let result = execute(
        "action.process.run",
        json!({
            "executable": executable,
            "arguments": arguments,
            "workingDirectory": directory.path()
        }),
    )
    .expect("process should start and return its nonzero result");

    assert_eq!(result.output_data.get("exit_code"), Some(&json!(7)));
    assert_eq!(result.output_data.get("success"), Some(&json!(false)));
    assert!(output(&result, "stdout").contains("stdout-value"));
    assert!(output(&result, "stderr").contains("stderr-value"));
    assert!(
        normalize_path(output(&result, "stdout"))
            .contains(&normalize_path(&directory.path().display().to_string()))
    );
}

#[test]
fn run_process_rejects_missing_executables_and_working_directories() {
    let executable_error = execute(
        "action.process.run",
        json!({
            "executable": "baudbound-definitely-missing-executable",
            "arguments": [],
            "workingDirectory": ""
        }),
    )
    .expect_err("missing executable must fail");
    assert!(
        executable_error
            .to_string()
            .contains("failed to start process")
    );

    let (executable, arguments) = successful_process_command();
    let working_directory_error = execute(
        "action.process.run",
        json!({
            "executable": executable,
            "arguments": arguments,
            "workingDirectory": "baudbound-definitely-missing-directory"
        }),
    )
    .expect_err("missing working directory must fail");
    assert!(
        working_directory_error
            .to_string()
            .contains("failed to start process")
    );
}

#[test]
fn process_status_supports_pid_path_name_and_not_found_results() {
    let current_pid = std::process::id();
    let current_executable = std::env::current_exe().expect("current executable should resolve");
    let process_name = current_executable
        .file_name()
        .and_then(|value| value.to_str())
        .expect("process name should be UTF-8");

    for (match_mode, target) in [
        ("pid", current_pid.to_string()),
        ("process_name", process_name.to_owned()),
        ("executable_path", current_executable.display().to_string()),
    ] {
        let result = execute(
            "action.process.status",
            json!({"matchMode": match_mode, "target": target}),
        )
        .unwrap_or_else(|error| panic!("{match_mode} lookup should succeed: {error}"));
        assert_eq!(result.output_data.get("running"), Some(&json!(true)));
        assert_eq!(
            result.output_data.get("process_id"),
            Some(&json!(current_pid))
        );
    }

    let missing = execute(
        "action.process.status",
        json!({"matchMode": "pid", "target": u32::MAX.to_string()}),
    )
    .expect("missing process is a successful status query");
    assert_eq!(missing.output_data.get("running"), Some(&json!(false)));
    assert_eq!(missing.output_data.get("state"), Some(&json!("not_found")));
    assert_eq!(missing.output_data.get("process_id"), Some(&Value::Null));
}

#[test]
fn process_queries_reject_invalid_and_desktop_only_match_modes() {
    for config in [
        json!({"matchMode": "pid", "target": "not-a-pid"}),
        json!({"matchMode": "unsupported", "target": "value"}),
        json!({"matchMode": "window_title", "target": "Window"}),
    ] {
        let error = execute("action.process.status", config)
            .expect_err("unsupported process query must fail");
        assert!(!error.to_string().trim().is_empty());
    }
}

#[test]
fn kill_process_rejects_invalid_or_missing_targets() {
    for config in [
        json!({"matchMode": "pid", "target": "not-a-pid"}),
        json!({"matchMode": "pid", "target": u32::MAX.to_string()}),
        json!({"matchMode": "unsupported", "target": "value"}),
        json!({"matchMode": "window_title", "target": "Window"}),
    ] {
        let error = execute("action.process.kill", config)
            .expect_err("invalid process kill target must fail");
        assert!(!error.to_string().trim().is_empty());
    }
}

#[test]
fn shell_command_captures_nonzero_exit_stdout_and_stderr() {
    let result = execute("action.shell", json!({"command": failing_shell_command()}))
        .expect("shell should return nonzero command output");

    assert_eq!(result.output_data.get("exit_code"), Some(&json!(7)));
    assert_eq!(result.output_data.get("success"), Some(&json!(false)));
    assert!(output(&result, "stdout").contains("stdout-value"));
    assert!(output(&result, "stderr").contains("stderr-value"));
}

#[cfg(windows)]
#[test]
fn shell_command_preserves_nested_powershell_quotes_and_script_blocks() {
    let command = r#"powershell -NoProfile -Command "$r='expected output'; if($r){if($null-ne $r.Length){$r}else{'missing'}}""#;
    let result = execute("action.shell", json!({"command": command}))
        .expect("nested PowerShell command should execute");

    assert_eq!(result.output_data.get("success"), Some(&json!(true)));
    assert_eq!(output(&result, "stdout").trim(), "expected output");
    assert_eq!(output(&result, "stderr"), "");
}

#[test]
fn shell_command_timeout_terminates_the_process_group_promptly() {
    let started = Instant::now();
    let error = execute(
        "action.shell",
        json!({"command": long_running_shell_command(), "timeoutSeconds": 1}),
    )
    .expect_err("a command exceeding its deadline must fail");

    assert!(error.to_string().contains("exceeded its timeout"));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn shell_command_cancellation_terminates_the_process_group_promptly() {
    let cancellation = RuntimeCancellationToken::new();
    let cancellation_signal = cancellation.clone();
    let canceller = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        cancellation_signal.cancel();
    });
    let started = Instant::now();
    let error = execute_with_cancellation(
        "action.shell",
        json!({"command": long_running_shell_command(), "timeoutSeconds": 30}),
        cancellation,
    )
    .expect_err("a cancelled command must fail as cancelled");
    canceller.join().expect("cancellation thread should finish");

    assert!(matches!(error, RuntimeActionError::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn shell_command_drains_stdout_and_stderr_without_pipe_deadlock() {
    let result = execute(
        "action.shell",
        json!({"command": high_output_shell_command(), "timeoutSeconds": 10}),
    )
    .expect("concurrent stdout and stderr output should be drained");

    assert_eq!(result.output_data.get("success"), Some(&json!(true)));
    assert!(output(&result, "stdout").contains("stdout-value"));
    assert!(output(&result, "stderr").contains("stderr-value"));
}

#[test]
fn process_output_limit_fails_after_draining_both_streams() {
    let handler = HeadlessActionHandler::default().with_limits(ActionLimits {
        max_process_output_bytes: ResourceLimit::limited(4),
        ..ActionLimits::default()
    });
    let error = execute_with_handler(
        &handler,
        "action.shell",
        json!({"command": output_limit_shell_command(), "timeoutSeconds": 5}),
        Value::Null,
    )
    .expect_err("process output over the configured limit must fail");

    assert!(
        error
            .to_string()
            .contains("configured 4 byte per-stream capture limit")
    );
}

#[test]
fn repeated_process_launches_release_the_active_process_permit() {
    let handler = HeadlessActionHandler::default().with_limits(ActionLimits {
        max_processes_per_script: ResourceLimit::limited(1),
        max_process_launches_per_minute: ResourceLimit::Unlimited,
        ..ActionLimits::default()
    });
    let (executable, arguments) = successful_process_command();

    for _ in 0..32 {
        let result = execute_with_handler(
            &handler,
            "action.process.run",
            json!({
                "executable": executable,
                "arguments": arguments,
                "workingDirectory": ""
            }),
            Value::Null,
        )
        .expect("a completed process must release the single active-process slot");
        assert_eq!(result.output_data.get("success"), Some(&json!(true)));
    }
}

#[test]
fn process_and_shell_reject_invalid_timeouts() {
    for timeout in [json!(0), json!(86_401), json!("not-a-number")] {
        let error = execute(
            "action.shell",
            json!({"command": successful_shell_command(), "timeoutSeconds": timeout}),
        )
        .expect_err("invalid timeout must be rejected before starting the command");
        assert!(error.to_string().contains("timeoutSeconds"));
    }
}

fn output<'a>(result: &'a baudbound_runtime::RuntimeActionResult, key: &str) -> &'a str {
    result.output_data[key]
        .as_str()
        .expect("process output should be text")
}

fn normalize_path(value: &str) -> String {
    value.trim().replace('\\', "/").to_ascii_lowercase()
}

#[cfg(windows)]
fn failing_process_command() -> (&'static str, Vec<&'static str>) {
    (
        "cmd",
        vec![
            "/C",
            "echo stdout-value & echo stderr-value 1>&2 & cd & exit /B 7",
        ],
    )
}

#[cfg(not(windows))]
fn failing_process_command() -> (&'static str, Vec<&'static str>) {
    (
        "sh",
        vec![
            "-c",
            "printf stdout-value; printf stderr-value >&2; pwd; exit 7",
        ],
    )
}

#[cfg(windows)]
fn successful_process_command() -> (&'static str, Vec<&'static str>) {
    ("cmd", vec!["/C", "exit 0"])
}

#[cfg(not(windows))]
fn successful_process_command() -> (&'static str, Vec<&'static str>) {
    ("sh", vec!["-c", "true"])
}

#[cfg(windows)]
fn failing_shell_command() -> &'static str {
    "echo stdout-value & echo stderr-value 1>&2 & exit /B 7"
}

#[cfg(windows)]
fn long_running_shell_command() -> &'static str {
    "ping 127.0.0.1 -n 30 >nul"
}

#[cfg(not(windows))]
fn long_running_shell_command() -> &'static str {
    "sleep 30"
}

#[cfg(windows)]
fn high_output_shell_command() -> &'static str {
    "for /L %i in (1,1,20000) do @(echo stdout-value-%i& echo stderr-value-%i 1>&2)"
}

#[cfg(not(windows))]
fn high_output_shell_command() -> &'static str {
    "i=0; while [ $i -lt 20000 ]; do printf 'stdout-value-%s\\n' \"$i\"; printf 'stderr-value-%s\\n' \"$i\" >&2; i=$((i + 1)); done"
}

#[cfg(windows)]
fn successful_shell_command() -> &'static str {
    "exit /B 0"
}

#[cfg(not(windows))]
fn successful_shell_command() -> &'static str {
    "true"
}

#[cfg(windows)]
fn output_limit_shell_command() -> &'static str {
    "echo 12345 & echo abcde 1>&2"
}

#[cfg(not(windows))]
fn output_limit_shell_command() -> &'static str {
    "printf 12345; printf abcde >&2"
}

#[cfg(not(windows))]
fn failing_shell_command() -> &'static str {
    "printf stdout-value; printf stderr-value >&2; exit 7"
}
