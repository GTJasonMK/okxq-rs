mod fetch;
mod plan;
mod records;
mod response;

use serde_json::Value;
use sqlx::SqlitePool;

use crate::{
    error::{AppError, AppResult},
    okx::OkxPublicClient,
    sync_jobs::SyncJobRequest,
};

use self::{
    fetch::fetch_base_candles,
    plan::resolve_sync_request_plan,
    records::update_base_sync_record,
    response::{sync_completion_payload, SyncCompletionPayloadInput, SyncCompletionTimeline},
};
use super::super::*;

pub(in crate::commands::local_api::market_ops) async fn run_sync_request(
    pool: SqlitePool,
    client: OkxPublicClient,
    request: SyncJobRequest,
    task_id: Option<String>,
    cancel_guard: Option<SyncCancelGuard>,
    progress: SyncProgressReporter,
) -> AppResult<Value> {
    let task_id = task_id
        .unwrap_or_else(|| format!("sync_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]));
    let created_at = now_text();
    let started_at = created_at.clone();

    check_sync_cancel(cancel_guard.as_ref()).await?;
    let plan = resolve_sync_request_plan(&request)?;

    tracing::info!(
        task_id = %task_id,
        inst_id = %request.inst_id,
        inst_type = %request.inst_type,
        display_timeframe = %plan.display_timeframe,
        source_timeframe = %plan.source_timeframe,
        target_timeframes = %plan.target_timeframes.join("/"),
        mode = %request.mode,
        days = request.days,
        "sync request resolved to candle pipeline"
    );
    progress
        .report(SyncProgressUpdate {
            progress: 3,
            message: format!(
                "开始同步 {} {}，目标周期 {}",
                request.inst_id,
                request.inst_type,
                plan.target_timeframes.join("/")
            ),
            target_fetch_count: plan.planned_fetch_count,
            target_save_count: plan.planned_save_count,
            target_derive_count: plan.planned_derive_count,
            target_batches: plan.planned_batches,
            ..Default::default()
        })
        .await?;

    let fetch_result = fetch_base_candles(
        &pool,
        &client,
        &request,
        &plan.source_timeframe,
        cancel_guard.as_ref(),
        &progress,
    )
    .await?;

    check_sync_cancel(cancel_guard.as_ref()).await?;
    let saved_count = fetch_result.saved_count;
    progress
        .report(SyncProgressUpdate {
            progress: 68,
            message: format!(
                "基础 K 线同步完成，拉取 {}/{} 条，落库 {}/{} 条",
                fetch_result.fetched_count,
                fetch_result.target_fetch_count,
                saved_count,
                fetch_result.target_save_count
            ),
            fetched_count: fetch_result.fetched_count,
            target_fetch_count: fetch_result.target_fetch_count,
            saved_count,
            target_save_count: fetch_result.target_save_count,
            inserted_count: saved_count,
            batches: fetch_result.batches,
            target_batches: fetch_result.target_batches,
            api_calls: fetch_result.api_calls,
            ..Default::default()
        })
        .await?;
    check_sync_cancel(cancel_guard.as_ref()).await?;
    let base_record =
        update_base_sync_record(&pool, &request, &plan.source_timeframe, &fetch_result).await?;
    let derive_result = derive_candles_from_base(
        DeriveCandleContext {
            pool: &pool,
            inst_id: &request.inst_id,
            inst_type: &request.inst_type,
            cancel_guard: cancel_guard.as_ref(),
            progress: &progress,
        },
        DeriveCandleRequest {
            source_timeframe: &plan.source_timeframe,
            target_timeframes: &plan.target_timeframes,
            history_complete: base_record.history_complete,
            last_sync_mode: Some(&request.mode),
            days: request.days,
        },
        DeriveCandleProgress {
            fetched_count: fetch_result.fetched_count,
            target_fetch_count: fetch_result.target_fetch_count,
            target_save_count: fetch_result.target_save_count,
            base_saved_count: saved_count,
            batches: fetch_result.batches,
            target_batches: fetch_result.target_batches,
            api_calls: fetch_result.api_calls,
        },
    )
    .await?;
    let display_record = select_display_record(
        get_sync_record_stats(
            &pool,
            &request.inst_id,
            &request.inst_type,
            &plan.display_timeframe,
        )
        .await?,
        base_record,
        &request,
        &plan.display_timeframe,
        &plan.source_timeframe,
    )?;
    let finished_at = now_text();
    Ok(sync_completion_payload(SyncCompletionPayloadInput {
        task_id: &task_id,
        request: &request,
        plan: &plan,
        fetch_result: &fetch_result,
        derive_result: &derive_result,
        display_record: &display_record,
        timeline: SyncCompletionTimeline {
            created_at: &created_at,
            started_at: &started_at,
            finished_at: &finished_at,
        },
    }))
}

fn select_display_record(
    display_record: Option<SyncRecordStats>,
    base_record: SyncRecordStats,
    request: &SyncJobRequest,
    display_timeframe: &str,
    source_timeframe: &str,
) -> AppResult<SyncRecordStats> {
    if let Some(record) = display_record {
        return Ok(record);
    }
    if display_timeframe == source_timeframe {
        return Ok(base_record);
    }
    Err(AppError::Runtime(format!(
        "{} {} 同步后未生成目标周期 {} 的本地记录",
        request.inst_id, request.inst_type, display_timeframe
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(timeframe: &str, source_timeframe: &str) -> SyncJobRequest {
        SyncJobRequest {
            inst_id: "BTC-USDT-SWAP".to_string(),
            inst_type: "SWAP".to_string(),
            timeframe: timeframe.to_string(),
            source_timeframe: source_timeframe.to_string(),
            target_timeframes: vec![timeframe.to_string()],
            mode: "window".to_string(),
            days: 30,
            start_ts: None,
            end_ts: None,
            repair_method: String::new(),
            target_fetch_count: 0,
            target_save_count: 0,
            target_derive_count: 0,
            target_batches: 0,
        }
    }

    fn record() -> SyncRecordStats {
        SyncRecordStats {
            last_sync_time: Some("2026-05-28T00:00:00Z".to_string()),
            oldest_timestamp: Some(0),
            newest_timestamp: Some(60_000),
            candle_count: 2,
            history_complete: false,
            last_sync_mode: "window".to_string(),
        }
    }

    #[test]
    fn display_record_must_exist_for_derived_timeframe() {
        let err = select_display_record(None, record(), &request("1H", "1m"), "1H", "1m")
            .expect_err("missing derived record must fail");

        assert!(err.to_string().contains("未生成目标周期 1H"));
    }

    #[test]
    fn base_record_can_display_base_timeframe() {
        let selected = select_display_record(None, record(), &request("1m", "1m"), "1m", "1m")
            .expect("base timeframe can use base record");

        assert_eq!(selected.candle_count, 2);
    }
}
