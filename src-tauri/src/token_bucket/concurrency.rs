use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use tokio::sync::Notify;

use super::{settings::OKXConcurrencySettings, GLOBAL_CONCURRENCY_KEY, UNKNOWN_CONCURRENCY_KEY};

#[derive(Clone)]
pub(super) struct ConcurrencyLimiter {
    capacity: Arc<AtomicUsize>,
    active: Arc<AtomicUsize>,
    waiting: Arc<AtomicUsize>,
    notify: Arc<Notify>,
}

pub(super) struct ConcurrencyPermit {
    limiter: ConcurrencyLimiter,
}

struct WaitingSlot {
    waiting: Arc<AtomicUsize>,
}

impl WaitingSlot {
    fn new(waiting: Arc<AtomicUsize>) -> Self {
        waiting.fetch_add(1, Ordering::SeqCst);
        Self { waiting }
    }
}

impl ConcurrencyLimiter {
    fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            capacity: Arc::new(AtomicUsize::new(capacity)),
            active: Arc::new(AtomicUsize::new(0)),
            waiting: Arc::new(AtomicUsize::new(0)),
            notify: Arc::new(Notify::new()),
        }
    }

    pub(super) async fn acquire(&self) -> ConcurrencyPermit {
        loop {
            let notified = self.notify.notified();
            let capacity = self.capacity();
            let active = self.active.load(Ordering::Acquire);
            if active < capacity {
                match self.active.compare_exchange(
                    active,
                    active + 1,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        return ConcurrencyPermit {
                            limiter: self.clone(),
                        };
                    }
                    Err(_) => continue,
                }
            }

            let waiting_slot = WaitingSlot::new(self.waiting.clone());
            notified.await;
            drop(waiting_slot);
        }
    }

    pub(super) fn set_capacity(&self, capacity: usize) {
        self.capacity.store(capacity.max(1), Ordering::Release);
        self.notify.notify_waiters();
    }

    pub(super) fn capacity(&self) -> usize {
        self.capacity.load(Ordering::Acquire).max(1)
    }

    pub(super) fn in_flight(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }

    pub(super) fn available(&self) -> usize {
        self.capacity().saturating_sub(self.in_flight())
    }

    pub(super) fn waiting(&self) -> usize {
        self.waiting.load(Ordering::SeqCst)
    }
}

impl Drop for ConcurrencyPermit {
    fn drop(&mut self) {
        self.limiter.active.fetch_sub(1, Ordering::AcqRel);
        self.limiter.notify.notify_waiters();
    }
}

impl Drop for WaitingSlot {
    fn drop(&mut self) {
        self.waiting.fetch_sub(1, Ordering::SeqCst);
    }
}

pub(super) fn default_concurrency_limiters() -> HashMap<String, ConcurrencyLimiter> {
    let settings = OKXConcurrencySettings::default();
    [
        (
            GLOBAL_CONCURRENCY_KEY.to_string(),
            settings.okx_max_concurrency,
        ),
        (
            "rest:public".to_string(),
            settings.okx_public_rest_concurrency,
        ),
        (
            "rest:private".to_string(),
            settings.okx_private_rest_concurrency,
        ),
        (
            "rest:trade".to_string(),
            settings.okx_trade_rest_concurrency,
        ),
        (
            "ws:ws_control".to_string(),
            settings.okx_ws_control_concurrency,
        ),
        (
            UNKNOWN_CONCURRENCY_KEY.to_string(),
            settings.okx_unknown_concurrency,
        ),
    ]
    .into_iter()
    .map(|(key, capacity)| (key, ConcurrencyLimiter::new(capacity)))
    .collect()
}
