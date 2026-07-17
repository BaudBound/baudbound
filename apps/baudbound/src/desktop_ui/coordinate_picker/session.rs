#[cfg(windows)]
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

#[cfg(windows)]
pub(in crate::desktop_ui) struct CoordinatePickerState {
    active: Mutex<Option<PickerSession>>,
    next_session_id: AtomicU64,
}

#[cfg(not(windows))]
#[derive(Default)]
pub(in crate::desktop_ui) struct CoordinatePickerState {
    _unsupported: (),
}

#[cfg(windows)]
impl Default for CoordinatePickerState {
    fn default() -> Self {
        Self {
            active: Mutex::new(None),
            next_session_id: AtomicU64::new(1),
        }
    }
}

#[cfg(windows)]
impl CoordinatePickerState {
    pub(super) fn reserve(&self) -> Result<String, String> {
        let mut active = self.lock_active()?;
        if active.is_some() {
            return Err("A screen coordinate picker is already open.".to_owned());
        }

        let session_id = format!(
            "{:016x}",
            self.next_session_id.fetch_add(1, Ordering::Relaxed)
        );
        *active = Some(PickerSession {
            id: session_id.clone(),
            window_labels: Vec::new(),
        });
        Ok(session_id)
    }

    pub(super) fn set_windows(
        &self,
        session_id: &str,
        window_labels: Vec<String>,
    ) -> Result<(), String> {
        let mut active = self.lock_active()?;
        let session = active
            .as_mut()
            .filter(|session| session.id == session_id)
            .ok_or_else(|| "the coordinate picker session is no longer active".to_owned())?;
        session.window_labels = window_labels;
        Ok(())
    }

    pub(super) fn take(&self, session_id: &str) -> Result<PickerSession, String> {
        let mut active = self.lock_active()?;
        if active
            .as_ref()
            .is_none_or(|session| session.id != session_id)
        {
            return Err("The coordinate picker session is no longer active.".to_owned());
        }
        active
            .take()
            .ok_or_else(|| "The coordinate picker session is no longer active.".to_owned())
    }

    pub(super) fn clear(&self, session_id: &str) {
        if let Ok(mut active) = self.active.lock()
            && active
                .as_ref()
                .is_some_and(|session| session.id == session_id)
        {
            *active = None;
        }
    }

    fn lock_active(&self) -> Result<std::sync::MutexGuard<'_, Option<PickerSession>>, String> {
        self.active
            .lock()
            .map_err(|_| "coordinate picker state lock is poisoned".to_owned())
    }
}

#[cfg(windows)]
#[derive(Debug)]
pub(super) struct PickerSession {
    pub(super) id: String,
    pub(super) window_labels: Vec<String>,
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn permits_only_one_picker_session_and_releases_it_after_completion() {
        let state = CoordinatePickerState::default();

        let first = state.reserve().expect("first session should be reserved");
        assert_eq!(
            state.reserve(),
            Err("A screen coordinate picker is already open.".to_owned())
        );
        assert!(state.take("wrong").is_err());
        assert_eq!(
            state
                .take(&first)
                .expect("first session should be released")
                .id,
            first
        );
        let second = state.reserve().expect("next session should be permitted");
        state.clear(&second);
        state
            .reserve()
            .expect("a failed startup reservation should be released");
    }
}
