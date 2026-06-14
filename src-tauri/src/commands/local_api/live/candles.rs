mod db;
mod json;
mod merge;

#[cfg(test)]
mod tests;

use crate::commands::local_api::market_ops::normalize_timeframe_name;

pub(in crate::commands::local_api::live) use db::load_latest_diagnostic_candles;
pub(in crate::commands::local_api::live) use json::candle_to_json;
pub(in crate::commands::local_api::live) use merge::merge_latest_diagnostic_candle;

fn canonical_diagnostic_timeframe(value: &str) -> String {
    let normalized = normalize_timeframe_name(value);
    if normalized.is_empty() {
        value.trim().to_string()
    } else {
        normalized
    }
}
