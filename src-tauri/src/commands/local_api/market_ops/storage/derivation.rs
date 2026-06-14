use sqlx::SqlitePool;

use self::{
    aggregate::aggregate_candles, estimate::estimate_total_derived_count, load::load_base_candles,
    load::load_base_candles_range,
};
use super::super::*;
use super::pipeline::{save_candles, CandleSaveProgress, CandleSaveScope};
use super::records::{update_sync_record, update_sync_record_metadata};

mod aggregate;
mod estimate;
mod load;

#[derive(Clone, Copy)]
pub(in crate::commands::local_api::market_ops) struct DeriveCandleContext<'a> {
    pub(in crate::commands::local_api::market_ops) pool: &'a SqlitePool,
    pub(in crate::commands::local_api::market_ops) inst_id: &'a str,
    pub(in crate::commands::local_api::market_ops) inst_type: &'a str,
    pub(in crate::commands::local_api::market_ops) cancel_guard: Option<&'a SyncCancelGuard>,
    pub(in crate::commands::local_api::market_ops) progress: &'a SyncProgressReporter,
}

pub(in crate::commands::local_api::market_ops) struct DeriveCandleRequest<'a> {
    pub(in crate::commands::local_api::market_ops) source_timeframe: &'a str,
    pub(in crate::commands::local_api::market_ops) target_timeframes: &'a [String],
    pub(in crate::commands::local_api::market_ops) history_complete: bool,
    pub(in crate::commands::local_api::market_ops) last_sync_mode: Option<&'a str>,
    pub(in crate::commands::local_api::market_ops) days: i64,
}

pub(in crate::commands::local_api::market_ops) struct DeriveCandleRangeRequest<'a> {
    pub(in crate::commands::local_api::market_ops) source_timeframe: &'a str,
    pub(in crate::commands::local_api::market_ops) target_timeframes: &'a [String],
    pub(in crate::commands::local_api::market_ops) start_ts: i64,
    pub(in crate::commands::local_api::market_ops) end_ts: i64,
    pub(in crate::commands::local_api::market_ops) last_sync_mode: Option<&'a str>,
}

#[derive(Clone, Copy)]
pub(in crate::commands::local_api::market_ops) struct DeriveCandleProgress {
    pub(in crate::commands::local_api::market_ops) fetched_count: i64,
    pub(in crate::commands::local_api::market_ops) target_fetch_count: i64,
    pub(in crate::commands::local_api::market_ops) target_save_count: i64,
    pub(in crate::commands::local_api::market_ops) base_saved_count: i64,
    pub(in crate::commands::local_api::market_ops) batches: i64,
    pub(in crate::commands::local_api::market_ops) target_batches: i64,
    pub(in crate::commands::local_api::market_ops) api_calls: i64,
}

impl DeriveCandleProgress {
    fn update(
        self,
        progress: i64,
        message: String,
        derived_count: i64,
        target_derive_count: i64,
    ) -> SyncProgressUpdate {
        SyncProgressUpdate {
            progress,
            message,
            fetched_count: self.fetched_count,
            target_fetch_count: self.target_fetch_count,
            saved_count: self.base_saved_count,
            target_save_count: self.target_save_count,
            inserted_count: self.base_saved_count + derived_count,
            derived_count,
            target_derive_count,
            batches: self.batches,
            target_batches: self.target_batches,
            api_calls: self.api_calls,
        }
    }

    fn save_progress(self, derived_offset: i64, target_derive_count: i64) -> CandleSaveProgress {
        CandleSaveProgress {
            fetched_count: self.fetched_count,
            target_fetch_count: self.target_fetch_count,
            target_save_count: self.target_save_count,
            batches: self.batches,
            target_batches: self.target_batches,
            api_calls: self.api_calls,
            saved_offset: self.base_saved_count,
            derived_offset,
            target_derive_count,
            derived: true,
        }
    }
}

pub(in crate::commands::local_api::market_ops) async fn derive_candles_from_base(
    context: DeriveCandleContext<'_>,
    request: DeriveCandleRequest<'_>,
    progress_counts: DeriveCandleProgress,
) -> AppResult<DeriveResult> {
    let source_timeframe = normalize_timeframe_name(request.source_timeframe);
    let source_timeframe = if source_timeframe.is_empty() {
        BASE_CANDLE_TIMEFRAME.to_string()
    } else {
        source_timeframe
    };
    let target_timeframes = normalize_target_timeframes(request.target_timeframes.to_vec());
    if target_timeframes.is_empty() {
        return Ok(DeriveResult::default());
    }

    let mut result = DeriveResult::default();
    let derived_targets = target_timeframes
        .iter()
        .filter(|timeframe| timeframe.as_str() != source_timeframe)
        .cloned()
        .collect::<Vec<_>>();
    if derived_targets.is_empty() {
        update_sync_record(
            context.pool,
            context.inst_id,
            context.inst_type,
            &source_timeframe,
            Some(request.history_complete),
            request.last_sync_mode,
        )
        .await?;
        return Ok(result);
    }

    let base_min_timestamp = derive_base_min_timestamp(
        request.last_sync_mode,
        request.days,
        &source_timeframe,
        &target_timeframes,
    );
    let base_candles = load_base_candles(
        context.pool,
        context.inst_id,
        context.inst_type,
        &source_timeframe,
        base_min_timestamp,
        context.cancel_guard,
    )
    .await?;
    if base_candles.is_empty() {
        return Ok(result);
    }
    let derived_history_complete = request.history_complete && base_min_timestamp.is_none();

    let target_derive_count =
        estimate_total_derived_count(&base_candles, &source_timeframe, &target_timeframes);
    result.target_count = target_derive_count;
    let target_count = target_timeframes.len().max(1) as i64;
    for timeframe in target_timeframes {
        check_sync_cancel(context.cancel_guard).await?;
        if timeframe == source_timeframe {
            update_sync_record(
                context.pool,
                context.inst_id,
                context.inst_type,
                &timeframe,
                Some(derived_history_complete),
                request.last_sync_mode,
            )
            .await?;
            continue;
        }
        if !DERIVED_CANDLE_TIMEFRAMES.contains(&timeframe.as_str()) {
            tracing::warn!(
                inst_id = context.inst_id,
                inst_type = context.inst_type,
                timeframe,
                "skip unsupported derived candle timeframe"
            );
            continue;
        }
        context
            .progress
            .report(progress_counts.update(
                88,
                format!("准备从 {} 对齐 {}", source_timeframe, timeframe),
                result.saved_count,
                target_derive_count,
            ))
            .await?;
        let candles = aggregate_candles(&base_candles, &timeframe, context.cancel_guard).await?;
        if candles.is_empty() {
            continue;
        }
        let saved = save_candles(
            CandleSaveScope {
                pool: context.pool,
                inst_id: context.inst_id,
                inst_type: context.inst_type,
                timeframe: &timeframe,
                cancel_guard: context.cancel_guard,
                progress: context.progress,
            },
            &candles,
            progress_counts.save_progress(result.saved_count, target_derive_count),
        )
        .await?;
        update_sync_record_metadata(
            context.pool,
            context.inst_id,
            context.inst_type,
            &timeframe,
            Some(request.history_complete),
            request.last_sync_mode,
        )
        .await?;
        result.saved_count += saved;
        result.derived_timeframes.push(timeframe);
        context
            .progress
            .report(progress_counts.update(
                (90 + ((result.derived_timeframes.len() as i64 * 8) / target_count)).min(98),
                format!(
                    "已对齐 {}，累计对齐 {} 条",
                    result.derived_timeframes.join("/"),
                    result.saved_count
                ),
                result.saved_count,
                target_derive_count,
            ))
            .await?;
    }

    Ok(result)
}

pub(in crate::commands::local_api::market_ops) async fn derive_candles_from_base_range(
    context: DeriveCandleContext<'_>,
    request: DeriveCandleRangeRequest<'_>,
    progress_counts: DeriveCandleProgress,
) -> AppResult<DeriveResult> {
    let source_timeframe = normalize_timeframe_name(request.source_timeframe);
    let source_timeframe = if source_timeframe.is_empty() {
        BASE_CANDLE_TIMEFRAME.to_string()
    } else {
        source_timeframe
    };
    let target_timeframes = normalize_target_timeframes(request.target_timeframes.to_vec());
    if target_timeframes.is_empty() {
        return Ok(DeriveResult::default());
    }

    let max_target_ms = target_timeframes
        .iter()
        .map(|timeframe| timeframe_to_ms(timeframe).max(1))
        .max()
        .unwrap_or_else(|| timeframe_to_ms(&source_timeframe).max(1));
    let source_ms = timeframe_to_ms(&source_timeframe).max(1);
    let load_start = request
        .start_ts
        .saturating_sub(request.start_ts.rem_euclid(max_target_ms));
    let load_end = request
        .end_ts
        .saturating_sub(request.end_ts.rem_euclid(max_target_ms))
        .saturating_add(max_target_ms)
        .saturating_sub(source_ms);
    let base_candles = load_base_candles_range(
        context.pool,
        context.inst_id,
        context.inst_type,
        &source_timeframe,
        load_start,
        load_end,
        context.cancel_guard,
    )
    .await?;
    if base_candles.is_empty() {
        return Ok(DeriveResult::default());
    }

    let target_derive_count =
        estimate_total_derived_count(&base_candles, &source_timeframe, &target_timeframes);
    let target_count = target_timeframes.len().max(1) as i64;
    let mut result = DeriveResult {
        target_count: target_derive_count,
        ..Default::default()
    };
    for timeframe in target_timeframes {
        check_sync_cancel(context.cancel_guard).await?;
        if timeframe == source_timeframe {
            update_sync_record(
                context.pool,
                context.inst_id,
                context.inst_type,
                &timeframe,
                None,
                request.last_sync_mode,
            )
            .await?;
            continue;
        }
        if !DERIVED_CANDLE_TIMEFRAMES.contains(&timeframe.as_str()) {
            tracing::warn!(
                inst_id = context.inst_id,
                inst_type = context.inst_type,
                timeframe,
                "skip unsupported derived candle timeframe in range repair"
            );
            continue;
        }
        context
            .progress
            .report(progress_counts.update(
                88,
                format!("补齐后从 {} 对齐 {}", source_timeframe, timeframe),
                result.saved_count,
                target_derive_count,
            ))
            .await?;
        let mut candles =
            aggregate_candles(&base_candles, &timeframe, context.cancel_guard).await?;
        candles.retain(|candle| {
            candle.timestamp >= request.start_ts && candle.timestamp <= request.end_ts
        });
        if candles.is_empty() {
            continue;
        }
        let saved = save_candles(
            CandleSaveScope {
                pool: context.pool,
                inst_id: context.inst_id,
                inst_type: context.inst_type,
                timeframe: &timeframe,
                cancel_guard: context.cancel_guard,
                progress: context.progress,
            },
            &candles,
            progress_counts.save_progress(result.saved_count, target_derive_count),
        )
        .await?;
        update_sync_record_metadata(
            context.pool,
            context.inst_id,
            context.inst_type,
            &timeframe,
            None,
            request.last_sync_mode,
        )
        .await?;
        result.saved_count += saved;
        result.derived_timeframes.push(timeframe);
        context
            .progress
            .report(progress_counts.update(
                (90 + ((result.derived_timeframes.len() as i64 * 8) / target_count)).min(98),
                format!(
                    "范围对齐完成 {}，累计实际写入 {} 条",
                    result.derived_timeframes.join("/"),
                    result.saved_count
                ),
                result.saved_count,
                target_derive_count,
            ))
            .await?;
    }
    Ok(result)
}

fn derive_base_min_timestamp(
    mode: Option<&str>,
    days: i64,
    source_timeframe: &str,
    target_timeframes: &[String],
) -> Option<i64> {
    if mode.is_some_and(|value| value.eq_ignore_ascii_case("full")) {
        return None;
    }
    let source_ms = timeframe_to_ms(source_timeframe).max(0);
    let max_target_ms = target_timeframes
        .iter()
        .map(|timeframe| timeframe_to_ms(timeframe).max(0))
        .max()
        .unwrap_or(source_ms)
        .max(source_ms);
    let buffer_ms = max_target_ms.saturating_mul(2);
    Some(
        chrono::Utc::now()
            .timestamp_millis()
            .saturating_sub(days.max(1).saturating_mul(DAY_MS))
            .saturating_sub(buffer_ms),
    )
}
