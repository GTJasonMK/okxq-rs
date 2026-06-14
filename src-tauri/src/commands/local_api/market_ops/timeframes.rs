use super::*;

use crate::timeframes::normalized_okx_timeframe_to_ms;

pub(in crate::commands::local_api::market_ops) const OKX_CANDLE_BATCH_LIMIT: u32 = 300;
pub(in crate::commands::local_api::market_ops) const DAY_MS: i64 = 86_400_000;
pub(in crate::commands::local_api::market_ops) const BASE_CANDLE_TIMEFRAME: &str = "1m";
pub(in crate::commands::local_api::market_ops) const DERIVED_CANDLE_TIMEFRAMES: &[&str] = &[
    "3m", "5m", "15m", "30m", "1H", "2H", "4H", "6H", "12H", "1D", "1W", "1M",
];

pub(in crate::commands::local_api::market_ops) fn normalize_sync_mode(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "full" => "full".to_string(),
        "incremental" => "incremental".to_string(),
        _ => "window".to_string(),
    }
}

pub(in crate::commands::local_api::market_ops) fn required_timeframe_name(
    value: &str,
    field: &str,
) -> AppResult<String> {
    let normalized = normalize_timeframe_name(value);
    if normalized.is_empty() {
        return Err(AppError::Validation(format!(
            "不支持的 K 线周期 {field}={}",
            value.trim()
        )));
    }
    Ok(normalized)
}

pub(in crate::commands::local_api::market_ops) fn required_target_timeframes(
    values: Vec<String>,
) -> AppResult<Vec<String>> {
    let mut normalized = Vec::new();
    for value in values {
        if value.trim().is_empty() {
            continue;
        }
        normalized.push(required_timeframe_name(&value, "target_timeframes")?);
    }
    sort_timeframes(&mut normalized);
    Ok(normalized)
}

pub(in crate::commands::local_api::market_ops) fn timeframe_to_ms(timeframe: &str) -> i64 {
    normalized_okx_timeframe_to_ms(timeframe)
        .unwrap_or_else(|| unreachable!("unsupported normalized timeframe: {}", timeframe.trim()))
}

pub(in crate::commands::local_api) fn candle_coverage_metrics(
    timeframe: &str,
    oldest_timestamp: Option<i64>,
    newest_timestamp: Option<i64>,
    candle_count: i64,
) -> (i64, i64, f64) {
    let count = candle_count.max(0);
    let Some(oldest) = oldest_timestamp else {
        return (count, 0, if count > 0 { 1.0 } else { 0.0 });
    };
    let Some(newest) = newest_timestamp else {
        return (count, 0, if count > 0 { 1.0 } else { 0.0 });
    };
    let normalized_timeframe = normalize_timeframe_name(timeframe);
    if normalized_timeframe.is_empty() || newest < oldest {
        return (count, 0, if count > 0 { 1.0 } else { 0.0 });
    }
    let timeframe_ms = timeframe_to_ms(&normalized_timeframe);
    let expected = newest
        .saturating_sub(oldest)
        .saturating_div(timeframe_ms.max(1))
        .saturating_add(1)
        .max(count);
    let gap_count = expected.saturating_sub(count).max(0);
    let coverage_ratio = if expected > 0 {
        (count.min(expected) as f64 / expected as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (expected, gap_count, coverage_ratio)
}

pub(in crate::commands::local_api::market_ops) fn estimate_days_for_candle_count(
    timeframe: &str,
    count: i64,
    minimum_days: i64,
) -> i64 {
    let count = count.max(1);
    let timeframe_ms = timeframe_to_ms(timeframe).max(1);
    let estimated = (count.saturating_mul(timeframe_ms) + DAY_MS - 1) / DAY_MS;
    estimated.max(minimum_days.max(1))
}

pub(in crate::commands::local_api::market_ops) fn estimated_target_fetch_count_for_timeframe(
    mode: &str,
    days: i64,
    timeframe: &str,
) -> i64 {
    match mode {
        "derive" => 0,
        "full" => (max_sync_batches() as i64) * i64::from(OKX_CANDLE_BATCH_LIMIT),
        _ => days
            .max(1)
            .saturating_mul(DAY_MS)
            .saturating_div(timeframe_to_ms(timeframe).max(1))
            .saturating_add(1),
    }
}

pub(in crate::commands::local_api::market_ops) fn estimated_incremental_fetch_count(
    newest: i64,
    now_ms: i64,
    timeframe_ms: i64,
) -> i64 {
    if timeframe_ms <= 0 || now_ms <= newest {
        return 0;
    }
    ((now_ms - newest) / timeframe_ms).max(0)
}

pub(in crate::commands::local_api::market_ops) fn estimated_batches_for_count(count: i64) -> i64 {
    if count <= 0 {
        0
    } else {
        ((count + i64::from(OKX_CANDLE_BATCH_LIMIT) - 1) / i64::from(OKX_CANDLE_BATCH_LIMIT))
            .clamp(1, max_sync_batches() as i64)
    }
}
