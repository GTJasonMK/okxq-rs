use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use crate::{error::AppResult, okx::OkxCandle, trading_semantics::configured_leverage_from_params};

use super::super::types::{StrategyConfig, TradeRecord};

mod context;
mod execution;
mod exits;
mod market_data;
mod pricing;
mod quantity;
mod settings;
mod state;
mod submission;
#[cfg(test)]
mod tests;
mod values;

pub use market_data::{
    HistoricalCandleSeries, HistoricalFundingPoint, HistoricalFundingSeries, HistoricalMarketData,
};

use settings::SimSettings;
use state::{PlannedExit, SimOrder, SimPosition};

pub struct HistoricalLiveBacktest {
    config: StrategyConfig,
    days: i64,
    candles: Vec<OkxCandle>,
    settings: SimSettings,
    cash: f64,
    orders: Vec<SimOrder>,
    positions: HashMap<(String, String), SimPosition>,
    planned_exits: Vec<PlannedExit>,
    fills: Vec<Value>,
    rejected_orders: Vec<Value>,
    trade_records: Vec<TradeRecord>,
    funding_events: Vec<Value>,
    funding_missing_series: HashSet<(String, String)>,
    funding_mark_price_fallbacks: u64,
    planned_exit_reference_price_fallbacks: u64,
    total_funding: f64,
    equity_curve: Vec<Value>,
    equity_returns: Vec<f64>,
    previous_equity: f64,
    peak_equity: f64,
    max_drawdown: f64,
    day_start_equity: f64,
    trading_day: String,
    next_order_seq: u64,
    submitted_action_keys: HashSet<String>,
    strategy_actions: Vec<Value>,
    skipped_actions: Vec<Value>,
    execution_logs: Vec<Value>,
    strategy_diagnostics: Value,
    indicators: Value,
}

impl HistoricalLiveBacktest {
    #[cfg(test)]
    pub fn new(config: &StrategyConfig, candles: &[OkxCandle], days: i64) -> Self {
        Self::try_new(config, candles, days)
            .unwrap_or_else(|error| panic!("invalid historical live backtest config: {error}"))
    }

    pub fn try_new(config: &StrategyConfig, candles: &[OkxCandle], days: i64) -> AppResult<Self> {
        let mut candles = candles
            .iter()
            .filter(|item| item.is_valid_market_candle())
            .cloned()
            .collect::<Vec<_>>();
        candles.sort_by_key(|item| item.timestamp);
        let settings = SimSettings::try_from_config(config)?;
        let initial_capital = settings.initial_capital;
        Ok(Self {
            config: config.clone(),
            days,
            candles,
            settings,
            cash: initial_capital,
            orders: Vec::new(),
            positions: HashMap::new(),
            planned_exits: Vec::new(),
            fills: Vec::new(),
            rejected_orders: Vec::new(),
            trade_records: Vec::new(),
            funding_events: Vec::new(),
            funding_missing_series: HashSet::new(),
            funding_mark_price_fallbacks: 0,
            planned_exit_reference_price_fallbacks: 0,
            total_funding: 0.0,
            equity_curve: Vec::new(),
            equity_returns: Vec::new(),
            previous_equity: initial_capital,
            peak_equity: initial_capital,
            max_drawdown: 0.0,
            day_start_equity: initial_capital,
            trading_day: String::new(),
            next_order_seq: 1,
            submitted_action_keys: HashSet::new(),
            strategy_actions: Vec::new(),
            skipped_actions: Vec::new(),
            execution_logs: Vec::new(),
            strategy_diagnostics: json!({}),
            indicators: json!({}),
        })
    }

    fn next_order_id(&mut self) -> String {
        let value = format!("bt-{}", self.next_order_seq);
        self.next_order_seq = self.next_order_seq.saturating_add(1);
        value
    }

    pub(super) fn configured_leverage_for_inst_type(&self, inst_type: &str) -> AppResult<f64> {
        Ok(configured_leverage_from_params(&self.config.params, inst_type, "回测")?.unwrap_or(1.0))
    }
}
