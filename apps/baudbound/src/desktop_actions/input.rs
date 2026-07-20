use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest};
use enigo::{Enigo, Settings};

use super::config::failed_error;

mod state;

pub(super) use state::NativeInputState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputAction {
    Press,
    Down,
    Up,
}

impl InputAction {
    pub(super) fn from_request(request: &RuntimeActionRequest) -> Result<Self, RuntimeActionError> {
        let value = request
            .config
            .get("inputAction")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("press")
            .trim()
            .to_ascii_lowercase();
        match value.as_str() {
            "press" => Ok(Self::Press),
            "down" => Ok(Self::Down),
            "up" => Ok(Self::Up),
            _ => Err(failed_error(
                request,
                format!("unsupported input action {value:?}"),
            )),
        }
    }

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Press => "press",
            Self::Down => "down",
            Self::Up => "up",
        }
    }
}

pub(super) fn native_input(request: &RuntimeActionRequest) -> Result<Enigo, RuntimeActionError> {
    native_input_raw()
        .map_err(|source| failed_error(request, format!("native input init failed: {source}")))
}

fn native_input_raw() -> Result<Enigo, String> {
    Enigo::new(&Settings::default()).map_err(|source| source.to_string())
}
