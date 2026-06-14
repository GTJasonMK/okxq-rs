use super::super::*;

pub(super) fn planned_sync_targets(
    mode: &str,
    days: i64,
    source_timeframe: &str,
    target_timeframes: &[String],
) -> (i64, i64, i64, i64) {
    let target_fetch_count =
        estimated_target_fetch_count_for_timeframe(mode, days, source_timeframe);
    let target_save_count = target_fetch_count;
    let target_derive_count = estimated_planned_derive_count(
        mode,
        days,
        target_fetch_count,
        source_timeframe,
        target_timeframes,
    );
    let target_batches = estimated_batches_for_count(target_fetch_count);
    (
        target_fetch_count,
        target_save_count,
        target_derive_count,
        target_batches,
    )
}

fn estimated_planned_derive_count(
    mode: &str,
    days: i64,
    target_fetch_count: i64,
    source_timeframe: &str,
    target_timeframes: &[String],
) -> i64 {
    let base_span_ms = if mode == "full" && target_fetch_count > 0 {
        target_fetch_count.saturating_mul(timeframe_to_ms(source_timeframe).max(1))
    } else {
        days.max(1).saturating_mul(DAY_MS)
    };
    target_timeframes
        .iter()
        .filter(|timeframe| timeframe.as_str() != source_timeframe)
        .filter(|timeframe| DERIVED_CANDLE_TIMEFRAMES.contains(&timeframe.as_str()))
        .map(|timeframe| {
            let timeframe_ms = timeframe_to_ms(timeframe).max(1);
            base_span_ms
                .saturating_div(timeframe_ms)
                .saturating_add(1)
                .max(1)
        })
        .sum()
}
