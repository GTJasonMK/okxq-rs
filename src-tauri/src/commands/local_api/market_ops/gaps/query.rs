use serde_json::{json, Value};
use sqlx::{
    query::Query,
    sqlite::{SqliteArguments, SqliteRow},
    Row, Sqlite, SqlitePool,
};

use super::repair_plan::{expected_candles_in_range, gap_range_value, GapRange};
use super::*;

#[derive(Clone, Debug, Default)]
pub(super) struct InternalGapSummary {
    pub(super) events: i64,
    pub(super) missing_candles: i64,
    pub(super) max_gap_ms: i64,
}

#[derive(Clone, Debug, Default)]
pub(super) struct SeriesStats {
    pub(super) available_candles: i64,
    pub(super) oldest_timestamp: Option<i64>,
    pub(super) newest_timestamp: Option<i64>,
}

#[derive(Clone, Debug, Default)]
struct InternalGapAnalysis {
    summary: InternalGapSummary,
    ranges: Vec<GapRange>,
}

type GapQuery<'q> = Query<'q, Sqlite, SqliteArguments<'q>>;

const VALID_CANDLE_SCOPE_SQL: &str = r#"
          FROM candles INDEXED BY idx_candles_query
          WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
            AND timestamp BETWEEN ? AND ?
            AND timestamp > 0
            AND typeof(open) IN ('integer', 'real') AND open > 0
            AND typeof(high) IN ('integer', 'real') AND high > 0
            AND typeof(low) IN ('integer', 'real') AND low > 0
            AND typeof(close) IN ('integer', 'real') AND close > 0
            AND typeof(volume) IN ('integer', 'real') AND volume >= 0
            AND typeof(volume_ccy) IN ('integer', 'real') AND volume_ccy >= 0
"#;

fn internal_gap_ordered_cte_sql() -> String {
    format!(
        r#"
        WITH ordered AS (
          SELECT timestamp,
                 LAG(timestamp) OVER (ORDER BY timestamp) AS prev_ts
          {VALID_CANDLE_SCOPE_SQL}
        ),
"#
    )
}

fn bind_candle_scope<'q>(
    query: GapQuery<'q>,
    inst_id: &'q str,
    inst_type: &'q str,
    timeframe: &'q str,
    start_ts: i64,
    end_ts: i64,
) -> GapQuery<'q> {
    query
        .bind(inst_id)
        .bind(inst_type)
        .bind(timeframe)
        .bind(start_ts)
        .bind(end_ts)
}

fn bind_request_candle_scope<'q>(query: GapQuery<'q>, request: &'q GapPlanRequest) -> GapQuery<'q> {
    bind_candle_scope(
        query,
        &request.inst_id,
        &request.inst_type,
        &request.timeframe,
        request.start_ts,
        request.end_ts,
    )
}

fn internal_gap_range_from_row(row: &SqliteRow, timeframe_ms: i64) -> AppResult<Option<GapRange>> {
    let prev_ts = row.try_get::<i64, _>("prev_ts")?;
    let next_ts = row.try_get::<i64, _>("next_ts")?;
    let missing_candles = row.try_get::<i64, _>("missing_candles")?;
    let start_ts = prev_ts.saturating_add(timeframe_ms);
    let end_ts = next_ts.saturating_sub(timeframe_ms);
    Ok(
        (missing_candles > 0 && end_ts >= start_ts).then_some(GapRange {
            start_ts,
            end_ts,
            missing_candles,
        }),
    )
}

pub(super) async fn load_repair_gap_ranges(
    pool: &SqlitePool,
    request: &GapPlanRequest,
    timeframe_ms: i64,
) -> AppResult<Vec<GapRange>> {
    let stats = load_gap_series_stats(pool, request).await?;
    let mut ranges = edge_gap_ranges(request, timeframe_ms, &stats);
    if range_fully_covered(request, timeframe_ms, &stats) {
        return Ok(ranges);
    }
    let internal_limit = request.limit.saturating_sub(ranges.len() as i64).max(0);
    ranges.extend(load_internal_gap_ranges(pool, request, timeframe_ms, internal_limit).await?);
    ranges.sort_by_key(|range| range.start_ts);
    Ok(ranges)
}

pub(super) async fn build_gap_plan(
    pool: &SqlitePool,
    request: &GapPlanRequest,
) -> AppResult<Value> {
    let timeframe_ms = timeframe_to_ms(&request.timeframe).max(1);
    let expected_candles =
        expected_candles_in_range(request.start_ts, request.end_ts, timeframe_ms);
    let stats = load_gap_series_stats(pool, request).await?;
    let mut gap_ranges = edge_gap_ranges(request, timeframe_ms, &stats);
    let edge_gap_count = gap_ranges.len() as i64;
    let edge_missing_candles = gap_ranges
        .iter()
        .map(|range| range.missing_candles)
        .sum::<i64>();
    let internal_limit = request.limit.saturating_sub(edge_gap_count).max(0);
    let internal_analysis = if range_fully_covered(request, timeframe_ms, &stats) {
        InternalGapAnalysis::default()
    } else {
        load_internal_gap_analysis(pool, request, timeframe_ms, internal_limit).await?
    };
    let returned_gap_count = edge_gap_count + internal_analysis.ranges.len() as i64;
    gap_ranges.extend(internal_analysis.ranges);
    gap_ranges.sort_by_key(|range| range.start_ts);

    let missing_candles = if stats.available_candles <= 0 {
        expected_candles
    } else {
        edge_missing_candles + internal_analysis.summary.missing_candles
    }
    .clamp(0, expected_candles);
    let gap_event_count = if missing_candles > 0 && stats.available_candles <= 0 {
        1
    } else {
        edge_gap_count + internal_analysis.summary.events
    };
    let coverage_ratio = if expected_candles > 0 {
        (expected_candles.saturating_sub(missing_candles) as f64 / expected_candles as f64)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    let returned_missing_candles = gap_ranges
        .iter()
        .map(|range| range.missing_candles)
        .sum::<i64>();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let gap_values = gap_ranges
        .iter()
        .map(|range| gap_range_value(range, &request.timeframe, timeframe_ms, now_ms))
        .collect::<Vec<_>>();
    let paginated_ranges = gap_values
        .iter()
        .filter(|item| item.get("method").and_then(Value::as_str) == Some("paginated"))
        .count() as i64;
    let historical_zip_ranges = gap_values
        .iter()
        .filter(|item| item.get("method").and_then(Value::as_str) == Some("historical_zip"))
        .count() as i64;
    let truncated = returned_gap_count < gap_event_count;

    Ok(json!({
        "inst_id": request.inst_id,
        "inst_type": request.inst_type,
        "timeframe": request.timeframe,
        "source_timeframe": if request.timeframe == BASE_CANDLE_TIMEFRAME { request.timeframe.as_str() } else { BASE_CANDLE_TIMEFRAME },
        "target_timeframes": [request.timeframe.clone()],
        "range": {
            "start_ts": request.start_ts,
            "end_ts": request.end_ts,
            "start_time": ts_to_iso(Some(request.start_ts)),
            "end_time": ts_to_iso(Some(request.end_ts)),
        },
        "local_range": {
            "oldest_timestamp": stats.oldest_timestamp,
            "newest_timestamp": stats.newest_timestamp,
            "oldest_time": ts_to_iso(stats.oldest_timestamp),
            "newest_time": ts_to_iso(stats.newest_timestamp),
        },
        "expected_candles": expected_candles,
        "available_candles": stats.available_candles,
        "missing_candles": missing_candles,
        "coverage_ratio": coverage_ratio,
        "gap_event_count": gap_event_count,
        "returned_gap_count": returned_gap_count,
        "returned_missing_candles": returned_missing_candles,
        "truncated": truncated,
        "max_internal_gap_ms": internal_analysis.summary.max_gap_ms,
        "methods": {
            "paginated_ranges": paginated_ranges,
            "historical_zip_ranges": historical_zip_ranges,
        },
        "gaps": gap_values,
    }))
}

pub(super) async fn remaining_gap_candles(
    pool: &SqlitePool,
    request: &GapPlanRequest,
    timeframe_ms: i64,
) -> AppResult<i64> {
    let expected_candles =
        expected_candles_in_range(request.start_ts, request.end_ts, timeframe_ms);
    let stats = load_gap_series_stats(pool, request).await?;
    if stats.available_candles <= 0 {
        return Ok(expected_candles);
    }
    let edge_missing = edge_gap_ranges(request, timeframe_ms, &stats)
        .iter()
        .map(|range| range.missing_candles)
        .sum::<i64>();
    if range_fully_covered(request, timeframe_ms, &stats) {
        return Ok(edge_missing.clamp(0, expected_candles));
    }
    let internal_missing = load_internal_gap_analysis(pool, request, timeframe_ms, 0)
        .await?
        .summary
        .missing_candles;
    Ok(edge_missing
        .saturating_add(internal_missing)
        .clamp(0, expected_candles))
}

pub(super) async fn load_gap_series_stats(
    pool: &SqlitePool,
    request: &GapPlanRequest,
) -> AppResult<SeriesStats> {
    let sql = format!(
        r#"
        SELECT COUNT(*) AS available_candles,
               MIN(timestamp) AS oldest_timestamp,
               MAX(timestamp) AS newest_timestamp
        {VALID_CANDLE_SCOPE_SQL}
        "#
    );
    let row = bind_request_candle_scope(sqlx::query(&sql), request)
        .fetch_one(pool)
        .await?;
    Ok(SeriesStats {
        available_candles: row.try_get::<i64, _>("available_candles")?,
        oldest_timestamp: row.try_get::<Option<i64>, _>("oldest_timestamp")?,
        newest_timestamp: row.try_get::<Option<i64>, _>("newest_timestamp")?,
    })
}

fn range_fully_covered(request: &GapPlanRequest, timeframe_ms: i64, stats: &SeriesStats) -> bool {
    let expected_candles =
        expected_candles_in_range(request.start_ts, request.end_ts, timeframe_ms);
    expected_candles > 0
        && stats.available_candles == expected_candles
        && stats.oldest_timestamp == Some(request.start_ts)
        && stats.newest_timestamp == Some(request.end_ts)
}

pub(super) async fn local_candles_cover_range(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    start_ts: i64,
    end_ts: i64,
) -> AppResult<bool> {
    let timeframe_ms = timeframe_to_ms(timeframe).max(1);
    let expected = expected_candles_in_range(start_ts, end_ts, timeframe_ms);
    if expected <= 0 {
        return Ok(false);
    }
    let sql = format!(
        r#"
        SELECT COUNT(*) AS available_candles
        {VALID_CANDLE_SCOPE_SQL}
        "#
    );
    let row = bind_candle_scope(
        sqlx::query(&sql),
        inst_id,
        inst_type,
        timeframe,
        start_ts,
        end_ts,
    )
    .fetch_one(pool)
    .await?;
    let available = row.try_get::<i64, _>("available_candles")?;
    Ok(available >= expected)
}

pub(super) async fn load_internal_gap_ranges(
    pool: &SqlitePool,
    request: &GapPlanRequest,
    timeframe_ms: i64,
    limit: i64,
) -> AppResult<Vec<GapRange>> {
    if limit <= 0 {
        return Ok(Vec::new());
    }
    let ordered_cte = internal_gap_ordered_cte_sql();
    let sql = format!(
        r#"
        {ordered_cte}
        gaps AS (
          SELECT prev_ts, timestamp AS next_ts, timestamp - prev_ts AS diff_ms
          FROM ordered
          WHERE prev_ts IS NOT NULL AND timestamp - prev_ts > ?
        )
        SELECT prev_ts, next_ts, ((diff_ms / ?) - 1) AS missing_candles
        FROM gaps
        ORDER BY prev_ts
        LIMIT ?
        "#
    );
    let rows = bind_request_candle_scope(sqlx::query(&sql), request)
        .bind(timeframe_ms)
        .bind(timeframe_ms)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    let mut ranges = Vec::with_capacity(rows.len());
    for row in rows {
        if let Some(range) = internal_gap_range_from_row(&row, timeframe_ms)? {
            ranges.push(range);
        }
    }
    Ok(ranges)
}

async fn load_internal_gap_analysis(
    pool: &SqlitePool,
    request: &GapPlanRequest,
    timeframe_ms: i64,
    limit: i64,
) -> AppResult<InternalGapAnalysis> {
    let row_limit = limit.max(1);
    let ordered_cte = internal_gap_ordered_cte_sql();
    let sql = format!(
        r#"
        {ordered_cte}
        gaps AS (
          SELECT
            prev_ts,
            timestamp AS next_ts,
            timestamp - prev_ts AS diff_ms,
            ((timestamp - prev_ts) / ?) - 1 AS missing_candles
          FROM ordered
          WHERE prev_ts IS NOT NULL AND timestamp - prev_ts > ?
        ),
        numbered AS (
          SELECT
            prev_ts,
            next_ts,
            diff_ms,
            missing_candles,
            ROW_NUMBER() OVER (ORDER BY prev_ts) AS rn,
            COUNT(*) OVER () AS events,
            COALESCE(SUM(missing_candles) OVER (), 0) AS total_missing_candles,
            COALESCE(MAX(diff_ms) OVER (), 0) AS max_gap_ms
          FROM gaps
        )
        SELECT prev_ts, next_ts, missing_candles, rn,
               events, total_missing_candles, max_gap_ms
        FROM numbered
        WHERE rn <= ?
        ORDER BY prev_ts
        "#
    );
    let rows = bind_request_candle_scope(sqlx::query(&sql), request)
        .bind(timeframe_ms)
        .bind(timeframe_ms)
        .bind(row_limit)
        .fetch_all(pool)
        .await?;

    let Some(first) = rows.first() else {
        return Ok(InternalGapAnalysis::default());
    };
    let summary = InternalGapSummary {
        events: first.try_get::<i64, _>("events")?,
        missing_candles: first.try_get::<i64, _>("total_missing_candles")?,
        max_gap_ms: first.try_get::<i64, _>("max_gap_ms")?,
    };
    let mut ranges = Vec::with_capacity(rows.len());
    for row in rows {
        let rn = row.try_get::<i64, _>("rn")?;
        if rn > limit {
            continue;
        }
        if let Some(range) = internal_gap_range_from_row(&row, timeframe_ms)? {
            ranges.push(range);
        }
    }
    Ok(InternalGapAnalysis { summary, ranges })
}

pub(super) fn edge_gap_ranges(
    request: &GapPlanRequest,
    timeframe_ms: i64,
    stats: &SeriesStats,
) -> Vec<GapRange> {
    let Some(oldest) = stats.oldest_timestamp else {
        let missing_candles =
            expected_candles_in_range(request.start_ts, request.end_ts, timeframe_ms);
        return if missing_candles > 0 {
            vec![GapRange {
                start_ts: request.start_ts,
                end_ts: request.end_ts,
                missing_candles,
            }]
        } else {
            Vec::new()
        };
    };
    let Some(newest) = stats.newest_timestamp else {
        return Vec::new();
    };
    let mut ranges = Vec::new();
    if oldest > request.start_ts {
        let end_ts = oldest.saturating_sub(timeframe_ms);
        let missing_candles = expected_candles_in_range(request.start_ts, end_ts, timeframe_ms);
        if missing_candles > 0 {
            ranges.push(GapRange {
                start_ts: request.start_ts,
                end_ts,
                missing_candles,
            });
        }
    }
    if newest < request.end_ts {
        let start_ts = newest.saturating_add(timeframe_ms);
        let missing_candles = expected_candles_in_range(start_ts, request.end_ts, timeframe_ms);
        if missing_candles > 0 {
            ranges.push(GapRange {
                start_ts,
                end_ts: request.end_ts,
                missing_candles,
            });
        }
    }
    ranges
}
