use std::collections::BTreeMap;

use chrono::{Datelike, TimeZone, Weekday};

use crate::okx::OkxCandle;

use super::super::super::{
    check_sync_cancel, normalize_timeframe_name, timeframe_to_ms, AppResult, SyncCancelGuard,
    BASE_CANDLE_TIMEFRAME,
};

pub(super) async fn aggregate_candles(
    base_candles: &[OkxCandle],
    timeframe: &str,
    cancel_guard: Option<&SyncCancelGuard>,
) -> AppResult<Vec<OkxCandle>> {
    let mut buckets = BTreeMap::<i64, OkxCandle>::new();
    for (index, candle) in base_candles.iter().enumerate() {
        if index % 10_000 == 0 {
            check_sync_cancel(cancel_guard).await?;
        }
        let Some(bucket_start) = candle_bucket_start(candle.timestamp, timeframe) else {
            continue;
        };
        buckets
            .entry(bucket_start)
            .and_modify(|bucket| {
                bucket.high = bucket.high.max(candle.high);
                bucket.low = bucket.low.min(candle.low);
                bucket.close = candle.close;
                bucket.volume += candle.volume;
                bucket.volume_ccy += candle.volume_ccy;
                bucket.volume_quote += candle.volume_quote;
                bucket.confirm = candle.confirm.clone();
            })
            .or_insert_with(|| OkxCandle {
                timestamp: bucket_start,
                open: candle.open,
                high: candle.high,
                low: candle.low,
                close: candle.close,
                volume: candle.volume,
                volume_ccy: candle.volume_ccy,
                volume_quote: candle.volume_quote,
                confirm: candle.confirm.clone(),
            });
    }
    Ok(buckets.into_values().collect())
}

pub(super) fn candle_bucket_start(timestamp: i64, timeframe: &str) -> Option<i64> {
    let timeframe = normalize_timeframe_name(timeframe);
    if timeframe.is_empty() || timeframe == BASE_CANDLE_TIMEFRAME {
        return Some(timestamp);
    }
    if timeframe == "1M" {
        let dt = chrono::Utc.timestamp_millis_opt(timestamp).single()?;
        return chrono::Utc
            .with_ymd_and_hms(dt.year(), dt.month(), 1, 0, 0, 0)
            .single()
            .map(|value| value.timestamp_millis());
    }
    if timeframe == "1W" {
        let dt = chrono::Utc.timestamp_millis_opt(timestamp).single()?;
        let monday = dt.date_naive().week(Weekday::Mon).first_day();
        return monday
            .and_hms_opt(0, 0, 0)
            .map(|value| value.and_utc().timestamp_millis());
    }
    let timeframe_ms = timeframe_to_ms(&timeframe);
    if timeframe_ms <= 0 {
        return None;
    }
    Some(timestamp - timestamp.rem_euclid(timeframe_ms))
}
