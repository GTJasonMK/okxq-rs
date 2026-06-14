use serde_json::Value;

use crate::{
    live_strategy::required_action_candle_count_for_timeframe, strategy_engine::StrategyConfig,
};

pub(super) fn strategy_config_json(config: &StrategyConfig) -> Value {
    crate::live_strategy::strategy_config_json_for_evaluate(config)
}

pub(super) fn diagnostic_candle_count(params: &Value, timeframe: &str) -> usize {
    required_action_candle_count_for_timeframe(params, timeframe)
}
