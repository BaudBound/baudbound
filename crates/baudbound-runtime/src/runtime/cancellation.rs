use std::{
    fmt,
    sync::{
        Arc, Condvar, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use crossbeam_channel::{Receiver, Sender, bounded};

#[derive(Clone, Default)]
pub struct RuntimeCancellationToken {
    inner: Arc<CancellationState>,
}

#[derive(Default)]
struct CancellationState {
    cancelled: AtomicBool,
    changed: Condvar,
    children: Mutex<Vec<Weak<CancellationState>>>,
    next_subscriber_id: AtomicU64,
    subscribers: Mutex<Vec<(u64, Sender<()>)>>,
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

    #[must_use]
    pub fn subscribe(&self) -> RuntimeCancellationSubscription {
        let (sender, receiver) = bounded(1);
        let id = self
            .inner
            .next_subscriber_id
            .fetch_add(1, Ordering::Relaxed);
        let mut subscribers = self
            .inner
            .subscribers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.is_cancelled() {
            let _ = sender.send(());
        } else {
            subscribers.push((id, sender));
        }
        RuntimeCancellationSubscription {
            id,
            receiver,
            state: Arc::downgrade(&self.inner),
        }
    }
}

pub struct RuntimeCancellationSubscription {
    id: u64,
    receiver: Receiver<()>,
    state: Weak<CancellationState>,
}

impl RuntimeCancellationSubscription {
    #[must_use]
    pub fn receiver(&self) -> &Receiver<()> {
        &self.receiver
    }
}

impl Drop for RuntimeCancellationSubscription {
    fn drop(&mut self) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        let mut subscribers = state
            .subscribers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        subscribers.retain(|(id, _)| *id != self.id);
    }
}

fn cancel_state(state: &Arc<CancellationState>) {
    if state.cancelled.swap(true, Ordering::AcqRel) {
        return;
    }
    state.changed.notify_all();
    {
        let mut subscribers = state
            .subscribers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for (_, subscriber) in subscribers.drain(..) {
            let _ = subscriber.try_send(());
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_subscriptions_unregister_immediately() {
        let token = RuntimeCancellationToken::new();
        let subscription = token.subscribe();
        assert_eq!(
            token
                .inner
                .subscribers
                .lock()
                .expect("lock should succeed")
                .len(),
            1
        );

        drop(subscription);

        assert!(
            token
                .inner
                .subscribers
                .lock()
                .expect("lock should succeed")
                .is_empty()
        );
    }

    #[test]
    fn cancellation_notifies_current_and_late_subscribers() {
        let token = RuntimeCancellationToken::new();
        let current = token.subscribe();

        token.cancel();

        current
            .receiver()
            .recv_timeout(Duration::from_millis(50))
            .expect("current subscriber should be notified");
        let late = token.subscribe();
        late.receiver()
            .recv_timeout(Duration::from_millis(50))
            .expect("late subscriber should be notified immediately");
    }
}
