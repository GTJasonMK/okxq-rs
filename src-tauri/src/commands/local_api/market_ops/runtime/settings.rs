use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::token_bucket::OKXConcurrencySettings;

use super::values::{env_i64, env_u64, env_usize, json_i64, json_u64, json_usize};

const DEFAULT_MAX_SYNC_BATCHES: usize = 2_000;
const DEFAULT_OKX_PAGE_PAUSE_MS: u64 = 0;
const DEFAULT_SYNC_JOB_CONCURRENCY: usize = 2;
const DEFAULT_WINDOW_FETCH_CONCURRENCY: usize = 8;
const DEFAULT_WINDOW_FETCH_BATCHES_PER_SLICE: i64 = 32;
const DEFAULT_CANDLE_UPSERT_TRANSACTION_CHUNK: usize = 1_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(in crate::commands::local_api::market_ops) struct SyncRuntimeSettings {
    pub(in crate::commands::local_api::market_ops) max_sync_batches: usize,
    pub(in crate::commands::local_api::market_ops) okx_page_pause_ms: u64,
    pub(in crate::commands::local_api::market_ops) sync_job_concurrency: usize,
    pub(in crate::commands::local_api::market_ops) window_fetch_concurrency: usize,
    pub(in crate::commands::local_api::market_ops) window_fetch_batches_per_slice: i64,
    pub(in crate::commands::local_api::market_ops) candle_upsert_transaction_chunk: usize,
    pub(in crate::commands::local_api::market_ops) okx_max_concurrency: usize,
    pub(in crate::commands::local_api::market_ops) okx_public_rest_concurrency: usize,
    pub(in crate::commands::local_api::market_ops) okx_private_rest_concurrency: usize,
    pub(in crate::commands::local_api::market_ops) okx_trade_rest_concurrency: usize,
    pub(in crate::commands::local_api::market_ops) okx_ws_control_concurrency: usize,
    pub(in crate::commands::local_api::market_ops) okx_unknown_concurrency: usize,
}

impl Default for SyncRuntimeSettings {
    fn default() -> Self {
        let okx_concurrency = OKXConcurrencySettings::default();
        Self {
            max_sync_batches: env_usize("OKXQ_SYNC_MAX_BATCHES", DEFAULT_MAX_SYNC_BATCHES),
            okx_page_pause_ms: env_u64("OKXQ_OKX_PAGE_PAUSE_MS", DEFAULT_OKX_PAGE_PAUSE_MS),
            sync_job_concurrency: env_usize(
                "OKXQ_SYNC_JOB_CONCURRENCY",
                DEFAULT_SYNC_JOB_CONCURRENCY,
            ),
            window_fetch_concurrency: env_usize(
                "OKXQ_WINDOW_FETCH_CONCURRENCY",
                DEFAULT_WINDOW_FETCH_CONCURRENCY,
            ),
            window_fetch_batches_per_slice: env_i64(
                "OKXQ_WINDOW_FETCH_BATCHES_PER_SLICE",
                DEFAULT_WINDOW_FETCH_BATCHES_PER_SLICE,
            ),
            candle_upsert_transaction_chunk: env_usize(
                "OKXQ_CANDLE_UPSERT_TRANSACTION_CHUNK",
                DEFAULT_CANDLE_UPSERT_TRANSACTION_CHUNK,
            ),
            okx_max_concurrency: okx_concurrency.okx_max_concurrency,
            okx_public_rest_concurrency: okx_concurrency.okx_public_rest_concurrency,
            okx_private_rest_concurrency: okx_concurrency.okx_private_rest_concurrency,
            okx_trade_rest_concurrency: okx_concurrency.okx_trade_rest_concurrency,
            okx_ws_control_concurrency: okx_concurrency.okx_ws_control_concurrency,
            okx_unknown_concurrency: okx_concurrency.okx_unknown_concurrency,
        }
        .normalized()
    }
}

impl SyncRuntimeSettings {
    pub(in crate::commands::local_api::market_ops) fn from_value(value: Value) -> Self {
        let mut settings = Self::default();
        let Some(object) = value.as_object() else {
            return settings;
        };

        if let Some(value) = object.get("max_sync_batches").and_then(json_usize) {
            settings.max_sync_batches = value;
        }
        if let Some(value) = object.get("okx_page_pause_ms").and_then(json_u64) {
            settings.okx_page_pause_ms = value;
        }
        if let Some(value) = object.get("sync_job_concurrency").and_then(json_usize) {
            settings.sync_job_concurrency = value;
        }
        if let Some(value) = object.get("window_fetch_concurrency").and_then(json_usize) {
            settings.window_fetch_concurrency = value;
        }
        if let Some(value) = object
            .get("window_fetch_batches_per_slice")
            .and_then(json_i64)
        {
            settings.window_fetch_batches_per_slice = value;
        }
        if let Some(value) = object
            .get("candle_upsert_transaction_chunk")
            .and_then(json_usize)
        {
            settings.candle_upsert_transaction_chunk = value;
        }
        if let Some(value) = object.get("okx_max_concurrency").and_then(json_usize) {
            settings.okx_max_concurrency = value;
        }
        if let Some(value) = object
            .get("okx_public_rest_concurrency")
            .and_then(json_usize)
        {
            settings.okx_public_rest_concurrency = value;
        }
        if let Some(value) = object
            .get("okx_private_rest_concurrency")
            .and_then(json_usize)
        {
            settings.okx_private_rest_concurrency = value;
        }
        if let Some(value) = object
            .get("okx_trade_rest_concurrency")
            .and_then(json_usize)
        {
            settings.okx_trade_rest_concurrency = value;
        }
        if let Some(value) = object
            .get("okx_ws_control_concurrency")
            .and_then(json_usize)
        {
            settings.okx_ws_control_concurrency = value;
        }
        if let Some(value) = object.get("okx_unknown_concurrency").and_then(json_usize) {
            settings.okx_unknown_concurrency = value;
        }

        settings.normalized()
    }

    pub(in crate::commands::local_api::market_ops) fn normalized(self) -> Self {
        let okx_concurrency = self.okx_concurrency_settings();
        Self {
            max_sync_batches: self.max_sync_batches.clamp(1, 20_000),
            okx_page_pause_ms: self.okx_page_pause_ms.min(5_000),
            sync_job_concurrency: self.sync_job_concurrency.clamp(1, 16),
            window_fetch_concurrency: self.window_fetch_concurrency.clamp(1, 32),
            window_fetch_batches_per_slice: self.window_fetch_batches_per_slice.clamp(1, 256),
            candle_upsert_transaction_chunk: self
                .candle_upsert_transaction_chunk
                .clamp(100, 10_000),
            okx_max_concurrency: okx_concurrency.okx_max_concurrency,
            okx_public_rest_concurrency: okx_concurrency.okx_public_rest_concurrency,
            okx_private_rest_concurrency: okx_concurrency.okx_private_rest_concurrency,
            okx_trade_rest_concurrency: okx_concurrency.okx_trade_rest_concurrency,
            okx_ws_control_concurrency: okx_concurrency.okx_ws_control_concurrency,
            okx_unknown_concurrency: okx_concurrency.okx_unknown_concurrency,
        }
    }

    pub(in crate::commands::local_api::market_ops) fn okx_concurrency_settings(
        &self,
    ) -> OKXConcurrencySettings {
        OKXConcurrencySettings {
            okx_max_concurrency: self.okx_max_concurrency,
            okx_public_rest_concurrency: self.okx_public_rest_concurrency,
            okx_private_rest_concurrency: self.okx_private_rest_concurrency,
            okx_trade_rest_concurrency: self.okx_trade_rest_concurrency,
            okx_ws_control_concurrency: self.okx_ws_control_concurrency,
            okx_unknown_concurrency: self.okx_unknown_concurrency,
        }
        .normalized()
    }

    pub(in crate::commands::local_api::market_ops) fn limits() -> Value {
        json!({
            "max_sync_batches": {"min": 1, "max": 20_000},
            "okx_page_pause_ms": {"min": 0, "max": 5_000},
            "sync_job_concurrency": {"min": 1, "max": 16},
            "window_fetch_concurrency": {"min": 1, "max": 32},
            "window_fetch_batches_per_slice": {"min": 1, "max": 256},
            "candle_upsert_transaction_chunk": {"min": 100, "max": 10_000},
            "okx_max_concurrency": {"min": 1, "max": 64},
            "okx_public_rest_concurrency": {"min": 1, "max": 64},
            "okx_private_rest_concurrency": {"min": 1, "max": 32},
            "okx_trade_rest_concurrency": {"min": 1, "max": 16},
            "okx_ws_control_concurrency": {"min": 1, "max": 8},
            "okx_unknown_concurrency": {"min": 1, "max": 16}
        })
    }
}
