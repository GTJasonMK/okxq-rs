use sqlx::SqlitePool;

use crate::{error::AppResult, okx::OkxPublicClient, sync_jobs::SyncJobRequest};

use super::super::super::*;

pub(super) async fn fetch_base_candles(
    pool: &SqlitePool,
    client: &OkxPublicClient,
    request: &SyncJobRequest,
    source_timeframe: &str,
    cancel_guard: Option<&SyncCancelGuard>,
    progress: &SyncProgressReporter,
) -> AppResult<SyncFetchResult> {
    if request.mode == "derive" {
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
            message: "本地对齐完成".to_string(),
        });
    }

    let fetch_context = CandleFetchContext {
        pool,
        client,
        inst_id: &request.inst_id,
        inst_type: &request.inst_type,
        timeframe: source_timeframe,
        cancel_guard,
        progress,
    };
    match request.mode.as_str() {
        "full" => {
            fetch_full_candles(
                fetch_context.pool,
                fetch_context.client,
                fetch_context.inst_id,
                fetch_context.inst_type,
                source_timeframe,
                cancel_guard,
                progress,
            )
            .await
        }
        "incremental" => fetch_incremental_candles(fetch_context, request.days).await,
        _ => fetch_window_candles(fetch_context, request.days).await,
    }
}
