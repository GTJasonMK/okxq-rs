use serde_json::{json, Value};

use super::*;

const PAGINATED_MAX_API_CALLS: i64 = 50;
const PAGINATED_MAX_MISSING_CANDLES: i64 = 10_000;
const HISTORICAL_ZIP_MIN_SPAN_MS: i64 = 7 * DAY_MS;
const RECENT_TAIL_MS: i64 = 2 * DAY_MS;

#[derive(Clone, Debug)]
pub(super) struct GapRange {
    pub(super) start_ts: i64,
    pub(super) end_ts: i64,
    pub(super) missing_candles: i64,
}

#[derive(Clone, Debug, Default)]
pub(super) struct GapRepairTotals {
    pub(super) fetched_count: i64,
    pub(super) saved_count: i64,
    pub(super) derived_count: i64,
    pub(super) api_calls: i64,
    pub(super) zip_files: i64,
    pub(super) zip_rows_seen: i64,
    pub(super) zip_rows_imported: i64,
    pub(super) paginated_ranges: i64,
    pub(super) historical_zip_ranges: i64,
}

#[derive(Clone, Debug)]
pub(super) struct PlannedGapRepair {
    pub(super) range: GapRange,
    pub(super) decision: RepairDecision,
    pub(super) source_candles: i64,
    pub(super) derive_candles: i64,
}

#[derive(Clone, Debug, Default)]
pub(super) struct GapRepairPlanTargets {
    pub(super) missing_candles: i64,
    pub(super) source_candles: i64,
    pub(super) derive_candles: i64,
}

#[derive(Clone, Debug, Default)]
pub(super) struct GapRangeRepairReport {
    pub(super) method: String,
    pub(super) start_ts: i64,
    pub(super) end_ts: i64,
    pub(super) missing_candles: i64,
    pub(super) fetch_timeframe: String,
    pub(super) fetched_count: i64,
    pub(super) saved_count: i64,
    pub(super) derived_count: i64,
    pub(super) api_calls: i64,
    pub(super) zip_files: i64,
    pub(super) zip_rows_seen: i64,
    pub(super) zip_rows_imported: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RepairDecision {
    pub(super) method: &'static str,
    pub(super) reason: &'static str,
}

pub(super) fn plan_gap_repairs(
    ranges: &[GapRange],
    timeframe: &str,
    timeframe_ms: i64,
    now_ms: i64,
    method_override: &str,
) -> Vec<PlannedGapRepair> {
    ranges
        .iter()
        .cloned()
        .map(|range| {
            let decision = repair_decision_with_override(
                &range,
                timeframe,
                timeframe_ms,
                now_ms,
                method_override,
            );
            let source_candles = source_candles_for_repair(&range, &decision, timeframe);
            let derive_candles = if timeframe != BASE_CANDLE_TIMEFRAME {
                range.missing_candles
            } else {
                0
            };
            PlannedGapRepair {
                range,
                decision,
                source_candles,
                derive_candles,
            }
        })
        .collect()
}

pub(super) fn repair_plan_targets(planned_ranges: &[PlannedGapRepair]) -> GapRepairPlanTargets {
    planned_ranges
        .iter()
        .fold(GapRepairPlanTargets::default(), |mut targets, planned| {
            targets.missing_candles += planned.range.missing_candles;
            targets.source_candles += planned.source_candles;
            targets.derive_candles += planned.derive_candles;
            targets
        })
}

pub(super) fn gap_range_value(
    range: &GapRange,
    timeframe: &str,
    timeframe_ms: i64,
    now_ms: i64,
) -> Value {
    let decision = repair_method_for_gap(range, timeframe, timeframe_ms, now_ms);
    let source_timeframe = source_timeframe_for_decision(&decision, timeframe);
    json!({
        "start_ts": range.start_ts,
        "end_ts": range.end_ts,
        "start_time": ts_to_iso(Some(range.start_ts)),
        "end_time": ts_to_iso(Some(range.end_ts)),
        "span_ms": range.end_ts.saturating_sub(range.start_ts).saturating_add(timeframe_ms),
        "missing_candles": range.missing_candles,
        "method": decision.method,
        "reason": decision.reason,
        "fetch_timeframe": source_timeframe,
        "target_timeframes": [timeframe.to_string()],
        "requires_derivation": timeframe != BASE_CANDLE_TIMEFRAME,
        "zip": if decision.method == "historical_zip" {
            json!({
                "provider": "okx_historical_market_data",
                "module": "candlesticks",
                "date_aggr_type": historical_zip_aggr_type(range),
                "source_timeframe": BASE_CANDLE_TIMEFRAME,
            })
        } else {
            Value::Null
        },
    })
}

pub(super) fn repair_method_for_gap(
    range: &GapRange,
    timeframe: &str,
    timeframe_ms: i64,
    now_ms: i64,
) -> RepairDecision {
    let span_ms = range
        .end_ts
        .saturating_sub(range.start_ts)
        .saturating_add(timeframe_ms);
    let api_calls = paginated_api_calls_for_missing(range.missing_candles);
    if range.end_ts >= now_ms.saturating_sub(RECENT_TAIL_MS) {
        return RepairDecision {
            method: "paginated",
            reason: "近端缺口优先用分页接口补齐，历史 zip 文件通常有发布滞后",
        };
    }
    if span_ms >= HISTORICAL_ZIP_MIN_SPAN_MS
        || api_calls > PAGINATED_MAX_API_CALLS
        || range.missing_candles > PAGINATED_MAX_MISSING_CANDLES
    {
        return RepairDecision {
            method: "historical_zip",
            reason: if timeframe == BASE_CANDLE_TIMEFRAME {
                "大跨度历史缺口优先用 OKX 历史 zip 导入，避免大量分页请求"
            } else {
                "大跨度历史缺口优先导入 1m 历史 zip，再对齐目标周期"
            },
        };
    }
    RepairDecision {
        method: "paginated",
        reason: "小范围缺口用分页接口补齐，进度和失败重试更直接",
    }
}

pub(super) fn repair_decision_with_override(
    range: &GapRange,
    timeframe: &str,
    timeframe_ms: i64,
    now_ms: i64,
    method_override: &str,
) -> RepairDecision {
    match method_override {
        "paginated" => RepairDecision {
            method: "paginated",
            reason: "用户指定分页补齐",
        },
        "historical_zip" => RepairDecision {
            method: "historical_zip",
            reason: "用户指定历史 zip 补齐",
        },
        _ => repair_method_for_gap(range, timeframe, timeframe_ms, now_ms),
    }
}

pub(super) fn source_candles_for_repair(
    range: &GapRange,
    decision: &RepairDecision,
    target_timeframe: &str,
) -> i64 {
    let source_timeframe = source_timeframe_for_decision(decision, target_timeframe);
    let end_ts = source_end_ts_for_target_range(range, target_timeframe);
    expected_candles_in_range(
        align_down(range.start_ts, timeframe_to_ms(source_timeframe).max(1)),
        align_down(end_ts, timeframe_to_ms(source_timeframe).max(1)),
        timeframe_to_ms(source_timeframe).max(1),
    )
}

pub(super) fn historical_source_end_ts(range: &GapRange, target_timeframe: &str) -> i64 {
    source_end_ts_for_target_range(range, target_timeframe)
}

pub(super) fn source_end_ts_for_target_range(range: &GapRange, target_timeframe: &str) -> i64 {
    if target_timeframe == BASE_CANDLE_TIMEFRAME {
        return range.end_ts;
    }
    let target_ms = timeframe_to_ms(target_timeframe).max(1);
    let source_ms = timeframe_to_ms(BASE_CANDLE_TIMEFRAME).max(1);
    range
        .end_ts
        .saturating_add(target_ms)
        .saturating_sub(source_ms)
}

pub(super) fn source_timeframe_for_target(target_timeframe: &str) -> String {
    if target_timeframe == BASE_CANDLE_TIMEFRAME {
        target_timeframe.to_string()
    } else {
        BASE_CANDLE_TIMEFRAME.to_string()
    }
}

pub(super) fn source_timeframe_for_decision<'a>(
    decision: &RepairDecision,
    target_timeframe: &'a str,
) -> &'a str {
    if decision.method == "historical_zip" || target_timeframe != BASE_CANDLE_TIMEFRAME {
        BASE_CANDLE_TIMEFRAME
    } else {
        target_timeframe
    }
}

pub(super) fn source_timeframe_for_repair_totals(
    target_timeframe: &str,
    historical_zip_ranges: i64,
) -> String {
    if historical_zip_ranges > 0 || target_timeframe != BASE_CANDLE_TIMEFRAME {
        BASE_CANDLE_TIMEFRAME.to_string()
    } else {
        target_timeframe.to_string()
    }
}

fn paginated_api_calls_for_missing(missing_candles: i64) -> i64 {
    if missing_candles <= 0 {
        0
    } else {
        (missing_candles + i64::from(OKX_CANDLE_BATCH_LIMIT) - 1)
            / i64::from(OKX_CANDLE_BATCH_LIMIT)
    }
}

pub(super) fn historical_zip_aggr_type(range: &GapRange) -> &'static str {
    if range.end_ts.saturating_sub(range.start_ts) >= 90 * DAY_MS {
        "monthly"
    } else {
        "daily"
    }
}

pub(super) fn expected_candles_in_range(start_ts: i64, end_ts: i64, timeframe_ms: i64) -> i64 {
    if timeframe_ms <= 0 || end_ts < start_ts {
        return 0;
    }
    end_ts
        .saturating_sub(start_ts)
        .saturating_div(timeframe_ms)
        .saturating_add(1)
}

pub(in crate::commands::local_api::market_ops) fn latest_complete_candle_ts(
    now_ms: i64,
    timeframe_ms: i64,
) -> i64 {
    align_down(now_ms, timeframe_ms).saturating_sub(timeframe_ms.max(1))
}

pub(super) fn align_down(timestamp_ms: i64, timeframe_ms: i64) -> i64 {
    timestamp_ms.saturating_sub(timestamp_ms.rem_euclid(timeframe_ms.max(1)))
}

pub(super) fn merge_repair_totals(totals: &mut GapRepairTotals, report: &GapRangeRepairReport) {
    totals.fetched_count += report.fetched_count;
    totals.saved_count += report.saved_count;
    totals.derived_count += report.derived_count;
    totals.api_calls += report.api_calls;
    totals.zip_files += report.zip_files;
    totals.zip_rows_seen += report.zip_rows_seen;
    totals.zip_rows_imported += report.zip_rows_imported;
    if report.method == "historical_zip" {
        totals.historical_zip_ranges += 1;
    } else {
        totals.paginated_ranges += 1;
    }
}

pub(super) fn repair_report_value(report: &GapRangeRepairReport) -> Value {
    json!({
        "method": report.method,
        "start_ts": report.start_ts,
        "end_ts": report.end_ts,
        "start_time": ts_to_iso(Some(report.start_ts)),
        "end_time": ts_to_iso(Some(report.end_ts)),
        "missing_candles": report.missing_candles,
        "fetch_timeframe": report.fetch_timeframe,
        "fetched_count": report.fetched_count,
        "saved_count": report.saved_count,
        "derived_count": report.derived_count,
        "api_calls": report.api_calls,
        "zip_files": report.zip_files,
        "zip_rows_seen": report.zip_rows_seen,
        "zip_rows_imported": report.zip_rows_imported,
    })
}
