use std::sync::{
    atomic::{AtomicUsize, Ordering},
    OnceLock,
};

use tokio::sync::Notify;

pub(in crate::commands::local_api::market_ops) struct SyncJobLimiter {
    active: AtomicUsize,
    notify: Notify,
}

pub(in crate::commands::local_api::market_ops) struct SyncJobPermit {
    limiter: &'static SyncJobLimiter,
}

impl SyncJobLimiter {
    fn new() -> Self {
        Self {
            active: AtomicUsize::new(0),
            notify: Notify::new(),
        }
    }

    pub(in crate::commands::local_api::market_ops) async fn acquire(
        &'static self,
        limit: usize,
    ) -> SyncJobPermit {
        let limit = limit.clamp(1, 16);
        loop {
            let notified = self.notify.notified();
            let active = self.active.load(Ordering::Acquire);
            if active < limit
                && self
                    .active
                    .compare_exchange(active, active + 1, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            {
                return SyncJobPermit { limiter: self };
            }
            notified.await;
        }
    }

    pub(in crate::commands::local_api::market_ops) fn active(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }

    pub(in crate::commands::local_api::market_ops) fn notify_limit_change(&self) {
        self.notify.notify_waiters();
    }
}

impl Drop for SyncJobPermit {
    fn drop(&mut self) {
        self.limiter.active.fetch_sub(1, Ordering::AcqRel);
        self.limiter.notify.notify_waiters();
    }
}

pub(in crate::commands::local_api::market_ops) fn sync_job_limiter() -> &'static SyncJobLimiter {
    static LIMITER: OnceLock<SyncJobLimiter> = OnceLock::new();
    LIMITER.get_or_init(SyncJobLimiter::new)
}
