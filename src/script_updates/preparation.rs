use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_ACTIVE_PREPARATIONS: usize = 16;
const MAX_REQUEST_ID_LENGTH: usize = 128;

#[derive(Clone, Default)]
pub(crate) struct RemotePreparationRegistry {
    active: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl RemotePreparationRegistry {
    pub(crate) fn start(&self, request_id: &str) -> Result<RemotePreparationGuard, String> {
        validate_request_id(request_id)?;
        let mut active = self
            .active
            .lock()
            .map_err(|_| "remote preparation lock is poisoned".to_owned())?;
        if active.contains_key(request_id) {
            return Err("this remote preparation request is already active".to_owned());
        }
        if active.len() >= MAX_ACTIVE_PREPARATIONS {
            return Err("too many remote package downloads are active".to_owned());
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        active.insert(request_id.to_owned(), Arc::clone(&cancelled));
        Ok(RemotePreparationGuard {
            active: Arc::clone(&self.active),
            cancelled,
            request_id: request_id.to_owned(),
        })
    }

    pub(crate) fn cancel(&self, request_id: &str) -> Result<bool, String> {
        validate_request_id(request_id)?;
        let active = self
            .active
            .lock()
            .map_err(|_| "remote preparation lock is poisoned".to_owned())?;
        let Some(cancelled) = active.get(request_id) else {
            return Ok(false);
        };
        cancelled.store(true, Ordering::Release);
        Ok(true)
    }
}

pub(crate) struct RemotePreparationGuard {
    active: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    cancelled: Arc<AtomicBool>,
    request_id: String,
}

impl RemotePreparationGuard {
    pub(crate) fn cancellation_token(&self) -> RemoteCancellationToken {
        RemoteCancellationToken {
            cancelled: Arc::clone(&self.cancelled),
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Clone)]
pub(crate) struct RemoteCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl RemoteCancellationToken {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl Drop for RemotePreparationGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(&self.request_id);
        }
    }
}

fn validate_request_id(request_id: &str) -> Result<(), String> {
    if request_id.is_empty()
        || request_id.len() > MAX_REQUEST_ID_LENGTH
        || !request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("the remote preparation request ID is invalid".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_requests_can_be_cancelled_and_are_removed_on_drop() {
        let registry = RemotePreparationRegistry::default();
        let guard = registry.start("request-1").unwrap();
        assert!(!guard.is_cancelled());
        assert!(registry.cancel("request-1").unwrap());
        assert!(guard.is_cancelled());
        drop(guard);
        assert!(!registry.cancel("request-1").unwrap());
        assert!(registry.start("request-1").is_ok());
    }

    #[test]
    fn duplicate_and_invalid_request_ids_are_rejected() {
        let registry = RemotePreparationRegistry::default();
        let _guard = registry.start("request_1").unwrap();
        assert!(registry.start("request_1").is_err());
        assert!(registry.start("request with spaces").is_err());
    }
}
