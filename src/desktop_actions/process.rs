use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};

#[cfg(not(windows))]
use super::config::failed_error;
use super::config::required_string;

pub(super) fn run_process_status_by_window_title(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let target = required_string(request, "target")?;
    native_process_status_by_window_title(request, &target)
}

pub(super) fn run_kill_process_by_window_title(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let target = required_string(request, "target")?;
    native_kill_process_by_window_title(request, &target)
}

#[cfg(windows)]
fn native_process_status_by_window_title(
    request: &RuntimeActionRequest,
    target: &str,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    super::windows_desktop::process::process_status_by_window_title(request, target)
}

#[cfg(not(windows))]
fn native_process_status_by_window_title(
    request: &RuntimeActionRequest,
    _target: &str,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    Err(failed_error(
        request,
        "process window-title queries require Windows Desktop",
    ))
}

#[cfg(windows)]
fn native_kill_process_by_window_title(
    request: &RuntimeActionRequest,
    target: &str,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    super::windows_desktop::process::kill_process_by_window_title(request, target)
}

#[cfg(not(windows))]
fn native_kill_process_by_window_title(
    request: &RuntimeActionRequest,
    _target: &str,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    Err(failed_error(
        request,
        "process window-title termination requires Windows Desktop",
    ))
}
