use std::sync::Mutex;

use serde_json::{json, Value};

use crate::{
    commands::local_api::live::round6, okx::OkxCandle, strategy_engine::StrategyConfig,
    strategy_executor::types::RuntimeStrategyAction,
};

#[derive(Clone)]
struct LiveComputationCacheEntry {
    key: String,
    decision: CachedDiagnosticDecision,
}

#[derive(Clone)]
pub(super) struct CachedDiagnosticDecision {
    pub(super) actions: Vec<RuntimeStrategyAction>,
    pub(super) diagnostics: Value,
}

static DECISION_DIAGNOSTICS_CACHE: Mutex<Option<LiveComputationCacheEntry>> = Mutex::new(None);

pub(super) fn decision_cache_get(key: &str) -> Option<CachedDiagnosticDecision> {
    computation_cache_get(&DECISION_DIAGNOSTICS_CACHE, key)
}

pub(super) fn decision_cache_set(key: String, decision: &CachedDiagnosticDecision) {
    computation_cache_set(&DECISION_DIAGNOSTICS_CACHE, key, decision);
}

pub(super) fn live_computation_cache_key(
    config: &StrategyConfig,
    candles: &[OkxCandle],
    source_stamp: &str,
    context_stamp: Option<&Value>,
) -> String {
    let first = candles.first();
    let last = candles.last();
    serde_json::to_string(&json!({
        "source": source_stamp,
        "strategy_id": &config.strategy_id,
        "symbol": &config.symbol,
        "inst_type": &config.inst_type,
        "timeframe": &config.timeframe,
        "initial_capital": config.initial_capital,
        "position_size": config.position_size,
        "stop_loss": config.stop_loss,
        "take_profit": config.take_profit,
        "params": &config.params,
        "context": context_stamp,
        "candles": {
            "len": candles.len(),
            "first_ts": first.map(|candle| candle.timestamp),
            "last_ts": last.map(|candle| candle.timestamp),
            "last_open": last.map(|candle| round6(candle.open)),
            "last_high": last.map(|candle| round6(candle.high)),
            "last_low": last.map(|candle| round6(candle.low)),
            "last_close": last.map(|candle| round6(candle.close)),
            "last_volume": last.map(|candle| round6(candle.volume)),
            "last_confirm": last.map(|candle| candle.confirm.as_str()),
        },
    }))
    .expect("live diagnostic cache key should serialize")
}

pub(super) fn runtime_strategy_source_stamp(
    project_root: &std::path::Path,
    file_name: &str,
) -> String {
    let path = project_root.join("strategies").join(file_name);
    let Ok(metadata) = std::fs::metadata(path) else {
        return format!("strategy:{file_name}:missing");
    };
    let modified_ms = metadata
        .modified()
        .expect("strategy source modified time should be available")
        .duration_since(std::time::UNIX_EPOCH)
        .expect("strategy source modified time should not predate unix epoch")
        .as_millis();
    format!("strategy:{file_name}:{}:{modified_ms}", metadata.len())
}

fn computation_cache_get(
    cache: &Mutex<Option<LiveComputationCacheEntry>>,
    key: &str,
) -> Option<CachedDiagnosticDecision> {
    let guard = cache
        .lock()
        .expect("live diagnostic cache mutex should not be poisoned");
    guard
        .as_ref()
        .filter(|entry| entry.key == key)
        .map(|entry| entry.decision.clone())
}

fn computation_cache_set(
    cache: &Mutex<Option<LiveComputationCacheEntry>>,
    key: String,
    decision: &CachedDiagnosticDecision,
) {
    let mut guard = cache
        .lock()
        .expect("live diagnostic cache mutex should not be poisoned");
    *guard = Some(LiveComputationCacheEntry {
        key,
        decision: decision.clone(),
    });
}
