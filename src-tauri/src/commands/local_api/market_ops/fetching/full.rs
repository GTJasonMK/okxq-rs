use std::collections::BTreeSet;

use sqlx::SqlitePool;

use crate::okx::OkxPublicClient;

use super::super::*;
use super::paging::pause_okx_page;

pub(in crate::commands::local_api::market_ops) async fn fetch_full_candles(
    pool: &SqlitePool,
    client: &OkxPublicClient,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    cancel_guard: Option<&SyncCancelGuard>,
    progress: &SyncProgressReporter,
) -> AppResult<SyncFetchResult> {
    let max_batches = max_sync_batches();
    let max_target_fetch_count = (max_batches as i64) * i64::from(OKX_CANDLE_BATCH_LIMIT);
    let mut pipeline = BaseCandleSavePipeline::new(BaseCandleSavePipelineConfig {
        pool: pool.clone(),
        inst_id: inst_id.to_string(),
        inst_type: inst_type.to_string(),
        timeframe: timeframe.to_string(),
        target_fetch_count: max_target_fetch_count,
        target_save_count: max_target_fetch_count,
        target_batches: max_batches as i64,
        progress: progress.clone(),
        cancel_guard: cancel_guard.cloned(),
    });
    let mut seen_timestamps = BTreeSet::<i64>::new();
    let mut fetched_count = 0i64;
    let mut cursor_after: Option<String> = None;
    let mut previous_oldest = None;
    let mut batches = 0usize;
    let mut api_calls = 0usize;
    let mut history_complete = false;
    let mut truncated = false;

    loop {
        check_sync_cancel(cancel_guard).await?;
        if batches >= max_batches {
            truncated = true;
            break;
        }
        let batch = client
            .get_candles(
                inst_id,
                timeframe,
                OKX_CANDLE_BATCH_LIMIT,
                None,
                cursor_after.clone(),
                true,
            )
            .await?;
        api_calls += 1;
        if batch.is_empty() {
            history_complete = fetched_count > 0;
            break;
        }

        let oldest = batch.first().map(|item| item.timestamp).unwrap_or(0);
        let mut filtered_batch = Vec::new();
        for candle in batch {
            if seen_timestamps.insert(candle.timestamp) {
                filtered_batch.push(candle);
            }
        }
        batches += 1;
        fetched_count += filtered_batch.len() as i64;
        pipeline
            .submit(
                filtered_batch,
                fetched_count,
                batches as i64,
                api_calls as i64,
                format!(
                    "全量回补 {} 中：第 {} 批，已获取 {} 条，API 请求 {} 次",
                    timeframe, batches, fetched_count, api_calls
                ),
                (5 + (batches as i64 * 2).min(55)).min(67),
            )
            .await?;
        progress
            .report(SyncProgressUpdate {
                progress: (5 + (batches as i64 * 2).min(55)).min(67),
                message: format!(
                    "全量回补 {} 中：第 {} 批，已获取 {} 条，已落库 {} 条，API 请求 {} 次",
                    timeframe,
                    batches,
                    fetched_count,
                    pipeline.saved_count(),
                    api_calls
                ),
                fetched_count,
                target_fetch_count: max_target_fetch_count,
                saved_count: pipeline.saved_count(),
                target_save_count: max_target_fetch_count,
                inserted_count: pipeline.saved_count(),
                batches: batches as i64,
                target_batches: max_batches as i64,
                api_calls: api_calls as i64,
                ..Default::default()
            })
            .await?;

        if previous_oldest == Some(oldest) {
            history_complete = true;
            break;
        }
        previous_oldest = Some(oldest);
        cursor_after = Some(oldest.to_string());
        pause_okx_page().await;
    }

    let saved_count = pipeline.finish().await?;
    let completed_target = if truncated {
        max_target_fetch_count
    } else {
        fetched_count
    };
    let completed_batches = if truncated {
        max_batches as i64
    } else {
        batches as i64
    };
    Ok(SyncFetchResult {
        fetched_count,
        target_fetch_count: completed_target,
        saved_count,
        target_save_count: completed_target,
        batches: batches as i64,
        target_batches: completed_batches,
        api_calls: api_calls as i64,
        history_complete: history_complete && !truncated,
        truncated,
        message: "全量历史回补完成".to_string(),
    })
}
