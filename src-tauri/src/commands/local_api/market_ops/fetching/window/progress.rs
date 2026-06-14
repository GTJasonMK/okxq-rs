use std::sync::atomic::{AtomicI64, Ordering};

#[derive(Debug)]
pub(super) struct WindowFetchProgress {
    pub(super) target_fetch_count: i64,
    pub(super) target_batches: i64,
    fetched_count: AtomicI64,
    batches: AtomicI64,
    api_calls: AtomicI64,
}

impl WindowFetchProgress {
    pub(super) fn new(target_fetch_count: i64, target_batches: i64) -> Self {
        Self {
            target_fetch_count,
            target_batches,
            fetched_count: AtomicI64::new(0),
            batches: AtomicI64::new(0),
            api_calls: AtomicI64::new(0),
        }
    }

    pub(super) fn record_batch(&self, fetched_delta: i64) -> (i64, i64, i64) {
        let fetched = self
            .fetched_count
            .fetch_add(fetched_delta.max(0), Ordering::Relaxed)
            + fetched_delta.max(0);
        let batches = self.batches.fetch_add(1, Ordering::Relaxed) + 1;
        let api_calls = self.api_calls.fetch_add(1, Ordering::Relaxed) + 1;
        (fetched, batches, api_calls)
    }
}

pub(super) fn window_fetch_progress_percent(fetched_count: i64, target_fetch_count: i64) -> i64 {
    if target_fetch_count <= 0 {
        return 20;
    }
    (5 + ((fetched_count.max(0) * 62) / target_fetch_count.max(1))).clamp(5, 67)
}
