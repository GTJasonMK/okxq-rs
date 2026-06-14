use std::collections::BTreeSet;

use super::super::*;
use super::{paging::pause_okx_page, window::fetch_window_candles, CandleFetchContext};

pub(in crate::commands::local_api::market_ops) async fn fetch_incremental_candles(
    context: CandleFetchContext<'_>,
    days: i64,
) -> AppResult<SyncFetchResult> {
    let Some((_oldest, newest)) = local_candle_bounds(
        context.pool,
        context.inst_id,
        context.inst_type,
        context.timeframe,
    )
    .await?
    else {
        let mut result = fetch_window_candles(context, days).await?;
        result.message = "增量同步完成（本地为空，已执行窗口初始化）".to_string();
        return Ok(result);
    };

    let timeframe_ms = timeframe_to_ms(context.timeframe);
    let now_ms = chrono::Utc::now().timestamp_millis();
    let target_fetch_count = estimated_incremental_fetch_count(newest, now_ms, timeframe_ms);
    let target_batches = estimated_batches_for_count(target_fetch_count);
    if newest.saturating_add(timeframe_ms) > now_ms {
        return Ok(SyncFetchResult {
            fetched_count: 0,
            target_fetch_count: 0,
            saved_count: 0,
            target_save_count: 0,
            batches: 0,
            target_batches: 0,
            api_calls: 0,
            history_complete: false,
            truncated: false,
            message: "本地数据已是最新，无需增量同步".to_string(),
        });
    }

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
    let mut seen_timestamps = BTreeSet::<i64>::new();
    let mut fetched_count = 0i64;
    let mut cursor_before = newest;
    let mut batches = 0usize;
    let mut api_calls = 0usize;
    let mut truncated = false;
    let max_batches = max_sync_batches();

    loop {
        check_sync_cancel(context.cancel_guard).await?;
        if batches >= max_batches {
            truncated = true;
            break;
        }
        let batch = context
            .client
            .get_candles(
                context.inst_id,
                context.timeframe,
                OKX_CANDLE_BATCH_LIMIT,
                Some(cursor_before.to_string()),
                None,
                true,
            )
            .await?;
        api_calls += 1;

        let mut newest_in_batch = cursor_before;
        let mut added = 0usize;
        let mut filtered_batch = Vec::new();
        for candle in batch
            .into_iter()
            .filter(|item| item.timestamp > cursor_before)
        {
            newest_in_batch = newest_in_batch.max(candle.timestamp);
            if seen_timestamps.insert(candle.timestamp) {
                filtered_batch.push(candle);
                added += 1;
            }
        }
        fetched_count += filtered_batch.len() as i64;
        pipeline
            .submit(
                filtered_batch,
                fetched_count,
                (batches + 1) as i64,
                api_calls as i64,
                format!(
                    "增量同步 {} 中：第 {} 批，已获取 {}/{} 条，新增 {} 条",
                    context.timeframe,
                    batches + 1,
                    fetched_count,
                    target_fetch_count,
                    added
                ),
                (10 + (batches as i64 * 4).min(50)).min(68),
            )
            .await?;
        context
            .progress
            .report(SyncProgressUpdate {
                progress: (10 + (batches as i64 * 4).min(50)).min(68),
                message: format!(
                    "增量同步 {} 中：第 {} 批，已获取 {}/{} 条，已落库 {}/{} 条，新增 {} 条",
                    context.timeframe,
                    batches + 1,
                    fetched_count,
                    target_fetch_count,
                    pipeline.saved_count(),
                    target_fetch_count,
                    added
                ),
                fetched_count,
                target_fetch_count,
                saved_count: pipeline.saved_count(),
                target_save_count: target_fetch_count,
                inserted_count: pipeline.saved_count(),
                batches: (batches + 1) as i64,
                target_batches,
                api_calls: api_calls as i64,
                ..Default::default()
            })
            .await?;

        if added == 0 || newest_in_batch <= cursor_before {
            break;
        }
        batches += 1;
        if newest_in_batch.saturating_add(timeframe_ms) >= now_ms {
            break;
        }
        cursor_before = newest_in_batch;
        pause_okx_page().await;
    }

    let saved_count = pipeline.finish().await?;
    Ok(SyncFetchResult {
        fetched_count,
        target_fetch_count: target_fetch_count.max(fetched_count),
        saved_count,
        target_save_count: target_fetch_count.max(fetched_count),
        batches: batches as i64,
        target_batches: target_batches.max(batches as i64),
        api_calls: api_calls as i64,
        history_complete: false,
        truncated,
        message: "增量同步完成".to_string(),
    })
}
