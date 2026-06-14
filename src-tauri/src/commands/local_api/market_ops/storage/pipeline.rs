use sqlx::SqlitePool;
use tokio::task::JoinHandle;

use crate::okx::OkxCandle;

use self::writer::{save_base_candle_batch, BaseCandleBatchSave};
use super::super::*;

mod progress;
mod writer;

pub(in crate::commands::local_api::market_ops) use self::writer::{
    save_candles, CandleSaveProgress, CandleSaveScope,
};

pub(in crate::commands::local_api::market_ops) struct BaseCandleSavePipelineConfig {
    pub(in crate::commands::local_api::market_ops) pool: SqlitePool,
    pub(in crate::commands::local_api::market_ops) inst_id: String,
    pub(in crate::commands::local_api::market_ops) inst_type: String,
    pub(in crate::commands::local_api::market_ops) timeframe: String,
    pub(in crate::commands::local_api::market_ops) target_fetch_count: i64,
    pub(in crate::commands::local_api::market_ops) target_save_count: i64,
    pub(in crate::commands::local_api::market_ops) target_batches: i64,
    pub(in crate::commands::local_api::market_ops) progress: SyncProgressReporter,
    pub(in crate::commands::local_api::market_ops) cancel_guard: Option<SyncCancelGuard>,
}

pub(in crate::commands::local_api::market_ops) struct BaseCandleSavePipeline {
    pool: SqlitePool,
    inst_id: String,
    inst_type: String,
    timeframe: String,
    target_fetch_count: i64,
    target_save_count: i64,
    target_batches: i64,
    progress: SyncProgressReporter,
    cancel_guard: Option<SyncCancelGuard>,
    pending: Option<JoinHandle<AppResult<i64>>>,
    saved_count: i64,
}

impl BaseCandleSavePipeline {
    pub(in crate::commands::local_api::market_ops) fn new(
        config: BaseCandleSavePipelineConfig,
    ) -> Self {
        Self {
            pool: config.pool,
            inst_id: config.inst_id,
            inst_type: config.inst_type,
            timeframe: config.timeframe,
            target_fetch_count: config.target_fetch_count,
            target_save_count: config.target_save_count,
            target_batches: config.target_batches,
            progress: config.progress,
            cancel_guard: config.cancel_guard,
            pending: None,
            saved_count: 0,
        }
    }

    pub(in crate::commands::local_api::market_ops) fn saved_count(&self) -> i64 {
        self.saved_count
    }

    pub(in crate::commands::local_api::market_ops) async fn submit(
        &mut self,
        candles: Vec<OkxCandle>,
        fetched_count: i64,
        batches: i64,
        api_calls: i64,
        fetch_message: String,
        fetch_progress: i64,
    ) -> AppResult<()> {
        self.await_pending().await?;
        if candles.is_empty() {
            return Ok(());
        }

        let pool = self.pool.clone();
        let inst_id = self.inst_id.clone();
        let inst_type = self.inst_type.clone();
        let timeframe = self.timeframe.clone();
        let target_fetch_count = self.target_fetch_count.max(fetched_count);
        let target_save_count = self.target_save_count.max(fetched_count);
        let target_batches = self.target_batches.max(batches);
        let progress = self.progress.clone();
        let cancel_guard = self.cancel_guard.clone();
        let saved_offset = self.saved_count;
        self.pending = Some(tokio::spawn(async move {
            save_base_candle_batch(BaseCandleBatchSave {
                pool,
                inst_id,
                inst_type,
                timeframe,
                candles,
                cancel_guard,
                progress,
                progress_counts: CandleSaveProgress {
                    fetched_count,
                    target_fetch_count,
                    target_save_count,
                    batches,
                    target_batches,
                    api_calls,
                    saved_offset,
                    derived_offset: 0,
                    target_derive_count: 0,
                    derived: false,
                },
                fetch_message,
                fetch_progress,
            })
            .await
        }));
        Ok(())
    }

    pub(in crate::commands::local_api::market_ops) async fn finish(mut self) -> AppResult<i64> {
        self.await_pending().await?;
        Ok(self.saved_count)
    }

    async fn await_pending(&mut self) -> AppResult<()> {
        let Some(pending) = self.pending.take() else {
            return Ok(());
        };
        let saved = pending
            .await
            .map_err(|error| AppError::Runtime(format!("基础 K 线落库任务异常: {error}")))??;
        self.saved_count += saved;
        Ok(())
    }
}
