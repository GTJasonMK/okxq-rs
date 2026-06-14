mod derivation;
mod fetch;
mod freshness;
mod persistence;
mod storage_kind;

#[cfg(test)]
mod tests;

const OKX_CANDLE_BATCH_LIMIT: u32 = 300;
const CANDLE_UPSERT_CHUNK_SIZE: usize = 500;
const LEGACY_DERIVATION_SOURCE_TIMEFRAME: &str = "1m";
const MAX_DERIVED_SOURCE_CANDLES: usize = 250_000;

pub(super) use fetch::fetch_candles_by_timeframe;
pub(in crate::live_strategy) use fetch::fetch_strategy_candles;
