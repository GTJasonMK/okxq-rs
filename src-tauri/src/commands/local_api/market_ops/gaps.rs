use serde_json::Value;
use sqlx::SqlitePool;

use crate::sync_jobs::{SyncJob, SyncJobRequest};

use self::query::{build_gap_plan, load_repair_gap_ranges};
use self::repair::run_gap_repair_request;
pub(in crate::commands::local_api::market_ops) use self::repair_plan::latest_complete_candle_ts;
use self::repair_plan::{align_down, plan_gap_repairs, repair_plan_targets, GapRange};
use super::*;

mod historical_zip;
mod query;
mod repair;
mod repair_plan;

#[cfg(test)]
mod tests;

const DEFAULT_GAP_PLAN_DAYS: i64 = 30;
const DEFAULT_GAP_PLAN_LIMIT: i64 = 100;
const MAX_GAP_PLAN_LIMIT: i64 = 1_000;

#[derive(Clone, Debug, PartialEq, Eq)]
struct GapPlanRequest {
    inst_id: String,
    inst_type: String,
    timeframe: String,
    start_ts: i64,
    end_ts: i64,
    limit: i64,
}

pub(in crate::commands::local_api) async fn market_gap_plan(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let plan_request = gap_plan_request(req)?;
    let data = build_gap_plan(&state.db, &plan_request).await?;
    Ok(code_ok(data))
}

pub(in crate::commands::local_api) async fn start_gap_repair_job(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    load_sync_runtime_settings(state).await?;
    let plan_request = gap_repair_plan_request(req)?;
    let method_override = gap_repair_method(req)?;
    let request = gap_repair_sync_job_request(&state.db, &plan_request, &method_override).await?;
    let (job, reused) =
        enqueue_gap_repair_job(state, request, plan_request, method_override).await?;
    let mut data = job.to_value()?;
    if let Some(obj) = data.as_object_mut() {
        obj.insert("reused_existing".to_string(), Value::Bool(reused));
    }
    Ok(code_ok(data))
}

pub(in crate::commands::local_api::market_ops) async fn enqueue_gap_repair_job_for_range(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    start_ts: i64,
    end_ts: i64,
    method_override: &str,
) -> AppResult<Option<(SyncJob, bool)>> {
    let timeframe = normalize_timeframe_name(timeframe);
    if timeframe.is_empty() || end_ts < start_ts {
        return Ok(None);
    }
    let timeframe_ms = timeframe_to_ms(&timeframe).max(1);
    let plan_request = GapPlanRequest {
        inst_id: inst_id.trim().to_uppercase(),
        inst_type: inst_type.trim().to_uppercase(),
        timeframe,
        start_ts: align_down(start_ts, timeframe_ms),
        end_ts: align_down(end_ts, timeframe_ms),
        limit: MAX_GAP_PLAN_LIMIT,
    };
    if plan_request.end_ts < plan_request.start_ts {
        return Ok(None);
    }
    let ranges = load_repair_gap_ranges(&state.db, &plan_request, timeframe_ms).await?;
    if ranges
        .iter()
        .map(|range| range.missing_candles)
        .sum::<i64>()
        <= 0
    {
        return Ok(None);
    }
    let request = gap_repair_sync_job_request_from_ranges(&plan_request, method_override, &ranges)?;
    let (job, reused) =
        enqueue_gap_repair_job(state, request, plan_request, method_override.to_string()).await?;
    Ok(Some((job, reused)))
}

fn gap_repair_method(req: &LocalApiRequest) -> AppResult<String> {
    let method = request_string(req, "method", "auto")
        .trim()
        .to_ascii_lowercase();
    if !matches!(method.as_str(), "auto" | "paginated" | "historical_zip") {
        return Err(AppError::Validation(format!(
            "不支持的缺口补齐方式 method={method}"
        )));
    }
    Ok(method)
}

async fn gap_repair_sync_job_request(
    pool: &SqlitePool,
    plan_request: &GapPlanRequest,
    method_override: &str,
) -> AppResult<SyncJobRequest> {
    let timeframe_ms = timeframe_to_ms(&plan_request.timeframe).max(1);
    let ranges = load_repair_gap_ranges(pool, plan_request, timeframe_ms).await?;
    gap_repair_sync_job_request_from_ranges(plan_request, method_override, &ranges)
}

fn gap_repair_sync_job_request_from_ranges(
    plan_request: &GapPlanRequest,
    method_override: &str,
    ranges: &[GapRange],
) -> AppResult<SyncJobRequest> {
    let timeframe_ms = timeframe_to_ms(&plan_request.timeframe).max(1);
    let now_ms = chrono::Utc::now().timestamp_millis();
    let planned_ranges = plan_gap_repairs(
        ranges,
        &plan_request.timeframe,
        timeframe_ms,
        now_ms,
        method_override,
    );
    let plan_targets = repair_plan_targets(&planned_ranges);
    if plan_targets.missing_candles <= 0 || planned_ranges.is_empty() {
        return Err(AppError::Validation(format!(
            "{} {} {} 当前范围没有需要补齐的缺口",
            plan_request.inst_id, plan_request.inst_type, plan_request.timeframe
        )));
    }
    let source_timeframe = if plan_request.timeframe != BASE_CANDLE_TIMEFRAME
        || method_override == "historical_zip"
        || planned_ranges
            .iter()
            .any(|planned| planned.decision.method == "historical_zip")
    {
        BASE_CANDLE_TIMEFRAME.to_string()
    } else {
        plan_request.timeframe.clone()
    };
    let days = plan_request
        .end_ts
        .saturating_sub(plan_request.start_ts)
        .saturating_add(DAY_MS - 1)
        .saturating_div(DAY_MS)
        .max(1);
    Ok(SyncJobRequest {
        inst_id: plan_request.inst_id.clone(),
        inst_type: plan_request.inst_type.clone(),
        timeframe: plan_request.timeframe.clone(),
        source_timeframe,
        target_timeframes: vec![plan_request.timeframe.clone()],
        mode: "gap_repair".to_string(),
        days,
        start_ts: Some(plan_request.start_ts),
        end_ts: Some(plan_request.end_ts),
        repair_method: method_override.to_string(),
        target_fetch_count: plan_targets.source_candles,
        target_save_count: plan_targets.source_candles,
        target_derive_count: plan_targets.derive_candles,
        target_batches: planned_ranges.len() as i64,
    })
}

async fn enqueue_gap_repair_job(
    state: &AppState,
    request: SyncJobRequest,
    plan_request: GapPlanRequest,
    method_override: String,
) -> AppResult<(SyncJob, bool)> {
    enqueue_background_sync_job(
        state,
        request,
        BackgroundSyncJobKind::GapRepair,
        move |pool, client, _task_id, cancel_guard, progress| async move {
            run_gap_repair_request(
                &pool,
                &client,
                plan_request,
                method_override,
                Some(cancel_guard),
                progress,
            )
            .await
        },
    )
    .await
}

fn gap_plan_request(req: &LocalApiRequest) -> AppResult<GapPlanRequest> {
    let raw_inst_id = request_string(req, "inst_id", "").trim().to_uppercase();
    if raw_inst_id.is_empty() {
        return Err(AppError::Validation("缺口计划需要 inst_id".to_string()));
    }

    let inst_type = normalize_market_inst_type(
        &request_string(req, "inst_type", &infer_inst_type(&raw_inst_id)),
        &raw_inst_id,
    )?;
    let inst_id = normalize_market_inst_id(&raw_inst_id, &inst_type);
    let timeframe = required_timeframe_name(&request_string(req, "timeframe", "1m"), "timeframe")?;
    let timeframe_ms = timeframe_to_ms(&timeframe).max(1);
    let now_ms = chrono::Utc::now().timestamp_millis();
    let default_end_ts = latest_complete_candle_ts(now_ms, timeframe_ms);
    let end_ts = optional_request_i64(req, "end_ts").unwrap_or(default_end_ts);
    let start_ts = optional_request_i64(req, "start_ts").unwrap_or_else(|| {
        let days = request_i64(req, "days", DEFAULT_GAP_PLAN_DAYS)
            .clamp(1, 3650)
            .saturating_mul(DAY_MS);
        end_ts.saturating_sub(days)
    });
    let aligned_start_ts = align_down(start_ts, timeframe_ms);
    let aligned_end_ts = align_down(end_ts, timeframe_ms);
    if aligned_end_ts < aligned_start_ts {
        return Err(AppError::Validation(format!(
            "缺口计划时间范围无效：start_ts={start_ts} end_ts={end_ts}"
        )));
    }

    Ok(GapPlanRequest {
        inst_id,
        inst_type,
        timeframe,
        start_ts: aligned_start_ts,
        end_ts: aligned_end_ts,
        limit: request_i64(req, "limit", DEFAULT_GAP_PLAN_LIMIT).clamp(1, MAX_GAP_PLAN_LIMIT),
    })
}

fn gap_repair_plan_request(req: &LocalApiRequest) -> AppResult<GapPlanRequest> {
    if optional_request_i64(req, "start_ts").is_none()
        || optional_request_i64(req, "end_ts").is_none()
    {
        return Err(AppError::Validation(
            "精确补齐需要明确 start_ts 和 end_ts".to_string(),
        ));
    }
    gap_plan_request(req)
}

fn optional_request_i64(req: &LocalApiRequest, key: &str) -> Option<i64> {
    req.body
        .get(key)
        .or_else(|| req.params.get(key))
        .and_then(Value::as_i64)
}
