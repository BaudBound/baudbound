use std::{
    process::{Command, Stdio},
    thread,
};

use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use serde_json::{Map, Number, Value};
use sysinfo::{Pid, ProcessesToUpdate, Signal, System};

use crate::{config_string, failed, required_string};

pub(crate) fn process_status_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let target = required_string(request, "target")?;
    let match_mode =
        config_string(&request.config, "matchMode").unwrap_or_else(|| "process_name".to_owned());
    let system = process_system();
    let process = find_process(request, &system, &match_mode, &target)?;

    let output_data = match process {
        Some(process) => process_status_output(process, true),
        None => Map::from_iter([
            ("running".to_owned(), Value::Bool(false)),
            ("state".to_owned(), Value::String("not_found".to_owned())),
            ("process_id".to_owned(), Value::Null),
            ("process_name".to_owned(), Value::Null),
            ("executable_path".to_owned(), Value::Null),
        ]),
    };

    Ok(RuntimeActionResult { output_data })
}

pub(crate) fn kill_process_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let target = required_string(request, "target")?;
    let match_mode =
        config_string(&request.config, "matchMode").unwrap_or_else(|| "process_name".to_owned());
    let system = process_system();
    let Some(process) = find_process(request, &system, &match_mode, &target)? else {
        return failed(
            request,
            format!("no process matched {match_mode} target {target}"),
        );
    };

    let mut output_data = process_status_output(process, true);
    let killed = process
        .kill_with(Signal::Kill)
        .unwrap_or_else(|| process.kill());
    output_data.insert("killed".to_owned(), Value::Bool(killed));
    if !killed {
        return failed(
            request,
            format!(
                "failed to terminate process {} ({})",
                process.pid(),
                process.name().to_string_lossy()
            ),
        );
    }

    Ok(RuntimeActionResult { output_data })
}

pub(crate) fn open_application_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let application = required_string(request, "application")?;
    let arguments = config_string(&request.config, "arguments").unwrap_or_default();
    let parsed_arguments = parse_command_arguments(request, &arguments)?;

    let mut command = Command::new(&application);
    command
        .args(&parsed_arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command
        .spawn()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to open application {application}: {source}"),
        })?;
    let process_id = child.id();

    thread::spawn(move || {
        let _ = child.wait();
    });

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("application_id".to_owned(), Value::String(application)),
            (
                "process_id".to_owned(),
                Value::Number(Number::from(process_id)),
            ),
            (
                "arguments".to_owned(),
                Value::Array(parsed_arguments.into_iter().map(Value::String).collect()),
            ),
        ]),
    })
}

pub(crate) fn run_process_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let executable = required_string(request, "executable")?;
    let arguments = config_string(&request.config, "arguments").unwrap_or_default();
    let working_directory = config_string(&request.config, "workingDirectory").unwrap_or_default();
    let parsed_arguments = parse_command_arguments(request, &arguments)?;

    let mut command = Command::new(&executable);
    command.args(&parsed_arguments);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if !working_directory.trim().is_empty() {
        command.current_dir(&working_directory);
    }

    let child = command
        .spawn()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to start process {executable}: {source}"),
        })?;
    let process_id = child.id();
    let output = child
        .wait_with_output()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed while waiting for process {executable}: {source}"),
        })?;

    Ok(process_result(
        process_id,
        output.status.code(),
        output.stdout,
        output.stderr,
    ))
}

pub(crate) fn shell_command_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let command = required_string(request, "command")?;
    let mut shell = platform_shell_command(&command);
    shell.stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = shell.spawn().map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to start shell command: {source}"),
    })?;
    let process_id = child.id();
    let output = child
        .wait_with_output()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed while waiting for shell command: {source}"),
        })?;

    Ok(process_result(
        process_id,
        output.status.code(),
        output.stdout,
        output.stderr,
    ))
}

fn process_result(
    process_id: u32,
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
) -> RuntimeActionResult {
    RuntimeActionResult {
        output_data: Map::from_iter([
            (
                "process_id".to_owned(),
                Value::Number(Number::from(process_id)),
            ),
            (
                "exit_code".to_owned(),
                exit_code.map_or(Value::Null, |code| Value::Number(Number::from(code))),
            ),
            (
                "success".to_owned(),
                Value::Bool(exit_code.is_some_and(|code| code == 0)),
            ),
            (
                "stdout".to_owned(),
                Value::String(String::from_utf8_lossy(&stdout).to_string()),
            ),
            (
                "stderr".to_owned(),
                Value::String(String::from_utf8_lossy(&stderr).to_string()),
            ),
        ]),
    }
}

fn platform_shell_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut shell = Command::new("cmd");
        shell.args(["/C", command]);
        shell
    }

    #[cfg(not(windows))]
    {
        let mut shell = Command::new("sh");
        shell.args(["-c", command]);
        shell
    }
}

fn process_system() -> System {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system
}

fn find_process<'a>(
    request: &RuntimeActionRequest,
    system: &'a System,
    match_mode: &str,
    target: &str,
) -> Result<Option<&'a sysinfo::Process>, RuntimeActionError> {
    match match_mode {
        "pid" => {
            let process_id =
                target
                    .trim()
                    .parse::<usize>()
                    .map_err(|source| RuntimeActionError::Failed {
                        action_type: request.action_type.clone(),
                        message: format!("invalid process id {target}: {source}"),
                    })?;
            Ok(system.process(Pid::from(process_id)))
        }
        "process_name" => Ok(system
            .processes()
            .values()
            .filter(|process| {
                process
                    .name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(target.trim())
            })
            .min_by_key(|process| process.pid().as_u32())),
        "executable_path" => {
            let normalized_target = normalize_path_string(target);
            Ok(system
                .processes()
                .values()
                .filter(|process| {
                    process
                        .exe()
                        .map(|path| normalize_path_string(&path.display().to_string()))
                        .is_some_and(|path| path == normalized_target)
                })
                .min_by_key(|process| process.pid().as_u32()))
        }
        "window_title" => Err(RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: "window title matching is only available in the desktop runner".to_owned(),
        }),
        other => Err(RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("unsupported process match mode {other}"),
        }),
    }
}

fn process_status_output(process: &sysinfo::Process, running: bool) -> Map<String, Value> {
    Map::from_iter([
        ("running".to_owned(), Value::Bool(running)),
        (
            "state".to_owned(),
            Value::String(if running { "running" } else { "not_found" }.to_owned()),
        ),
        (
            "process_id".to_owned(),
            Value::Number(Number::from(process.pid().as_u32())),
        ),
        (
            "process_name".to_owned(),
            Value::String(process.name().to_string_lossy().to_string()),
        ),
        (
            "executable_path".to_owned(),
            process.exe().map_or(Value::Null, |path| {
                Value::String(path.display().to_string())
            }),
        ),
    ])
}

fn normalize_path_string(path: &str) -> String {
    let normalized = path.trim().replace('\\', "/");
    if cfg!(windows) {
        normalized.to_ascii_lowercase()
    } else {
        normalized
    }
}

pub(crate) fn parse_command_arguments(
    request: &RuntimeActionRequest,
    input: &str,
) -> Result<Vec<String>, RuntimeActionError> {
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut quote = None::<char>;

    while let Some(character) = chars.next() {
        if character == '\\' {
            let escaped_character = chars.peek().copied().filter(|next| {
                *next == '\\' || next.is_whitespace() || matches!(*next, '"' | '\'')
            });
            if let Some(escaped_character) = escaped_character {
                chars.next();
                current.push(escaped_character);
            } else {
                current.push('\\');
            }
            continue;
        }

        match quote {
            Some(active_quote) if character == active_quote => quote = None,
            Some(_) => current.push(character),
            None if matches!(character, '"' | '\'') => quote = Some(character),
            None if character.is_whitespace() => {
                if !current.is_empty() {
                    arguments.push(std::mem::take(&mut current));
                }
                while matches!(chars.peek(), Some(next) if next.is_whitespace()) {
                    chars.next();
                }
            }
            None => current.push(character),
        }
    }

    if quote.is_some() {
        return failed(
            request,
            "process arguments contain an unterminated quoted string",
        );
    }
    if !current.is_empty() {
        arguments.push(current);
    }

    Ok(arguments)
}
