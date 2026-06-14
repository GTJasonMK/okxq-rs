use sqlx::SqlitePool;

use crate::sync_jobs::SyncJobRequest;

use super::super::*;

pub(super) async fn select_read_sync_request(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
    required_start_ts: Option<i64>,
    force_refresh: bool,
) -> AppResult<Option<SyncJobRequest>> {
    let (oldest_timestamp, newest_timestamp, candle_count) =
        candle_stats(pool, inst_id, inst_type, timeframe).await?;
    let sync_record = get_sync_record_stats(pool, inst_id, inst_type, timeframe).await?;
    let history_complete = sync_record
        .as_ref()
        .map(|record| record.history_complete)
        .unwrap_or(false);
    let count_hint = limit.max(0);
    let bootstrap_days =
        estimate_days_for_candle_count(timeframe, count_hint.max(300), 7).clamp(1, 3650);
    let full_days = bootstrap_days.max(30).clamp(1, 3650);
    let target_timeframes = vec![normalize_timeframe_name(timeframe)];
    let can_derive_from_base = !force_refresh
        && timeframe != BASE_CANDLE_TIMEFRAME
        && base_candles_cover_target(
            pool,
            inst_id,
            inst_type,
            timeframe,
            limit,
            required_start_ts,
        )
        .await?;

    if candle_count <= 0 || oldest_timestamp.is_none() || newest_timestamp.is_none() {
        let mode = if can_derive_from_base {
            "derive"
        } else {
            "full"
        };
        return Ok(Some(sync_request(
            inst_id,
            inst_type,
            timeframe,
            mode,
            full_days,
            target_timeframes,
        )?));
    }

    if !history_complete && count_hint > 0 && candle_count < count_hint {
        let mode = if can_derive_from_base {
            "derive"
        } else {
            "full"
        };
        return Ok(Some(sync_request(
            inst_id,
            inst_type,
            timeframe,
            mode,
            full_days,
            target_timeframes,
        )?));
    }

    if let (Some(required_start_ts), Some(oldest_timestamp)) = (required_start_ts, oldest_timestamp)
    {
        if !history_complete && required_start_ts < oldest_timestamp {
            let mode = if can_derive_from_base {
                "derive"
            } else {
                "full"
            };
            return Ok(Some(sync_request(
                inst_id,
                inst_type,
                timeframe,
                mode,
                full_days,
                target_timeframes,
            )?));
        }
    }

    let timeframe_ms = timeframe_to_ms(timeframe);
    let newest = newest_timestamp.unwrap_or_default();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let latest_gap_detected = newest.saturating_add(2 * timeframe_ms) < now_ms;
    if force_refresh || latest_gap_detected {
        let mode = if can_derive_from_base {
            "derive"
        } else {
            "incremental"
        };
        return Ok(Some(sync_request(
            inst_id,
            inst_type,
            timeframe,
            mode,
            bootstrap_days.max(7),
            target_timeframes,
        )?));
    }

    Ok(None)
}

async fn base_candles_cover_target(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    target_timeframe: &str,
    limit: i64,
    required_start_ts: Option<i64>,
) -> AppResult<bool> {
    let Some((oldest, newest)) =
        local_candle_bounds(pool, inst_id, inst_type, BASE_CANDLE_TIMEFRAME).await?
    else {
        return Ok(false);
    };
    let now_ms = chrono::Utc::now().timestamp_millis();
    let latest_ok = required_start_ts.is_some()
        || newest.saturating_add(5 * timeframe_to_ms(BASE_CANDLE_TIMEFRAME)) >= now_ms;
    if !latest_ok {
        return Ok(false);
    }
    let required_start = required_start_ts.unwrap_or_else(|| {
        let days = estimate_days_for_candle_count(target_timeframe, limit.max(300), 1).max(1);
        now_ms.saturating_sub(days.saturating_mul(DAY_MS))
    });
    Ok(oldest <= required_start)
}
