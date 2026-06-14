use crate::timeframes::{normalize_okx_timeframe, sort_okx_timeframes};

pub(in crate::commands::local_api::market_ops) fn normalize_target_timeframes(
    values: Vec<String>,
) -> Vec<String> {
    let mut values = values
        .into_iter()
        .map(|value| normalize_timeframe_name(&value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    sort_timeframes(&mut values);
    values
}

pub(in crate::commands::local_api) fn normalize_timeframe_name(value: &str) -> String {
    normalize_okx_timeframe(value).unwrap_or("").to_string()
}

pub(in crate::commands::local_api::market_ops) fn sort_timeframes(values: &mut Vec<String>) {
    sort_okx_timeframes(values);
}
