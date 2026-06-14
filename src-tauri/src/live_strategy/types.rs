use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct LiveStrategyConfig {
    pub strategy_id: String,
    pub strategy_name: String,
    pub symbol: String,
    pub timeframe: String,
    pub inst_type: String,
    pub mode: String,
    pub initial_capital: f64,
    pub position_size: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub risk_timeframe: String,
    pub check_interval: u64,
    pub params: Value,
    pub project_root: PathBuf,
    pub risk_control_enabled: bool,
    pub max_single_loss_ratio: f64,
    pub max_position_pct: f64,
    pub max_order_value: f64,
}

impl LiveStrategyConfig {
    pub(crate) fn runtime_execution_mode(&self) -> &'static str {
        if self.mode.eq_ignore_ascii_case("live") {
            "exchange_live"
        } else {
            "exchange_demo"
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LiveExecutionLogEntry {
    pub seq: u64,
    pub run_id: String,
    pub mode: String,
    pub strategy_id: String,
    pub strategy_name: String,
    pub symbol: String,
    pub inst_type: String,
    pub timeframe: String,
    pub timestamp_ms: i64,
    pub time: String,
    pub stage: String,
    pub level: String,
    pub message: String,
    pub details: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct LiveStrategyStatus {
    pub status: String,
    pub run_id: String,
    pub mode: String,
    pub strategy_id: String,
    pub strategy_name: String,
    pub symbol: String,
    pub timeframe: String,
    pub inst_type: String,
    pub initial_capital: f64,
    pub position_size: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub params: Value,
    pub start_time: Option<String>,
    pub last_action_time: Option<String>,
    pub last_action: String,
    pub total_actions: i64,
    pub total_orders: i64,
    pub successful_orders: i64,
    pub failed_orders: i64,
    pub error_message: String,
    pub check_interval: u64,
    pub risk_timeframe: String,
    pub execution_mode: String,
    pub last_price: Option<f64>,
    pub last_action_strength: Option<f64>,
    pub last_action_reason: String,
    pub last_order_candle_ts: Option<i64>,
}

impl Default for LiveStrategyStatus {
    fn default() -> Self {
        Self {
            status: "stopped".to_string(),
            run_id: String::new(),
            mode: String::new(),
            strategy_id: String::new(),
            strategy_name: String::new(),
            symbol: String::new(),
            timeframe: String::new(),
            inst_type: "SPOT".to_string(),
            initial_capital: 0.0,
            position_size: 0.0,
            stop_loss: 0.0,
            take_profit: 0.0,
            params: Value::Null,
            start_time: None,
            last_action_time: None,
            last_action: String::new(),
            total_actions: 0,
            total_orders: 0,
            successful_orders: 0,
            failed_orders: 0,
            error_message: String::new(),
            check_interval: 60,
            risk_timeframe: "1m".to_string(),
            execution_mode: "exchange_demo".to_string(),
            last_price: None,
            last_action_strength: None,
            last_action_reason: String::new(),
            last_order_candle_ts: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_status_does_not_fabricate_last_action_observations() {
        let value =
            serde_json::to_value(LiveStrategyStatus::default()).expect("status should serialize");

        assert!(value["last_price"].is_null());
        assert!(value["last_action_strength"].is_null());
    }
}
