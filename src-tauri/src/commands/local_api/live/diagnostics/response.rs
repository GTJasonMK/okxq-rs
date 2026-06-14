use serde_json::{json, Value};

use crate::strategy_engine::StrategyConfig;

pub(super) fn enrich_strategy_output(
    mut value: Value,
    config: &StrategyConfig,
    candle_count: usize,
    realtime_candle_applied: bool,
) -> Value {
    if let Some(obj) = value.as_object_mut() {
        obj.insert("strategy_id".to_string(), json!(config.strategy_id));
        obj.insert("strategy_name".to_string(), json!(config.strategy_name));
        obj.insert("symbol".to_string(), json!(config.symbol));
        obj.insert("inst_type".to_string(), json!(config.inst_type));
        obj.insert("timeframe".to_string(), json!(config.timeframe));
        obj.insert("candle_count".to_string(), json!(candle_count));
        obj.insert(
            "realtime_candle_applied".to_string(),
            json!(realtime_candle_applied),
        );
    }
    value
}
