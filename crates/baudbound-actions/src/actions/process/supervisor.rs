use std::{
    io::{self, Read},
    process::{Command, ExitStatus, Stdio},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeCancellationToken};
use command_group::{CommandGroup, GroupChild};

use crate::failed;

const POLL_INTERVAL: Duration = Duration::from_millis(25);
const MAX_CAPTURED_STREAM_BYTES: usize = 8 * 1024 * 1024;

pub(super) struct SupervisedOutput {
    pub(super) process_id: u32,
    pub(super) status: ExitStatus,
    pub(super) stderr: Vec<u8>,
    pub(super) stdout: Vec<u8>,
}

pub(super) fn run_command(
    request: &RuntimeActionRequest,
    command: &mut Command,
    cancellation: &RuntimeCancellationToken,
    timeout: Duration,
    description: &str,
) -> Result<SupervisedOutput, RuntimeActionError> {
    let deadline =
        Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("timeout is too large for {description}"),
            })?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut group = command.group();
    #[cfg(windows)]
    group.kill_on_drop(true);
    let mut child = group.spawn().map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("failed to start {description}: {source}"),
    })?;
    let process_id = child.id();
    let stdout = match child.inner().stdout.take() {
        Some(stdout) => stdout,
        None => return cleanup_after_setup_error(&mut child, request, description, "stdout"),
    };
    let stderr = match child.inner().stderr.take() {
        Some(stderr) => stderr,
        None => return cleanup_after_setup_error(&mut child, request, description, "stderr"),
    };
    let stdout_reader = match spawn_reader(stdout, "stdout", request) {
        Ok(reader) => reader,
        Err(error) => {
            let _ = terminate_group(&mut child, request, description);
            return Err(error);
        }
    };
    let stderr_reader = match spawn_reader(stderr, "stderr", request) {
        Ok(reader) => reader,
        Err(error) => {
            let _ = terminate_group(&mut child, request, description);
            let _ = join_reader(stdout_reader, "stdout", request);
            return Err(error);
        }
    };

    let termination = match wait_for_exit(&mut child, cancellation, deadline, request, description)
    {
        Ok(termination) => termination,
        Err(error) => {
            let _ = terminate_group(&mut child, request, description);
            let _ = join_reader(stdout_reader, "stdout", request);
            let _ = join_reader(stderr_reader, "stderr", request);
            return Err(error);
        }
    };
    let stdout = join_reader(stdout_reader, "stdout", request)?;
    let stderr = join_reader(stderr_reader, "stderr", request)?;

    match termination {
        Termination::Exited(status) => {
            ensure_not_truncated(request, description, &stdout, &stderr)?;
            Ok(SupervisedOutput {
                process_id,
                status,
                stderr: stderr.bytes,
                stdout: stdout.bytes,
            })
        }
        Termination::Cancelled => Err(RuntimeActionError::Cancelled),
        Termination::TimedOut => failed(
            request,
            format!(
                "{description} exceeded its timeout of {} seconds and was terminated",
                timeout.as_secs_f64()
            ),
        ),
    }
}

enum Termination {
    Cancelled,
    Exited(ExitStatus),
    TimedOut,
}

fn wait_for_exit(
    child: &mut GroupChild,
    cancellation: &RuntimeCancellationToken,
    deadline: Instant,
    request: &RuntimeActionRequest,
    description: &str,
) -> Result<Termination, RuntimeActionError> {
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("failed while waiting for {description}: {source}"),
            })?
        {
            return Ok(Termination::Exited(status));
        }

        if cancellation.is_cancelled() {
            terminate_group(child, request, description)?;
            return Ok(Termination::Cancelled);
        }

        let now = Instant::now();
        if now >= deadline {
            terminate_group(child, request, description)?;
            return Ok(Termination::TimedOut);
        }
        let _ = cancellation.wait_for(POLL_INTERVAL.min(deadline.saturating_duration_since(now)));
    }
}

fn cleanup_after_setup_error<T>(
    child: &mut GroupChild,
    request: &RuntimeActionRequest,
    description: &str,
    stream_name: &str,
) -> Result<T, RuntimeActionError> {
    let cleanup_error = terminate_group(child, request, description).err();
    let suffix = cleanup_error
        .map(|error| format!("; process cleanup also failed: {error}"))
        .unwrap_or_default();
    failed(
        request,
        format!("failed to capture {stream_name} for {description}{suffix}"),
    )
}

fn terminate_group(
    child: &mut GroupChild,
    request: &RuntimeActionRequest,
    description: &str,
) -> Result<(), RuntimeActionError> {
    let kill_error = child
        .kill()
        .err()
        .filter(|source| source.kind() != io::ErrorKind::InvalidInput);
    let wait_error = child.wait().err();

    match (kill_error, wait_error) {
        (None, None) => Ok(()),
        (Some(kill), None) => failed(
            request,
            format!("failed to terminate {description}: {kill}"),
        ),
        (None, Some(wait)) => failed(
            request,
            format!("failed to reap {description} after termination: {wait}"),
        ),
        (Some(kill), Some(wait)) => failed(
            request,
            format!("failed to terminate {description}: {kill}; failed to reap it: {wait}"),
        ),
    }
}

struct CapturedStream {
    bytes: Vec<u8>,
    truncated: bool,
}

fn spawn_reader<R>(
    mut reader: R,
    stream_name: &'static str,
    request: &RuntimeActionRequest,
) -> Result<JoinHandle<io::Result<CapturedStream>>, RuntimeActionError>
where
    R: Read + Send + 'static,
{
    thread::Builder::new()
        .name(format!("baudbound-process-{stream_name}"))
        .spawn(move || {
            let mut captured = Vec::new();
            let mut truncated = false;
            let mut buffer = [0_u8; 16 * 1024];
            loop {
                let count = reader.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                let available = MAX_CAPTURED_STREAM_BYTES.saturating_sub(captured.len());
                let retained = available.min(count);
                captured.extend_from_slice(&buffer[..retained]);
                truncated |= retained < count;
            }
            Ok(CapturedStream {
                bytes: captured,
                truncated,
            })
        })
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to start {stream_name} capture thread: {source}"),
        })
}

fn join_reader(
    reader: JoinHandle<io::Result<CapturedStream>>,
    stream_name: &str,
    request: &RuntimeActionRequest,
) -> Result<CapturedStream, RuntimeActionError> {
    reader
        .join()
        .map_err(|_| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("{stream_name} capture thread panicked"),
        })?
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to read {stream_name}: {source}"),
        })
}

fn ensure_not_truncated(
    request: &RuntimeActionRequest,
    description: &str,
    stdout: &CapturedStream,
    stderr: &CapturedStream,
) -> Result<(), RuntimeActionError> {
    if stdout.truncated || stderr.truncated {
        return failed(
            request,
            format!(
                "{description} output exceeded the {} MiB per-stream capture limit",
                MAX_CAPTURED_STREAM_BYTES / (1024 * 1024)
            ),
        );
    }
    Ok(())
}
