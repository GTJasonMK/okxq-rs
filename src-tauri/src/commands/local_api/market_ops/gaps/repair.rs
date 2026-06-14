use std::collections::BTreeMap;

use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::okx::OkxPublicClient;

use super::*;
use super::{
    historical_zip::import_okx_historical_zip_range,
    query::{load_repair_gap_ranges, local_candles_cover_range, remaining_gap_candles},
    repair_plan::{
        historical_source_end_ts, merge_repair_totals, plan_gap_repairs, repair_plan_targets,
        repair_report_value, source_end_ts_for_target_range, source_timeframe_for_repair_totals,
        source_timeframe_for_target, GapRange, GapRangeRepairReport, GapRepairTotals,
        RepairDecision,
    },
    GapPlanRequest,
};

pub(super) async fn run_gap_repair_request(
    pool: &SqlitePool,
    client: &OkxPublicClient,
    plan_request: GapPlanRequest,
    method_override: String,
    cancel_guard: Option<SyncCancelGuard>,
    progress: SyncProgressReporter,
) -> AppResult<Value> {
    let timeframe_ms = timeframe_to_ms(&plan_request.timeframe).max(1);
    let ranges = load_repair_gap_ranges(pool, &plan_request, timeframe_ms).await?;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let planned_ranges = plan_gap_repairs(
        &ranges,
        &plan_request.timeframe,
        timeframe_ms,
        now_ms,
        &method_override,
    );
    let plan_targets = repair_plan_targets(&planned_ranges);
    progress
        .report(SyncProgressUpdate {
            progress: 3,
            message: format!(
                "准备补齐 {} {} {}：{} 段缺口，缺失 {}",
                plan_request.inst_id,
                plan_request.inst_type,
                plan_request.timeframe,
                planned_ranges.len(),
                plan_targets.missing_candles
            ),
            target_fetch_count: plan_targets.source_candles,
            target_save_count: plan_targets.source_candles,
            target_derive_count: plan_targets.derive_candles,
            target_batches: planned_ranges.len() as i64,
            ..Default::default()
        })
        .await?;

    let mut totals = GapRepairTotals::default();
    let mut reports = Vec::new();
    let mut deferred_source_sync_records = BTreeMap::new();
    for (index, planned) in planned_ranges.iter().enumerate() {
        let range = &planned.range;
        check_sync_cancel(cancel_guard.as_ref()).await?;
        let base_progress = 5 + ((index as i64 * 85) / planned_ranges.len().max(1) as i64);
        progress
            .report(SyncProgressUpdate {
                progress: base_progress,
                message: format!(
                    "补齐缺口 {}/{}：{} 至 {}，缺失 {}",
                    index + 1,
                    planned_ranges.len(),
                    ts_to_iso(Some(range.start_ts)).as_str().unwrap_or(""),
                    ts_to_iso(Some(range.end_ts)).as_str().unwrap_or(""),
                    range.missing_candles
                ),
                fetched_count: totals.fetched_count,
                target_fetch_count: plan_targets.source_candles,
                saved_count: totals.saved_count,
                target_save_count: plan_targets.source_candles,
                inserted_count: totals.saved_count + totals.derived_count,
                derived_count: totals.derived_count,
                target_derive_count: plan_targets.derive_candles,
                batches: index as i64,
                target_batches: planned_ranges.len() as i64,
                api_calls: totals.api_calls,
            })
            .await?;
        let report = repair_gap_range(
            pool,
            client,
            &plan_request,
            range,
            &planned.decision,
            &progress,
            cancel_guard.as_ref(),
        )
        .await?;
        merge_repair_totals(&mut totals, &report);
        defer_source_sync_record_refresh(&mut deferred_source_sync_records, &plan_request, &report);
        reports.push(repair_report_value(&report));
    }

    flush_deferred_source_sync_record_refreshes(pool, deferred_source_sync_records).await?;
    update_sync_record(
        pool,
        &plan_request.inst_id,
        &plan_request.inst_type,
        &plan_request.timeframe,
        None,
        Some("gap_repair"),
    )
    .await?;
    let remaining_missing = remaining_gap_candles(pool, &plan_request, timeframe_ms).await?;
    if remaining_missing > 0 {
        return Err(AppError::Runtime(format!(
            "{} {} {} 补齐任务已执行，但 {} 至 {} 仍缺失 {} 根 K 线",
            plan_request.inst_id,
            plan_request.inst_type,
            plan_request.timeframe,
            ts_to_iso(Some(plan_request.start_ts))
                .as_str()
                .unwrap_or(""),
            ts_to_iso(Some(plan_request.end_ts)).as_str().unwrap_or(""),
            remaining_missing
        )));
    }
    let display_record = get_sync_record_stats(
        pool,
        &plan_request.inst_id,
        &plan_request.inst_type,
        &plan_request.timeframe,
    )
    .await?
    .ok_or_else(|| {
        AppError::Runtime(format!(
            "{} {} {} 补齐后未生成本地同步记录",
            plan_request.inst_id, plan_request.inst_type, plan_request.timeframe
        ))
    })?;
    let oldest_time = ts_to_iso(display_record.oldest_timestamp);
    let newest_time = ts_to_iso(display_record.newest_timestamp);
    let sync_record_value = json!({
        "last_sync_time": &display_record.last_sync_time,
        "oldest_timestamp": display_record.oldest_timestamp,
        "newest_timestamp": display_record.newest_timestamp,
        "oldest_time": &oldest_time,
        "newest_time": &newest_time,
        "candle_count": display_record.candle_count,
        "history_complete": display_record.history_complete,
        "last_sync_mode": &display_record.last_sync_mode,
    });
    let source_timeframe =
        source_timeframe_for_repair_totals(&plan_request.timeframe, totals.historical_zip_ranges);
    Ok(json!({
        "message": format!("缺口补齐完成：处理 {} 段缺口", reports.len()),
        "inst_id": plan_request.inst_id,
        "inst_type": plan_request.inst_type,
        "timeframe": plan_request.timeframe,
        "source_timeframe": source_timeframe,
        "target_timeframes": [plan_request.timeframe.clone()],
        "last_sync_mode": &display_record.last_sync_mode,
        "last_sync_time": &display_record.last_sync_time,
        "oldest_timestamp": display_record.oldest_timestamp,
        "newest_timestamp": display_record.newest_timestamp,
        "oldest_time": &oldest_time,
        "newest_time": &newest_time,
        "range": {
            "start_ts": plan_request.start_ts,
            "end_ts": plan_request.end_ts,
            "start_time": ts_to_iso(Some(plan_request.start_ts)),
            "end_time": ts_to_iso(Some(plan_request.end_ts)),
        },
        "processed_gap_count": reports.len() as i64,
        "fetched_count": totals.fetched_count,
        "saved_count": totals.saved_count,
        "derived_count": totals.derived_count,
        "api_calls": totals.api_calls,
        "zip_files": totals.zip_files,
        "zip_rows_seen": totals.zip_rows_seen,
        "zip_rows_imported": totals.zip_rows_imported,
        "paginated_ranges": totals.paginated_ranges,
        "historical_zip_ranges": totals.historical_zip_ranges,
        "target_fetch_count": plan_targets.source_candles,
        "target_save_count": plan_targets.source_candles,
        "inserted_count": totals.saved_count + totals.derived_count,
        "target_derive_count": plan_targets.derive_candles,
        "batches": reports.len() as i64,
        "target_batches": planned_ranges.len() as i64,
        "candle_count": display_record.candle_count,
        "history_complete": display_record.history_complete,
        "truncated": false,
        "sync_record": sync_record_value,
        "gaps": reports,
    }))
}

pub(super) fn defer_source_sync_record_refresh(
    deferred: &mut BTreeMap<(String, String, String), String>,
    request: &GapPlanRequest,
    report: &GapRangeRepairReport,
) {
    let fetch_timeframe = report.fetch_timeframe.trim();
    if fetch_timeframe.is_empty() || fetch_timeframe == request.timeframe {
        return;
    }
    if report.fetched_count <= 0 && report.zip_rows_imported <= 0 {
        return;
    }
    let mode = match report.method.as_str() {
        "historical_zip" => "okx_historical_zip",
        "paginated" => "gap_paginated",
        _ => "gap_repair",
    };
    deferred.insert(
        (
            request.inst_id.clone(),
            request.inst_type.clone(),
            fetch_timeframe.to_string(),
        ),
        mode.to_string(),
    );
}

pub(super) async fn flush_deferred_source_sync_record_refreshes(
    pool: &SqlitePool,
    deferred: BTreeMap<(String, String, String), String>,
) -> AppResult<()> {
    for ((inst_id, inst_type, timeframe), last_sync_mode) in deferred {
        update_sync_record(
            pool,
            &inst_id,
            &inst_type,
            &timeframe,
            None,
            Some(&last_sync_mode),
        )
        .await?;
    }
    Ok(())
}

async fn repair_gap_range(
    pool: &SqlitePool,
    client: &OkxPublicClient,
    request: &GapPlanRequest,
    range: &GapRange,
    decision: &RepairDecision,
    progress: &SyncProgressReporter,
    cancel_guard: Option<&SyncCancelGuard>,
) -> AppResult<GapRangeRepairReport> {
    match decision.method {
        "historical_zip" => {
            repair_gap_range_with_historical_zip(
                pool,
                client,
                request,
                range,
                progress,
                cancel_guard,
            )
            .await
        }
        _ => {
            repair_gap_range_with_paginated(pool, client, request, range, progress, cancel_guard)
                .await
        }
    }
}

async fn repair_gap_range_with_paginated(
    pool: &SqlitePool,
    client: &OkxPublicClient,
    request: &GapPlanRequest,
    range: &GapRange,
    progress: &SyncProgressReporter,
    cancel_guard: Option<&SyncCancelGuard>,
) -> AppResult<GapRangeRepairReport> {
    let source_timeframe = source_timeframe_for_target(&request.timeframe);
    let source_end_ts = source_end_ts_for_target_range(range, &request.timeframe);
    let source_covered = request.timeframe != BASE_CANDLE_TIMEFRAME
        && local_candles_cover_range(
            pool,
            &request.inst_id,
            &request.inst_type,
            &source_timeframe,
            range.start_ts,
            source_end_ts,
        )
        .await?;
    let result = if source_covered {
        progress
            .report(SyncProgressUpdate {
                progress: 68,
                message: format!(
                    "本地 {} 已覆盖 {} 至 {}，直接对齐 {}",
                    source_timeframe,
                    ts_to_iso(Some(range.start_ts)).as_str().unwrap_or(""),
                    ts_to_iso(Some(source_end_ts)).as_str().unwrap_or(""),
                    request.timeframe
                ),
                ..Default::default()
            })
            .await?;
        SyncFetchResult {
            fetched_count: 0,
            target_fetch_count: 0,
            saved_count: 0,
            target_save_count: 0,
            batches: 0,
            target_batches: 0,
            api_calls: 0,
            history_complete: false,
            truncated: false,
            message: "本地基础 K 线已覆盖，直接对齐".to_string(),
        }
    } else {
        let result = fetch_range_candles(
            CandleFetchContext {
                pool,
                client,
                inst_id: &request.inst_id,
                inst_type: &request.inst_type,
                timeframe: &source_timeframe,
                cancel_guard,
                progress,
            },
            CandleFetchRange {
                start_ts: range.start_ts,
                end_ts: source_end_ts,
                completion_message: format!(
                    "分页缺口补齐完成：{} 至 {}",
                    ts_to_iso(Some(range.start_ts)).as_str().unwrap_or(""),
                    ts_to_iso(Some(source_end_ts)).as_str().unwrap_or("")
                ),
            },
        )
        .await?;
        result
    };
    let target_timeframes = vec![request.timeframe.clone()];
    let derive_result = if request.timeframe != BASE_CANDLE_TIMEFRAME {
        derive_candles_from_base_range(
            DeriveCandleContext {
                pool,
                inst_id: &request.inst_id,
                inst_type: &request.inst_type,
                cancel_guard,
                progress,
            },
            DeriveCandleRangeRequest {
                source_timeframe: BASE_CANDLE_TIMEFRAME,
                target_timeframes: &target_timeframes,
                start_ts: range.start_ts,
                end_ts: range.end_ts,
                last_sync_mode: Some("gap_paginated"),
            },
            DeriveCandleProgress {
                fetched_count: result.fetched_count,
                target_fetch_count: result.fetched_count,
                target_save_count: result.saved_count,
                base_saved_count: result.saved_count,
                batches: result.api_calls,
                target_batches: result.api_calls,
                api_calls: result.api_calls,
            },
        )
        .await?
    } else {
        Default::default()
    };
    Ok(GapRangeRepairReport {
        method: "paginated".to_string(),
        start_ts: range.start_ts,
        end_ts: range.end_ts,
        missing_candles: range.missing_candles,
        fetch_timeframe: source_timeframe,
        fetched_count: result.fetched_count,
        saved_count: result.saved_count,
        derived_count: derive_result.saved_count,
        api_calls: result.api_calls,
        ..Default::default()
    })
}

async fn repair_gap_range_with_historical_zip(
    pool: &SqlitePool,
    client: &OkxPublicClient,
    request: &GapPlanRequest,
    range: &GapRange,
    progress: &SyncProgressReporter,
    cancel_guard: Option<&SyncCancelGuard>,
) -> AppResult<GapRangeRepairReport> {
    let source_end_ts = historical_source_end_ts(range, &request.timeframe);
    let import_report = import_okx_historical_zip_range(
        CandleFetchContext {
            pool,
            client,
            inst_id: &request.inst_id,
            inst_type: &request.inst_type,
            timeframe: BASE_CANDLE_TIMEFRAME,
            cancel_guard,
            progress,
        },
        range.start_ts,
        source_end_ts,
    )
    .await?;
    update_sync_record_metadata(
        pool,
        &request.inst_id,
        &request.inst_type,
        BASE_CANDLE_TIMEFRAME,
        None,
        Some("okx_historical_zip"),
    )
    .await?;

    let derive_result = if request.timeframe == BASE_CANDLE_TIMEFRAME {
        update_sync_record_metadata(
            pool,
            &request.inst_id,
            &request.inst_type,
            &request.timeframe,
            None,
            Some("gap_historical_zip"),
        )
        .await?;
        Default::default()
    } else {
        derive_candles_from_base_range(
            DeriveCandleContext {
                pool,
                inst_id: &request.inst_id,
                inst_type: &request.inst_type,
                cancel_guard,
                progress,
            },
            DeriveCandleRangeRequest {
                source_timeframe: BASE_CANDLE_TIMEFRAME,
                target_timeframes: std::slice::from_ref(&request.timeframe),
                start_ts: range.start_ts,
                end_ts: range.end_ts,
                last_sync_mode: Some("gap_historical_zip"),
            },
            DeriveCandleProgress {
                fetched_count: import_report.rows_imported,
                target_fetch_count: import_report.rows_imported,
                target_save_count: import_report.rows_imported,
                base_saved_count: import_report.saved_count,
                batches: import_report.links,
                target_batches: import_report.links,
                api_calls: import_report.links,
            },
        )
        .await?
    };

    Ok(GapRangeRepairReport {
        method: "historical_zip".to_string(),
        start_ts: range.start_ts,
        end_ts: range.end_ts,
        missing_candles: range.missing_candles,
        fetch_timeframe: BASE_CANDLE_TIMEFRAME.to_string(),
        fetched_count: import_report.rows_imported,
        saved_count: import_report.saved_count,
        derived_count: derive_result.saved_count,
        api_calls: import_report.links,
        zip_files: import_report.files,
        zip_rows_seen: import_report.rows_seen,
        zip_rows_imported: import_report.rows_imported,
    })
}
