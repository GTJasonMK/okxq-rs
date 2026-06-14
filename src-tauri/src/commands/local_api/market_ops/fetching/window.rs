use std::{collections::BTreeMap, sync::Arc};

use futures_util::stream::{FuturesUnordered, StreamExt};
use tokio::task::JoinHandle;

use self::{
    progress::{window_fetch_progress_percent, WindowFetchProgress},
    slices::{build_window_fetch_slices, estimated_window_slice_batches},
    worker::{fetch_window_slice, WindowFetchSliceResult},
};
use super::super::*;
use super::{CandleFetchContext, CandleFetchRange};

mod progress;
mod slices;
#[cfg(test)]
mod tests;
mod worker;

pub(in crate::commands::local_api::market_ops) async fn fetch_window_candles(
    context: CandleFetchContext<'_>,
    days: i64,
) -> AppResult<SyncFetchResult> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let start_ts = now_ms.saturating_sub(days.saturating_mul(DAY_MS));
    let timeframe_ms = timeframe_to_ms(context.timeframe).max(1);
    let end_ts = now_ms
        .saturating_sub(now_ms.rem_euclid(timeframe_ms))
        .saturating_sub(timeframe_ms);
    fetch_range_candles(
        context,
        CandleFetchRange {
            start_ts,
            end_ts,
            completion_message: format!("窗口同步完成，最近 {days} 天数据已写入本地库"),
        },
    )
    .await
}

pub(in crate::commands::local_api::market_ops) async fn fetch_range_candles(
    context: CandleFetchContext<'_>,
    range: CandleFetchRange,
) -> AppResult<SyncFetchResult> {
    let timeframe_ms = timeframe_to_ms(context.timeframe).max(1);
    let aligned_start = range
        .start_ts
        .saturating_sub(range.start_ts.rem_euclid(timeframe_ms));
    let aligned_end = range
        .end_ts
        .saturating_sub(range.end_ts.rem_euclid(timeframe_ms))
        .saturating_add(timeframe_ms);
    let range_ms = aligned_end.saturating_sub(aligned_start).max(timeframe_ms);
    let target_fetch_count = range_ms.saturating_div(timeframe_ms).max(1);
    let slices = build_window_fetch_slices(aligned_start, aligned_end, context.timeframe);
    let total_slices = slices.len() as i64;
    let target_batches = estimated_window_slice_batches(&slices, context.timeframe);
    let mut pipeline = BaseCandleSavePipeline::new(BaseCandleSavePipelineConfig {
        pool: context.pool.clone(),
        inst_id: context.inst_id.to_string(),
        inst_type: context.inst_type.to_string(),
        timeframe: context.timeframe.to_string(),
        target_fetch_count,
        target_save_count: target_fetch_count,
        target_batches,
        progress: context.progress.clone(),
        cancel_guard: context.cancel_guard.cloned(),
    });
    let fetch_progress = Arc::new(WindowFetchProgress::new(target_fetch_count, target_batches));
    let concurrency = window_fetch_concurrency();
    tracing::info!(
        inst_id = context.inst_id,
        inst_type = context.inst_type,
        timeframe = context.timeframe,
        start_ts = aligned_start,
        end_ts = aligned_end,
        slices = total_slices,
        concurrency,
        target_fetch_count,
        "range candle sync using concurrent range fetch"
    );

    let mut pending = FuturesUnordered::<JoinHandle<AppResult<WindowFetchSliceResult>>>::new();
    let mut slice_iter = slices.into_iter();
    let mut fetched_count = 0i64;
    let mut batches = 0i64;
    let mut api_calls = 0i64;
    let mut completed_slices = 0i64;

    while pending.len() < concurrency {
        let Some(slice) = slice_iter.next() else {
            break;
        };
        pending.push(tokio::spawn(fetch_window_slice(
            context.client.clone(),
            context.inst_id.to_string(),
            context.timeframe.to_string(),
            slice,
            fetch_progress.clone(),
            context.cancel_guard.cloned(),
        )));
    }

    let mut completed_results = BTreeMap::<i64, WindowFetchSliceResult>::new();
    let mut next_save_index = 0i64;
    while let Some(result) = pending.next().await {
        check_sync_cancel(context.cancel_guard).await?;
        let slice_result = result
            .map_err(|error| AppError::Runtime(format!("窗口 K 线并发拉取任务异常: {error}")))??;
        completed_slices += 1;
        batches += slice_result.batches;
        api_calls += slice_result.api_calls;
        completed_results.insert(slice_result.index, slice_result);
        while let Some(ready) = completed_results.remove(&next_save_index) {
            fetched_count += ready.candles.len() as i64;
            pipeline
                .submit(
                    ready.candles,
                    fetched_count,
                    batches,
                    api_calls,
                    format!(
                        "并发拉取 {} 中：时间片 {}/{}，已获取 {}/{} 条，API 请求 {} 次",
                        context.timeframe,
                        completed_slices,
                        total_slices,
                        fetched_count,
                        target_fetch_count,
                        api_calls
                    ),
                    window_fetch_progress_percent(fetched_count, target_fetch_count),
                )
                .await?;
            next_save_index += 1;
        }
        context
            .progress
            .report(SyncProgressUpdate {
                progress: window_fetch_progress_percent(fetched_count, target_fetch_count),
                message: format!(
                    "并发拉取 {} 中：时间片 {}/{}，已获取 {}/{} 条，已落库 {}/{} 条，API 请求 {} 次",
                    context.timeframe,
                    completed_slices,
                    total_slices,
                    fetched_count,
                    target_fetch_count,
                    pipeline.saved_count(),
                    target_fetch_count,
                    api_calls
                ),
                fetched_count,
                target_fetch_count,
                saved_count: pipeline.saved_count(),
                target_save_count: target_fetch_count,
                inserted_count: pipeline.saved_count(),
                batches,
                target_batches,
                api_calls,
                ..Default::default()
            })
            .await?;

        while pending.len() < concurrency {
            let Some(slice) = slice_iter.next() else {
                break;
            };
            pending.push(tokio::spawn(fetch_window_slice(
                context.client.clone(),
                context.inst_id.to_string(),
                context.timeframe.to_string(),
                slice,
                fetch_progress.clone(),
                context.cancel_guard.cloned(),
            )));
        }
    }

    let saved_count = pipeline.finish().await?;
    Ok(SyncFetchResult {
        fetched_count,
        target_fetch_count: target_fetch_count.max(fetched_count),
        saved_count,
        target_save_count: target_fetch_count.max(fetched_count),
        batches,
        target_batches: target_batches.max(batches),
        api_calls,
        history_complete: false,
        truncated: false,
        message: range.completion_message,
    })
}
