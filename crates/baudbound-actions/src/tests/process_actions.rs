use std::{
    thread,
    time::{Duration, Instant},
};

use baudbound_runtime::{RuntimeActionError, RuntimeCancellationToken};
use serde_json::{Value, json};

use super::{execute, execute_with_cancellation};
use crate::actions::process::parse_command_arguments;

#[test]
fn parses_quoted_arguments_without_destroying_windows_paths() {
    let request = request("action.process.run");
    let arguments = parse_command_arguments(
        &request,
        r#"alpha "two words" 'three words' escaped\ value C:\Temp\file.txt quote\"value slash\\value"#,
    )
    .expect("valid arguments should parse");

    assert_eq!(
        arguments,
        [
            "alpha",
            "two words",
            "three words",
            "escaped value",
            r"C:\Temp\file.txt",
            "quote\"value",
            r"slash\value"
        ]
    );
}

#[test]
fn rejects_unterminated_quoted_arguments() {
    let request = request("action.process.run");
    let error = parse_command_arguments(&request, r#"one "unterminated"#)
        .expect_err("unterminated arguments must fail");
    assert!(error.to_string().contains("unterminated quoted string"));
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
            "arguments": "",
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

fn request(action_type: &str) -> baudbound_runtime::RuntimeActionRequest {
    baudbound_runtime::RuntimeActionRequest {
        action: None,
        action_type: action_type.to_owned(),
        config: Default::default(),
        node_id: "node-process-test".to_owned(),
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
fn failing_process_command() -> (&'static str, &'static str) {
    (
        "cmd",
        r#"/C "echo stdout-value & echo stderr-value 1>&2 & cd & exit /B 7""#,
    )
}

#[cfg(not(windows))]
fn failing_process_command() -> (&'static str, &'static str) {
    (
        "sh",
        r#"-c "printf stdout-value; printf stderr-value >&2; pwd; exit 7""#,
    )
}

#[cfg(windows)]
fn successful_process_command() -> (&'static str, &'static str) {
    ("cmd", "/C exit 0")
}

#[cfg(not(windows))]
fn successful_process_command() -> (&'static str, &'static str) {
    ("sh", "-c true")
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

#[cfg(not(windows))]
fn failing_shell_command() -> &'static str {
    "printf stdout-value; printf stderr-value >&2; exit 7"
}
