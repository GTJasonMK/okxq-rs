use std::collections::BTreeMap;

use crate::{live_strategy::runtime_helpers::timeframe_millis, okx::OkxCandle};

pub(super) fn latest_local_candle_is_fresh(candles: &[OkxCandle], timeframe: &str) -> bool {
    let Some(latest) = candles.last() else {
        return false;
    };
    latest_timestamp_is_fresh(latest.timestamp, timeframe)
}

pub(super) fn latest_map_candle_is_fresh(
    candles: &BTreeMap<i64, OkxCandle>,
    timeframe: &str,
) -> bool {
    let Some(latest) = candles.keys().next_back().copied() else {
        return false;
    };
    latest_timestamp_is_fresh(latest, timeframe)
}

pub(super) fn latest_timestamp_is_fresh(timestamp: i64, timeframe: &str) -> bool {
    let timeframe_ms = timeframe_millis(timeframe);
    if timeframe_ms <= 0 {
        return false;
    }
    timestamp.saturating_add(timeframe_ms.saturating_mul(2))
        >= chrono::Utc::now().timestamp_millis()
}

pub(super) fn tail_candles(by_ts: BTreeMap<i64, OkxCandle>, target: usize) -> Vec<OkxCandle> {
    let mut candles = by_ts.into_values().collect::<Vec<_>>();
    if candles.len() > target {
        candles.drain(0..candles.len() - target);
    }
    candles
}
