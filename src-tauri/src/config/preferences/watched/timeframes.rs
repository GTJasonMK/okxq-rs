use crate::timeframes::{normalize_okx_timeframe, okx_timeframe_order};

pub(super) fn normalize_timeframe(value: &str) -> Option<String> {
    normalize_okx_timeframe(value).map(ToOwned::to_owned)
}

pub(super) fn timeframe_order(value: &str) -> i64 {
    okx_timeframe_order(value)
}
