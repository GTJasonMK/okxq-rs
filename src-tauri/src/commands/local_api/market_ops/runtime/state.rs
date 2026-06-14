use std::sync::{
    atomic::{AtomicI64, AtomicUsize, Ordering},
    OnceLock,
};

use super::super::*;
use super::settings::SyncRuntimeSettings;

pub(super) const SYNC_RUNTIME_SETTINGS_KEY: &str = "sync_runtime_settings";

struct AtomicSyncRuntimeSettings {
    max_sync_batches: AtomicUsize,
    okx_page_pause_ms: AtomicUsize,
    sync_job_concurrency: AtomicUsize,
    window_fetch_concurrency: AtomicUsize,
    window_fetch_batches_per_slice: AtomicI64,
    candle_upsert_transaction_chunk: AtomicUsize,
    okx_max_concurrency: AtomicUsize,
    okx_public_rest_concurrency: AtomicUsize,
    okx_private_rest_concurrency: AtomicUsize,
    okx_trade_rest_concurrency: AtomicUsize,
    okx_ws_control_concurrency: AtomicUsize,
    okx_unknown_concurrency: AtomicUsize,
}

impl AtomicSyncRuntimeSettings {
    fn new(settings: SyncRuntimeSettings) -> Self {
        Self {
            max_sync_batches: AtomicUsize::new(settings.max_sync_batches),
            okx_page_pause_ms: AtomicUsize::new(settings.okx_page_pause_ms as usize),
            sync_job_concurrency: AtomicUsize::new(settings.sync_job_concurrency),
            window_fetch_concurrency: AtomicUsize::new(settings.window_fetch_concurrency),
            window_fetch_batches_per_slice: AtomicI64::new(settings.window_fetch_batches_per_slice),
            candle_upsert_transaction_chunk: AtomicUsize::new(
                settings.candle_upsert_transaction_chunk,
            ),
            okx_max_concurrency: AtomicUsize::new(settings.okx_max_concurrency),
            okx_public_rest_concurrency: AtomicUsize::new(settings.okx_public_rest_concurrency),
            okx_private_rest_concurrency: AtomicUsize::new(settings.okx_private_rest_concurrency),
            okx_trade_rest_concurrency: AtomicUsize::new(settings.okx_trade_rest_concurrency),
            okx_ws_control_concurrency: AtomicUsize::new(settings.okx_ws_control_concurrency),
            okx_unknown_concurrency: AtomicUsize::new(settings.okx_unknown_concurrency),
        }
    }

    fn load(&self) -> SyncRuntimeSettings {
        SyncRuntimeSettings {
            max_sync_batches: self.max_sync_batches.load(Ordering::Relaxed),
            okx_page_pause_ms: self.okx_page_pause_ms.load(Ordering::Relaxed) as u64,
            sync_job_concurrency: self.sync_job_concurrency.load(Ordering::Relaxed),
            window_fetch_concurrency: self.window_fetch_concurrency.load(Ordering::Relaxed),
            window_fetch_batches_per_slice: self
                .window_fetch_batches_per_slice
                .load(Ordering::Relaxed),
            candle_upsert_transaction_chunk: self
                .candle_upsert_transaction_chunk
                .load(Ordering::Relaxed),
            okx_max_concurrency: self.okx_max_concurrency.load(Ordering::Relaxed),
            okx_public_rest_concurrency: self.okx_public_rest_concurrency.load(Ordering::Relaxed),
            okx_private_rest_concurrency: self.okx_private_rest_concurrency.load(Ordering::Relaxed),
            okx_trade_rest_concurrency: self.okx_trade_rest_concurrency.load(Ordering::Relaxed),
            okx_ws_control_concurrency: self.okx_ws_control_concurrency.load(Ordering::Relaxed),
            okx_unknown_concurrency: self.okx_unknown_concurrency.load(Ordering::Relaxed),
        }
        .normalized()
    }

    fn store(&self, settings: &SyncRuntimeSettings) {
        let settings = settings.clone().normalized();
        self.max_sync_batches
            .store(settings.max_sync_batches, Ordering::Relaxed);
        self.okx_page_pause_ms
            .store(settings.okx_page_pause_ms as usize, Ordering::Relaxed);
        self.sync_job_concurrency
            .store(settings.sync_job_concurrency, Ordering::Relaxed);
        self.window_fetch_concurrency
            .store(settings.window_fetch_concurrency, Ordering::Relaxed);
        self.window_fetch_batches_per_slice
            .store(settings.window_fetch_batches_per_slice, Ordering::Relaxed);
        self.candle_upsert_transaction_chunk
            .store(settings.candle_upsert_transaction_chunk, Ordering::Relaxed);
        self.okx_max_concurrency
            .store(settings.okx_max_concurrency, Ordering::Relaxed);
        self.okx_public_rest_concurrency
            .store(settings.okx_public_rest_concurrency, Ordering::Relaxed);
        self.okx_private_rest_concurrency
            .store(settings.okx_private_rest_concurrency, Ordering::Relaxed);
        self.okx_trade_rest_concurrency
            .store(settings.okx_trade_rest_concurrency, Ordering::Relaxed);
        self.okx_ws_control_concurrency
            .store(settings.okx_ws_control_concurrency, Ordering::Relaxed);
        self.okx_unknown_concurrency
            .store(settings.okx_unknown_concurrency, Ordering::Relaxed);
    }
}

fn sync_runtime_store() -> &'static AtomicSyncRuntimeSettings {
    static SETTINGS: OnceLock<AtomicSyncRuntimeSettings> = OnceLock::new();
    SETTINGS.get_or_init(|| AtomicSyncRuntimeSettings::new(SyncRuntimeSettings::default()))
}

pub(in crate::commands::local_api::market_ops) fn sync_runtime_settings() -> SyncRuntimeSettings {
    sync_runtime_store().load()
}

pub(super) fn store_sync_runtime_settings(settings: &SyncRuntimeSettings) {
    sync_runtime_store().store(settings)
}

pub(in crate::commands::local_api::market_ops) async fn load_sync_runtime_settings(
    state: &AppState,
) -> AppResult<SyncRuntimeSettings> {
    let settings = state
        .preferences
        .get(SYNC_RUNTIME_SETTINGS_KEY)
        .await?
        .map(SyncRuntimeSettings::from_value)
        .unwrap_or_else(SyncRuntimeSettings::default)
        .normalized();
    store_sync_runtime_settings(&settings);
    state
        .token_bucket
        .apply_concurrency_settings(&settings.okx_concurrency_settings());
    Ok(settings)
}

pub(in crate::commands::local_api::market_ops) fn max_sync_batches() -> usize {
    sync_runtime_settings().max_sync_batches
}

pub(in crate::commands::local_api::market_ops) fn okx_page_pause_ms() -> u64 {
    sync_runtime_settings().okx_page_pause_ms
}

pub(in crate::commands::local_api::market_ops) fn window_fetch_concurrency() -> usize {
    sync_runtime_settings().window_fetch_concurrency
}

pub(in crate::commands::local_api::market_ops) fn window_fetch_batches_per_slice() -> i64 {
    sync_runtime_settings().window_fetch_batches_per_slice
}

pub(in crate::commands::local_api::market_ops) fn candle_upsert_transaction_chunk() -> usize {
    sync_runtime_settings().candle_upsert_transaction_chunk
}
