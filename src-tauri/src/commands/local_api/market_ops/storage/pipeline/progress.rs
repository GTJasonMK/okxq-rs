use super::super::super::{AppResult, SyncProgressReporter, SyncProgressUpdate};

pub(super) struct BatchSaveProgress<'a> {
    pub(super) progress: &'a SyncProgressReporter,
    pub(super) fetched_count: i64,
    pub(super) target_fetch_count: i64,
    pub(super) target_save_count: i64,
    pub(super) batches: i64,
    pub(super) target_batches: i64,
    pub(super) api_calls: i64,
    pub(super) saved_offset: i64,
    pub(super) derived_offset: i64,
    pub(super) target_derive_count: i64,
    pub(super) derived: bool,
    pub(super) base_progress: i64,
    pub(super) span: i64,
}

impl BatchSaveProgress<'_> {
    pub(super) async fn report_chunk_saved(
        &self,
        timeframe: &str,
        saved_count: i64,
        processed_count: i64,
        total_candles: usize,
    ) -> AppResult<()> {
        let total = total_candles.max(1) as i64;
        let processed_count = processed_count.clamp(0, total);
        let saved_count = saved_count.max(0);
        let unchanged_count = processed_count.saturating_sub(saved_count);
        let absolute_saved = self.saved_offset + saved_count;
        let absolute_derived = if self.derived {
            self.derived_offset + saved_count
        } else {
            self.derived_offset
        };
        self.progress
            .report(SyncProgressUpdate {
                progress: (self.base_progress + ((processed_count * self.span) / total)).min(98),
                message: format!(
                    "落库 {} 中：已处理 {} / {} 条{}，新增/更新 {} 条，重复未变更 {} 条",
                    timeframe,
                    processed_count,
                    total_candles,
                    if self.derived {
                        "对齐 K 线"
                    } else {
                        "基础 K 线"
                    },
                    saved_count,
                    unchanged_count
                ),
                fetched_count: self.fetched_count,
                target_fetch_count: self.target_fetch_count,
                saved_count: if self.derived {
                    self.saved_offset
                } else {
                    absolute_saved
                },
                target_save_count: self.target_save_count,
                inserted_count: self.saved_offset + saved_count,
                derived_count: absolute_derived,
                target_derive_count: self.target_derive_count,
                batches: self.batches,
                target_batches: self.target_batches,
                api_calls: self.api_calls,
            })
            .await
    }
}
