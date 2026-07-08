use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest};
use enigo::{Enigo, Settings};

use super::config::failed_error;

pub(super) fn native_input(request: &RuntimeActionRequest) -> Result<Enigo, RuntimeActionError> {
    Enigo::new(&Settings::default())
        .map_err(|source| failed_error(request, format!("native input init failed: {source}")))
}
