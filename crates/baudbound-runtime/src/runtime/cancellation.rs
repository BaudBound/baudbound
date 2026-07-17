use std::{
    fmt,
    sync::{
        Arc, Condvar, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Default)]
pub struct RuntimeCancellationToken {
    inner: Arc<CancellationState>,
}

#[derive(Default)]
struct CancellationState {
    cancelled: AtomicBool,
    changed: Condvar,
    children: Mutex<Vec<Weak<CancellationState>>>,
    wait_lock: Mutex<()>,
}

impl RuntimeCancellationToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        cancel_state(&self.inner);
    }

    #[must_use]
    pub fn child_token(&self) -> Self {
        let child = Self::new();
        let mut children = self
            .inner
            .children
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        children.retain(|candidate| candidate.strong_count() > 0);
        if self.is_cancelled() {
            child.cancel();
        } else {
            children.push(Arc::downgrade(&child.inner));
        }
        child
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn wait_for(&self, duration: Duration) -> bool {
        if self.is_cancelled() {
            return true;
        }

        let guard = self
            .inner
            .wait_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _guard = self
            .inner
            .changed
            .wait_timeout_while(guard, duration, |_| !self.is_cancelled())
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.is_cancelled()
    }
}

fn cancel_state(state: &Arc<CancellationState>) {
    if state.cancelled.swap(true, Ordering::AcqRel) {
        return;
    }
    state.changed.notify_all();
    let children = {
        let mut children = state
            .children
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let active = children
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        children.retain(|candidate| candidate.strong_count() > 0);
        active
    };
    for child in children {
        cancel_state(&child);
    }
}

impl fmt::Debug for RuntimeCancellationToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeCancellationToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}
