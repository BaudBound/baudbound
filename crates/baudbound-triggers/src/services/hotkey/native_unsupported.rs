use std::sync::mpsc::SyncSender;

use crate::{TriggerError, TriggerEvent, TriggerRegistration, TriggerServiceDiagnostics};

pub struct NativeHotkeyService;

impl NativeHotkeyService {
    #[must_use]
    pub fn empty() -> Self {
        Self
    }

    pub fn start(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        _sender: SyncSender<TriggerEvent>,
    ) -> Result<Self, TriggerError> {
        if registrations
            .into_iter()
            .any(|registration| registration.action_type == "trigger.hotkey")
        {
            return Err(TriggerError::Unsupported(
                "trigger.hotkey requires Windows Desktop".to_owned(),
            ));
        }
        Ok(Self)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        true
    }

    #[must_use]
    pub fn len(&self) -> usize {
        0
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics {
            running: false,
            state: "idle",
            summary: "0 native Windows hotkey binding(s) registered".to_owned(),
        }
    }
}
