use std::collections::BTreeMap;

use chrono::{Datelike, TimeZone, Weekday};

use crate::{
    error::{AppError, AppResult},
    live_strategy::runtime_helpers::{canonical_timeframe, timeframe_millis},
    okx::OkxCandle,
};

use super::MAX_DERIVED_SOURCE_CANDLES;

pub(super) fn source_candles_required_for_target(
    target: usize,
    source_timeframe: &str,
    timeframe: &str,
) -> AppResult<usize> {
    let target_ms = derivation_timeframe_millis(timeframe);
    let source_ms = derivation_timeframe_millis(source_timeframe);
    if target_ms <= 0 || source_ms <= 0 {
        return Err(AppError::Validation(format!(
            "不支持从 {} 派生 K 线周期 {}",
            source_timeframe, timeframe
        )));
    }
    if target_ms < source_ms || target_ms % source_ms != 0 {
        return Err(AppError::Validation(format!(
            "不支持从 {} 派生 K 线周期 {}，源周期不能大于目标周期且必须能整齐对齐目标周期",
            source_timeframe, timeframe
        )));
    }
    let ratio = usize::try_from((target_ms / source_ms).max(1)).unwrap_or(usize::MAX);
    let required = target.saturating_mul(ratio).saturating_add(ratio);
    if required > MAX_DERIVED_SOURCE_CANDLES {
        return Err(AppError::Validation(format!(
            "{} 派生 {} 根 {} K线需要至少 {} 根 {} 源 K线，超过实时上下文源数据上限 {}；请先用数据中心补齐派生数据",
            timeframe, target, timeframe, required, source_timeframe, MAX_DERIVED_SOURCE_CANDLES
        )));
    }
    Ok(required.max(3))
}

fn derivation_timeframe_millis(timeframe: &str) -> i64 {
    match timeframe {
        "1M" => 30 * 86_400_000,
        other => timeframe_millis(other),
    }
}

pub(super) fn aggregate_candles_from_source(
    source_candles: &[OkxCandle],
    timeframe: &str,
) -> Vec<OkxCandle> {
    let mut buckets = BTreeMap::<i64, OkxCandle>::new();
    for candle in source_candles {
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
    buckets.into_values().collect()
}

fn candle_bucket_start(timestamp: i64, timeframe: &str) -> Option<i64> {
    let timeframe = canonical_timeframe(timeframe)?;
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
    let timeframe_ms = timeframe_millis(timeframe);
    if timeframe_ms <= 0 {
        return None;
    }
    Some(timestamp - timestamp.rem_euclid(timeframe_ms))
}
