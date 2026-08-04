use std::{
    collections::{HashMap, VecDeque},
    net::IpAddr,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use baudbound_runtime::ResourceLimit;

pub struct ConnectionGate {
    active: AtomicUsize,
    limit: ResourceLimit,
}

pub struct ConnectionPermit {
    gate: Arc<ConnectionGate>,
}

pub struct PreAuthRateLimiter {
    global_limit: ResourceLimit,
    per_address_limit: ResourceLimit,
    state: Mutex<RateState>,
    window: Duration,
}

#[derive(Default)]
struct RateState {
    by_address: HashMap<IpAddr, usize>,
    requests: VecDeque<RateRecord>,
}

#[derive(Clone, Copy)]
struct RateRecord {
    address: IpAddr,
    received_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreAuthRateLimit {
    Address,
    Global,
}

impl ConnectionGate {
    #[must_use]
    pub fn new(limit: ResourceLimit) -> Self {
        Self {
            active: AtomicUsize::new(0),
            limit,
        }
    }

    pub fn try_acquire(self: &Arc<Self>) -> Option<ConnectionPermit> {
        self.active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                let next = current.checked_add(1)?;
                self.limit
                    .permits(u64::try_from(next).unwrap_or(u64::MAX))
                    .then_some(next)
            })
            .ok()?;
        Some(ConnectionPermit {
            gate: Arc::clone(self),
        })
    }

    #[must_use]
    pub fn active(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        let previous = self.gate.active.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "connection permit accounting underflowed");
    }
}

impl PreAuthRateLimiter {
    #[must_use]
    pub fn per_minute(global: ResourceLimit, per_address: ResourceLimit) -> Self {
        Self::new(global, per_address, Duration::from_secs(60))
    }

    #[must_use]
    pub fn new(
        global_limit: ResourceLimit,
        per_address_limit: ResourceLimit,
        window: Duration,
    ) -> Self {
        Self {
            global_limit,
            per_address_limit,
            state: Mutex::new(RateState::default()),
            window,
        }
    }

    pub fn check(&self, address: IpAddr) -> Result<(), PreAuthRateLimit> {
        self.check_at(address, Instant::now())
    }

    fn check_at(&self, address: IpAddr, now: Instant) -> Result<(), PreAuthRateLimit> {
        if self.global_limit == ResourceLimit::Unlimited
            && self.per_address_limit == ResourceLimit::Unlimited
        {
            return Ok(());
        }
        let mut state = self.lock();
        prune_expired(&mut state, now, self.window);
        if !limit_permits_next(self.global_limit, state.requests.len()) {
            return Err(PreAuthRateLimit::Global);
        }
        let address_requests = state.by_address.get(&address).copied().unwrap_or_default();
        if !limit_permits_next(self.per_address_limit, address_requests) {
            return Err(PreAuthRateLimit::Address);
        }

        state.requests.push_back(RateRecord {
            address,
            received_at: now,
        });
        *state.by_address.entry(address).or_default() = address_requests + 1;
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, RateState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn prune_expired(state: &mut RateState, now: Instant, window: Duration) {
    while state
        .requests
        .front()
        .is_some_and(|record| now.saturating_duration_since(record.received_at) >= window)
    {
        let record = state
            .requests
            .pop_front()
            .expect("front rate record must remain available");
        let remove = if let Some(count) = state.by_address.get_mut(&record.address) {
            *count = count.saturating_sub(1);
            *count == 0
        } else {
            false
        };
        if remove {
            state.by_address.remove(&record.address);
        }
    }
}

fn limit_permits_next(limit: ResourceLimit, current: usize) -> bool {
    current
        .checked_add(1)
        .is_some_and(|next| limit.permits(u64::try_from(next).unwrap_or(u64::MAX)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_permits_release_capacity_exactly_once() {
        let gate = Arc::new(ConnectionGate::new(ResourceLimit::limited(1)));
        let permit = gate.try_acquire().expect("first connection should acquire");
        assert_eq!(gate.active(), 1);
        assert!(gate.try_acquire().is_none());
        drop(permit);
        assert_eq!(gate.active(), 0);
        assert!(gate.try_acquire().is_some());
    }

    #[test]
    fn unlimited_connection_gate_does_not_apply_a_hidden_cap() {
        let gate = Arc::new(ConnectionGate::new(ResourceLimit::Unlimited));
        let permits = (0..1_024)
            .map(|_| gate.try_acquire().expect("unlimited gate should acquire"))
            .collect::<Vec<_>>();
        assert_eq!(gate.active(), permits.len());
    }

    #[test]
    fn rate_limits_global_and_per_address_windows_independently() {
        let start = Instant::now();
        let limiter = PreAuthRateLimiter::new(
            ResourceLimit::limited(3),
            ResourceLimit::limited(2),
            Duration::from_secs(60),
        );
        let first = "192.0.2.1".parse().unwrap();
        let second = "192.0.2.2".parse().unwrap();

        assert_eq!(limiter.check_at(first, start), Ok(()));
        assert_eq!(limiter.check_at(first, start), Ok(()));
        assert_eq!(
            limiter.check_at(first, start),
            Err(PreAuthRateLimit::Address)
        );
        assert_eq!(limiter.check_at(second, start), Ok(()));
        assert_eq!(
            limiter.check_at(second, start),
            Err(PreAuthRateLimit::Global)
        );
        assert_eq!(
            limiter.check_at(first, start + Duration::from_secs(60)),
            Ok(())
        );
    }

    #[test]
    fn fully_unlimited_rate_limiter_keeps_no_request_history() {
        let limiter = PreAuthRateLimiter::new(
            ResourceLimit::Unlimited,
            ResourceLimit::Unlimited,
            Duration::from_secs(60),
        );
        let address = "192.0.2.1".parse().unwrap();

        for _ in 0..1_024 {
            assert_eq!(limiter.check_at(address, Instant::now()), Ok(()));
        }

        let state = limiter.lock();
        assert!(state.requests.is_empty());
        assert!(state.by_address.is_empty());
    }
}
