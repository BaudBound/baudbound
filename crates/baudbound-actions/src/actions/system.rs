use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};

use crate::failed;

pub(crate) fn desktop_only_action(
    request: &RuntimeActionRequest,
    capability: &str,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    failed(
        request,
        format!("{capability} requires the desktop runner action adapter"),
    )
}
