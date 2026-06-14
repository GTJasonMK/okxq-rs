use serde_json::Value;

use super::super::*;
use super::planning::select_read_sync_request;

pub(in crate::commands::local_api) async fn ensure_local_candles_for_read(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
    force_refresh: bool,
) -> AppResult<Option<Value>> {
    ensure_local_candles_coverage(
        state,
        inst_id,
        inst_type,
        timeframe,
        limit,
        None,
        force_refresh,
    )
    .await
}

pub(in crate::commands::local_api) async fn ensure_local_candles_coverage(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
    required_start_ts: Option<i64>,
    force_refresh: bool,
) -> AppResult<Option<Value>> {
    let scope = validated_coverage_scope(state, inst_id, inst_type, timeframe).await?;

    let Some(request) = select_read_sync_request(
        &state.db,
        &scope.inst_id,
        &scope.inst_type,
        &scope.timeframe,
        limit,
        required_start_ts,
        force_refresh,
    )
    .await?
    else {
        return Ok(None);
    };

    tracing::info!(
        inst_id = %request.inst_id,
        inst_type = %request.inst_type,
        timeframe = %request.timeframe,
        mode = %request.mode,
        days = request.days,
        force_refresh,
        "local candle read requires controlled sync"
    );
    let payload = run_sync_request_guarded(state, request).await?;
    Ok(Some(payload))
}

pub(in crate::commands::local_api) async fn ensure_local_candles_range_coverage(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    start_ts: i64,
    end_ts: i64,
) -> AppResult<Option<Value>> {
    if end_ts < start_ts {
        return Err(AppError::Validation(format!(
            "K 线覆盖检查时间范围无效：start_ts={start_ts} end_ts={end_ts}"
        )));
    }
    let scope = validated_coverage_scope(state, inst_id, inst_type, timeframe).await?;

    let Some((job, reused)) = super::super::enqueue_gap_repair_job_for_range(
        state,
        &scope.inst_id,
        &scope.inst_type,
        &scope.timeframe,
        start_ts,
        end_ts,
        "auto",
    )
    .await?
    else {
        return Ok(None);
    };

    tracing::info!(
        task_id = %job.task_id,
        inst_id = %job.inst_id,
        inst_type = %job.inst_type,
        timeframe = %job.timeframe,
        start_ts = job.start_ts,
        end_ts = job.end_ts,
        reused,
        "local historical candle range requires precise gap repair"
    );
    let completed = state.sync_jobs.wait_for_terminal(&job.task_id).await?;
    terminal_sync_job_payload(completed, reused).map(Some)
}

struct CoverageScope {
    inst_id: String,
    inst_type: String,
    timeframe: String,
}

async fn validated_coverage_scope(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
) -> AppResult<CoverageScope> {
    load_sync_runtime_settings(state).await?;
    let normalized_inst_id = inst_id.trim().to_uppercase();
    let normalized_inst_type = inst_type.trim().to_uppercase();
    let normalized_timeframe = normalize_timeframe_name(timeframe);
    if normalized_inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 不能为空".to_string()));
    }
    if normalized_timeframe.is_empty() {
        return Err(AppError::Validation(
            "不支持的 K 线周期，当前仅支持 1m/3m/5m/15m/30m/1H/2H/4H/6H/12H/1D/1W/1M".to_string(),
        ));
    }
    let scopes = watched_instrument_scopes(state).await?;
    if !watched_request_allowed(&scopes, &normalized_inst_id, &normalized_inst_type) {
        return Err(AppError::Validation(format!(
            "{} {} 未在关注清单中启用，已拒绝读取或自动补齐 K 线",
            normalized_inst_id, normalized_inst_type
        )));
    }
    ensure_timeframe_allowed_for_scope(
        state,
        &normalized_inst_id,
        &normalized_inst_type,
        &normalized_timeframe,
    )
    .await?;

    Ok(CoverageScope {
        inst_id: normalized_inst_id,
        inst_type: normalized_inst_type,
        timeframe: normalized_timeframe,
    })
}
