use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};

use super::config::failed_error;

pub(super) fn run_keyboard(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    unsupported_input(request)
}

pub(super) fn run_keyboard_type_text(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    unsupported_input(request)
}

pub(super) fn run_mouse_click(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    unsupported_input(request)
}

pub(super) fn run_mouse_move(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    unsupported_input(request)
}

fn unsupported_input(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    Err(failed_error(
        request,
        "native keyboard and mouse control requires Windows Desktop",
    ))
}
