use serde_json::{json, Map, Value};

use crate::{
    okx::OkxCandle,
    risk_controls::{self, PositionRiskSnapshot, RuntimeRiskState},
    strategy_execution_contract::{StrategyExecutionConfig, StrategyExecutionIntent},
    trading_semantics::is_contract_inst_type,
};

use super::super::super::{
    numbers::{round6, round8},
    report::build_backtest_report,
    types::BacktestReport,
};
use super::{
    state::{SimOrder, SimPosition},
    values::{aggregate_position_side, order_json},
    HistoricalLiveBacktest, HistoricalMarketData,
};

impl HistoricalLiveBacktest {
    pub fn account_context(&self, timestamp: i64, market: &HistoricalMarketData) -> Value {
        let equity = self.current_equity(timestamp, market);
        let margin_used = self.margin_used(timestamp, market);
        let available_equity = (equity - margin_used).max(0.0);
        json!({
            "source": "historical_live_backtest",
            "mode": "historical_sim",
            "initial_capital": round6(self.settings.initial_capital),
            "cash": round6(self.cash),
            "equity": round6(equity),
            "total_equity": round6(equity),
            "total_eq": round6(equity),
            "usdt_balance": round6(self.cash),
            "usdt_available": round6(available_equity),
            "usdt_equity_usd": round6(equity),
            "margin_used": round6(margin_used),
            "available_equity": round6(available_equity),
            "details": [{
                "ccy": "USDT",
                "avail_bal": round6(available_equity),
                "avail_eq": round6(available_equity),
                "cash_bal": round6(self.cash),
                "eq": round6(equity),
                "eq_usd": round6(equity),
            }],
        })
    }

    pub fn positions_context(&self, timestamp: i64, market: &HistoricalMarketData) -> Value {
        let open = self
            .positions
            .values()
            .filter(|position| position.quantity > 0.0)
            .map(|position| {
                let (mark, mark_missing) = self.mark_price_with_missing(position, timestamp, market);
                let notional = self.position_notional_value(position, mark);
                json!({
                    "mode": "historical_sim",
                    "source": "historical_live_backtest",
                    "instId": position.symbol,
                    "symbol": position.symbol,
                    "inst_id": position.symbol,
                    "instType": position.inst_type,
                    "inst_type": position.inst_type,
                    "side": position.side,
                    "posSide": position.side,
                    "pos_side": position.side,
                    "leverage": round6(position.leverage),
                    "quantity": round8(position.exchange_quantity),
                    "pos": round8(position.exchange_quantity),
                    "availPos": round8(position.exchange_quantity),
                    "avail_pos": round8(position.exchange_quantity),
                    "basePos": round8(position.quantity),
                    "base_position_size": round8(position.quantity),
                    "avgPx": round8(position.entry_price),
                    "avg_px": round8(position.entry_price),
                    "entry_price": round8(position.entry_price),
                    "markPx": round8(mark),
                    "mark_px": round8(mark),
                    "mark_price": round8(mark),
                    "mark_price_source": if mark_missing { "entry_price_fallback" } else { "historical_last_close" },
                    "mark_price_missing": mark_missing,
                    "notionalUsd": round6(notional),
                    "notional_usd": round6(notional),
                    "upl": round6(position.unrealized_pnl(mark)),
                    "unrealized_pnl": round6(position.unrealized_pnl(mark)),
                    "funding_pnl": round6(position.accumulated_funding),
                    "opened_ts": position.opened_ts,
                    "last_funding_ts": position.last_funding_ts,
                    "entry_order_id": position.entry_order_id,
                })
            })
            .collect::<Vec<_>>();
        let mut positions = Map::new();
        positions.insert("open".to_string(), Value::Array(open.clone()));
        for row in open {
            let Some(symbol) = row
                .get("instId")
                .and_then(Value::as_str)
                .or_else(|| row.get("symbol").and_then(Value::as_str))
                .filter(|value| !value.trim().is_empty())
            else {
                continue;
            };
            let symbol = symbol.trim().to_ascii_uppercase();
            if let Some(pos_side) = row
                .get("pos_side")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                positions.insert(
                    format!("{symbol}:{}", pos_side.to_ascii_lowercase()),
                    row.clone(),
                );
            }
            positions.insert(symbol, row);
        }
        Value::Object(positions)
    }

    pub fn orders_context(&self) -> Value {
        let open = self
            .orders
            .iter()
            .filter(|order| order.is_open())
            .map(order_json)
            .collect::<Vec<_>>();
        let mut recent_rejections = self
            .rejected_orders
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>();
        recent_rejections.extend(
            self.orders
                .iter()
                .rev()
                .filter_map(cancelled_order_rejection_context_row),
        );
        recent_rejections.sort_by_key(|row| std::cmp::Reverse(context_row_timestamp(row)));
        recent_rejections.truncate(50);
        json!({
            "source": "historical_live_backtest",
            "mode": "historical_sim",
            "open": open,
            "recent_fills": self.fills.iter().rev().take(50).cloned().collect::<Vec<_>>(),
            "recent_rejections": recent_rejections,
        })
    }

    pub fn record_strategy_step(
        &mut self,
        actions: &[Value],
        skipped_actions: &[Value],
        execution_logs: &[Value],
        diagnostics: Value,
        indicators: Value,
    ) {
        self.strategy_actions.extend(actions.iter().cloned());
        self.skipped_actions.extend(skipped_actions.iter().cloned());
        self.execution_logs.extend(execution_logs.iter().cloned());
        self.strategy_diagnostics = diagnostics;
        self.indicators = indicators;
    }

    pub fn record_equity(&mut self, candle: &OkxCandle, market: &HistoricalMarketData) {
        let timestamp = candle.timestamp;
        self.refresh_trading_day(timestamp, market);
        let equity = self.current_equity(timestamp, market);
        let position_value = self.position_notional(timestamp, market);
        let unrealized_pnl = self.unrealized_pnl(timestamp, market);
        if self.previous_equity > 0.0 && equity.is_finite() {
            self.equity_returns
                .push((equity - self.previous_equity) / self.previous_equity);
        }
        self.previous_equity = equity;
        self.peak_equity = self.peak_equity.max(equity);
        if self.peak_equity > 0.0 {
            self.max_drawdown = self
                .max_drawdown
                .max((self.peak_equity - equity).max(0.0) / self.peak_equity * 100.0);
        }
        self.equity_curve.push(json!({
            "timestamp": timestamp,
            "equity": round6(equity),
            "cash": round6(self.cash),
            "position_value": round6(position_value),
            "position_notional": round6(position_value),
            "unrealized_pnl": round6(unrealized_pnl),
            "position_side": aggregate_position_side(&self.positions),
            "leverage": round6(self.settings.leverage),
            "positions": self.positions_context(timestamp, market).get("open").cloned().unwrap_or_else(|| json!([])),
        }));
    }

    pub(super) fn refresh_trading_day(&mut self, timestamp: i64, market: &HistoricalMarketData) {
        let trading_day = risk_controls::trading_day(timestamp);
        if self.trading_day != trading_day {
            self.trading_day = trading_day;
            self.day_start_equity = self.current_equity(timestamp, market);
        }
    }

    pub fn finish(self, market: &HistoricalMarketData, mut extra_detail: Value) -> BacktestReport {
        let final_ts = self
            .candles
            .last()
            .map(|item| item.timestamp)
            .unwrap_or_default();
        let final_capital = self.current_equity(final_ts, market);
        let runtime_action_summary = self.runtime_action_summary(market);
        let cost_model = self.cost_model_summary();
        if let Some(object) = extra_detail.as_object_mut() {
            object.insert("engine_version".to_string(), json!("historical_live_v1"));
            object.insert("execution_mode".to_string(), json!("historical_sim"));
            object.insert("strategy_protocol".to_string(), json!("evaluate"));
            object
                .entry("runtime_action_summary".to_string())
                .or_insert(runtime_action_summary);
            merge_detail_object_entry(object, "cost_model", cost_model);
            object.insert("orders".to_string(), Value::Array(self.orders_json()));
            object.insert("fills".to_string(), Value::Array(self.fills.clone()));
            object.insert(
                "funding_events".to_string(),
                Value::Array(self.funding_events.clone()),
            );
            object.insert(
                "rejected_orders".to_string(),
                Value::Array(self.rejected_orders.clone()),
            );
            object.insert(
                "open_positions".to_string(),
                self.positions_context(final_ts, market),
            );
            object.insert(
                "strategy_actions".to_string(),
                Value::Array(self.strategy_actions.clone()),
            );
            object.insert(
                "skipped_actions".to_string(),
                Value::Array(self.skipped_actions.clone()),
            );
            object.insert(
                "strategy_execution_logs".to_string(),
                Value::Array(self.execution_logs.clone()),
            );
            object.insert(
                "strategy_diagnostics".to_string(),
                self.strategy_diagnostics.clone(),
            );
            object.insert("indicators".to_string(), self.indicators.clone());
            object.insert(
                "simulation_assumptions".to_string(),
                self.simulation_assumptions(),
            );
        }
        build_backtest_report(
            &self.config,
            &self.candles,
            self.days,
            self.settings.initial_capital,
            final_capital,
            self.max_drawdown,
            self.trade_records
                .iter()
                .map(|trade| trade.commission)
                .sum(),
            self.equity_returns,
            self.trade_records,
            self.equity_curve,
            extra_detail,
        )
    }

    pub(super) fn margin_available(
        &self,
        inst_type: &str,
        new_notional: f64,
        timestamp: i64,
        market: &HistoricalMarketData,
    ) -> crate::error::AppResult<bool> {
        if !is_contract_inst_type(inst_type) {
            return Ok(self.cash + f64::EPSILON >= new_notional * (1.0 + self.settings.fee_rate));
        }
        let equity = self.current_equity(timestamp, market);
        let leverage = self.configured_leverage_for_inst_type(inst_type)?;
        let margin_after = self.margin_used(timestamp, market) + new_notional / leverage;
        Ok(margin_after <= equity + f64::EPSILON)
    }

    pub(super) fn runtime_risk_state(
        &self,
        timestamp: i64,
        market: &HistoricalMarketData,
    ) -> RuntimeRiskState {
        RuntimeRiskState {
            initial_capital: self.settings.initial_capital,
            day_start_equity: self.day_start_equity,
            current_equity: self.current_equity(timestamp, market),
            positions: self
                .positions
                .values()
                .map(|position| {
                    let mark = self.mark_price(position, timestamp, market);
                    let notional = self.position_notional_value(position, mark);
                    PositionRiskSnapshot {
                        symbol: position.symbol.clone(),
                        side_dir: position.side_dir(),
                        notional,
                    }
                })
                .collect(),
        }
    }

    pub(super) fn execution_config_for_intent(
        &self,
        intent: &StrategyExecutionIntent,
    ) -> StrategyExecutionConfig {
        let base = StrategyExecutionConfig {
            symbol: self.config.symbol.clone(),
            inst_type: self.config.inst_type.clone(),
            timeframe: self.config.timeframe.clone(),
            stop_loss: self.config.stop_loss,
            take_profit: self.config.take_profit,
            params: self.config.params.clone(),
        };
        intent.apply_config_overrides(base)
    }

    fn current_equity(&self, timestamp: i64, market: &HistoricalMarketData) -> f64 {
        self.cash
            + self
                .positions
                .values()
                .map(|position| {
                    let mark = self.mark_price(position, timestamp, market);
                    if is_contract_inst_type(&position.inst_type) {
                        position.unrealized_pnl(mark)
                    } else {
                        self.position_notional_value(position, mark)
                    }
                })
                .sum::<f64>()
    }

    fn position_notional(&self, timestamp: i64, market: &HistoricalMarketData) -> f64 {
        self.positions
            .values()
            .map(|position| {
                self.position_notional_value(position, self.mark_price(position, timestamp, market))
            })
            .sum()
    }

    fn unrealized_pnl(&self, timestamp: i64, market: &HistoricalMarketData) -> f64 {
        self.positions
            .values()
            .filter(|position| is_contract_inst_type(&position.inst_type))
            .map(|position| position.unrealized_pnl(self.mark_price(position, timestamp, market)))
            .sum()
    }

    fn margin_used(&self, timestamp: i64, market: &HistoricalMarketData) -> f64 {
        self.positions
            .values()
            .filter(|position| is_contract_inst_type(&position.inst_type))
            .map(|position| {
                let mark = self.mark_price(position, timestamp, market);
                let leverage = position.leverage.max(1.0);
                self.position_notional_value(position, mark) / leverage
            })
            .sum()
    }

    fn mark_price(
        &self,
        position: &SimPosition,
        timestamp: i64,
        market: &HistoricalMarketData,
    ) -> f64 {
        self.mark_price_with_missing(position, timestamp, market).0
    }

    fn mark_price_with_missing(
        &self,
        position: &SimPosition,
        timestamp: i64,
        market: &HistoricalMarketData,
    ) -> (f64, bool) {
        match market.last_close(&position.symbol, &position.timeframe, timestamp) {
            Some(mark) => (mark, false),
            None => (position.entry_price, true),
        }
    }

    fn position_notional_value(&self, position: &SimPosition, mark_price: f64) -> f64 {
        self.exchange_quantity_value(
            &position.inst_type,
            &position.symbol,
            position.exchange_quantity,
            mark_price,
        )
        .expect("open simulated position must have valid instrument rules")
    }

    fn orders_json(&self) -> Vec<Value> {
        self.orders.iter().map(order_json).collect()
    }

    fn runtime_action_summary(&self, market: &HistoricalMarketData) -> Value {
        let mut open_position = 0_u64;
        let mut close_position = 0_u64;
        let mut place_risk_order = 0_u64;
        let mut cancel_order = 0_u64;
        let mut modify_order = 0_u64;
        let mut hold = 0_u64;
        let mut unsupported = 0_u64;
        let mut open_action_count = 0_u64;
        let mut open_actions_with_planned_exit = 0_u64;
        for action in &self.strategy_actions {
            let action_name = action_name(action);
            match action_name.as_str() {
                "open_position" => {
                    open_position += 1;
                    open_action_count += 1;
                    if action_has_planned_exit(action) {
                        open_actions_with_planned_exit += 1;
                    }
                }
                "close_position" => close_position += 1,
                "place_risk_order" => place_risk_order += 1,
                "cancel_order" => cancel_order += 1,
                "modify_order" => modify_order += 1,
                "hold" => hold += 1,
                _ => unsupported += 1,
            }
        }
        let planned_exit_contract =
            planned_exit_contract_status(open_action_count, open_actions_with_planned_exit);
        let mut planned_close_count = 0_u64;
        let mut risk_close_count = 0_u64;
        for trade in &self.trade_records {
            if trade.pnl.is_none() {
                continue;
            }
            let action = trade
                .action
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            let reason = trade.reason.trim();
            if reason.starts_with("planned_exit:") {
                planned_close_count += 1;
            }
            if action == "place_risk_order" || matches!(reason, "stop_loss" | "take_profit") {
                risk_close_count += 1;
            }
        }
        let planned_exit_coverage_pct = if open_action_count > 0 {
            open_actions_with_planned_exit as f64 / open_action_count as f64 * 100.0
        } else {
            0.0
        };
        let final_ts = self
            .candles
            .last()
            .map(|item| item.timestamp)
            .unwrap_or_default();
        let open_positions_missing_mark_count = self
            .positions
            .values()
            .filter(|position| {
                market
                    .last_close(&position.symbol, &position.timeframe, final_ts)
                    .is_none()
            })
            .count() as u64;
        let mut warnings = Vec::new();
        if open_action_count > 0 && open_actions_with_planned_exit == 0 {
            warnings.push("open_actions_missing_planned_exit");
        }
        if !self.positions.is_empty() {
            warnings.push("open_trades_without_close_events");
        }
        if open_positions_missing_mark_count > 0 {
            warnings.push("open_positions_missing_mark_price");
        }
        if self.funding_mark_price_fallbacks > 0 {
            warnings.push("funding_mark_price_fallback");
        }
        if self.planned_exit_reference_price_fallbacks > 0 {
            warnings.push("planned_exit_reference_price_fallback");
        }
        json!({
            "open_position": open_position,
            "close_position": close_position,
            "place_risk_order": place_risk_order,
            "cancel_order": cancel_order,
            "modify_order": modify_order,
            "hold": hold,
            "unsupported": unsupported,
            "total": self.strategy_actions.len(),
            "open_action_count": open_action_count,
            "open_actions_with_planned_exit": open_actions_with_planned_exit,
            "planned_exit_coverage_pct": round6(planned_exit_coverage_pct),
            "planned_exit_contract": planned_exit_contract,
            "planned_close_count": planned_close_count,
            "risk_close_count": risk_close_count,
            "open_positions_missing_mark_count": open_positions_missing_mark_count,
            "funding_mark_price_fallback_count": self.funding_mark_price_fallbacks,
            "planned_exit_reference_price_fallback_count": self.planned_exit_reference_price_fallbacks,
            "warnings": warnings,
        })
    }

    fn simulation_assumptions(&self) -> Value {
        let uses_contract_instruments = self.uses_contract_instruments();
        json!({
            "market_order_fill": "next_executable_candle_open",
            "limit_order_fill": "marketable_limits_fill_at_open_like_price_capped_by_limit_else_resting_touch",
            "ioc_fok_limit_fill": "submission_candle_open_or_cancel",
            "risk_order_fill": "high_low_trigger_with_gap_conservative_price",
            "funding_cost_model": "okx_periodic_swap_funding",
            "funding_source": "okx_funding_rates",
            "funding_sign": "positive_received_negative_paid",
            "total_funding": round6(self.total_funding),
            "funding_events_total": self.funding_events.len(),
            "funding_mark_price_fallback_count": self.funding_mark_price_fallbacks,
            "planned_exit_reference_price_fallback_count": self.planned_exit_reference_price_fallbacks,
            "funding_missing_series": self.funding_missing_series_json(),
            "fee_rate": self.settings.fee_rate,
            "slippage_rate": self.settings.slippage_rate,
            "spread_rate": self.settings.spread_rate,
            "participation_rate": self.settings.participation_rate,
            "contract_mode": uses_contract_instruments,
            "configured_contract_mode": self.settings.contract_mode,
            "leverage": self.settings.leverage,
            "instrument_rules_source": self.settings.instrument_rules_source,
            "order_size_unit": self.order_size_unit_label(uses_contract_instruments),
            "internal_position_unit": "base_currency",
            "position_context_pos_unit": "okx_exchange_size",
            "contract_value": self.settings.contract_value,
            "contract_value_ccy": self.settings.contract_value_ccy,
            "tick_size": self.settings.tick_size,
            "lot_size": self.settings.lot_size,
            "min_size": self.settings.min_size,
            "min_notional": self.settings.min_notional,
        })
    }

    fn uses_contract_instruments(&self) -> bool {
        self.settings.contract_mode
            || is_contract_inst_type(&self.config.inst_type)
            || self
                .orders
                .iter()
                .any(|order| is_contract_inst_type(&order.inst_type))
            || self
                .positions
                .values()
                .any(|position| is_contract_inst_type(&position.inst_type))
            || self
                .planned_exits
                .iter()
                .any(|plan| is_contract_inst_type(&plan.inst_type))
    }

    fn order_size_unit_label(&self, uses_contract_instruments: bool) -> &'static str {
        if uses_contract_instruments {
            "okx_exchange_size_by_inst_type"
        } else {
            "base_currency"
        }
    }

    fn cost_model_summary(&self) -> Value {
        let uses_contract_instruments = self.uses_contract_instruments();
        json!({
            "fee_rate": self.settings.fee_rate,
            "slippage_rate": self.settings.slippage_rate,
            "spread_rate": self.settings.spread_rate,
            "participation_rate": self.settings.participation_rate,
            "contract_mode": uses_contract_instruments,
            "configured_contract_mode": self.settings.contract_mode,
            "order_size_unit": self.order_size_unit_label(uses_contract_instruments),
            "funding_source": "okx_funding_rates",
            "funding_cost_model": "okx_periodic_swap_funding",
            "total_funding": round6(self.total_funding),
            "funding_events_total": self.funding_events.len(),
            "funding_missing_series": self.funding_missing_series_json(),
        })
    }

    fn funding_missing_series_json(&self) -> Value {
        Value::Array(
            self.funding_missing_series
                .iter()
                .map(|(symbol, inst_type)| {
                    json!({
                        "symbol": symbol,
                        "inst_id": symbol,
                        "inst_type": inst_type,
                    })
                })
                .collect(),
        )
    }
}

fn merge_detail_object_entry(target: &mut Map<String, Value>, key: &str, source: Value) {
    let Some(source_object) = source.as_object() else {
        target.entry(key.to_string()).or_insert(source);
        return;
    };
    match target.get_mut(key).and_then(Value::as_object_mut) {
        Some(existing) => {
            for (field, value) in source_object {
                existing.insert(field.clone(), value.clone());
            }
        }
        None => {
            target.insert(key.to_string(), Value::Object(source_object.clone()));
        }
    }
}

fn cancelled_order_rejection_context_row(order: &SimOrder) -> Option<Value> {
    if !matches!(order.status.as_str(), "cancelled" | "canceled") {
        return None;
    }
    if order.exchange_filled > 0.0 || order.fill_summary.count > 0 {
        return None;
    }
    let event_ts = order
        .last_processed_ts
        .max(order.submitted_ts)
        .max(order.action_ts);
    let mut row = order_json(order);
    if let Some(object) = row.as_object_mut() {
        object.insert("status".to_string(), json!("canceled"));
        object.insert("success".to_string(), json!(false));
        object.insert("timestamp".to_string(), json!(event_ts));
        object.insert("updated_ts".to_string(), json!(event_ts));
        object.insert("created_ts".to_string(), json!(order.action_ts));
        if object
            .get("error_message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            object.insert("error_message".to_string(), json!("canceled"));
        }
    }
    Some(row)
}

fn context_row_timestamp(row: &Value) -> i64 {
    row.get("timestamp")
        .and_then(Value::as_i64)
        .or_else(|| row.get("updated_ts").and_then(Value::as_i64))
        .or_else(|| row.get("submitted_ts").and_then(Value::as_i64))
        .or_else(|| row.get("action_timestamp").and_then(Value::as_i64))
        .unwrap_or(0)
}

fn action_name(action: &Value) -> String {
    action
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

fn action_has_planned_exit(action: &Value) -> bool {
    ["planned_exit_time", "exit_time"]
        .iter()
        .any(|key| value_positive_i64(action.get(*key)).is_some())
}

fn value_positive_i64(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|item| i64::try_from(item).ok()))
        .or_else(|| {
            value
                .as_f64()
                .filter(|item| item.is_finite())
                .map(|item| item.round() as i64)
        })
        .or_else(|| value.as_str().and_then(|item| item.trim().parse().ok()))
        .filter(|item| *item > 0)
}

fn planned_exit_contract_status(
    open_action_count: u64,
    open_actions_with_planned_exit: u64,
) -> &'static str {
    if open_action_count == 0 {
        "no_open_actions"
    } else if open_actions_with_planned_exit == open_action_count {
        "planned_exit_complete"
    } else if open_actions_with_planned_exit > 0 {
        "planned_exit_partial"
    } else {
        "planned_exit_missing"
    }
}
