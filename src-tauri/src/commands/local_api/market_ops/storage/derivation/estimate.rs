use std::collections::BTreeSet;

use crate::okx::OkxCandle;

use super::super::super::DERIVED_CANDLE_TIMEFRAMES;
use super::aggregate::candle_bucket_start;

pub(super) fn estimate_total_derived_count(
    base_candles: &[OkxCandle],
    source_timeframe: &str,
    target_timeframes: &[String],
) -> i64 {
    target_timeframes
        .iter()
        .filter(|timeframe| timeframe.as_str() != source_timeframe)
        .filter(|timeframe| DERIVED_CANDLE_TIMEFRAMES.contains(&timeframe.as_str()))
        .map(|timeframe| estimate_derived_count(base_candles, timeframe))
        .sum()
}

fn estimate_derived_count(base_candles: &[OkxCandle], timeframe: &str) -> i64 {
    let mut buckets = BTreeSet::<i64>::new();
    for candle in base_candles {
        if let Some(bucket_start) = candle_bucket_start(candle.timestamp, timeframe) {
            buckets.insert(bucket_start);
        }
    }
    buckets.len() as i64
}
