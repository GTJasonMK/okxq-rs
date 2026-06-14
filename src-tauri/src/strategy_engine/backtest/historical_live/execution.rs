use serde_json::json;

use crate::{
    error::AppResult,
    okx::OkxCandle,
    trading_semantics::{
        fill_or_kill_order_type, has_periodic_funding_rate, immediate_remainder_order_type,
        is_contract_inst_type, maker_only_order_type,
    },
};

use super::super::super::{
    numbers::{round6, round8},
    types::TradeRecord,
};
use super::{
    market_data::HistoricalMarketData,
    state::{OrderAction, SimOrder, SimOrderQuantity, SimPosition},
    values::{
        norm_pos_side, norm_symbol, normalized_position_side, order_action_label,
        rejected_order_json, round_down_to_step,
    },
    HistoricalLiveBacktest,
};

impl HistoricalLiveBacktest {
    pub fn process_market_until(&mut self, timestamp: i64, market: &HistoricalMarketData) {
        self.settle_funding_until(timestamp, market);
        self.submit_due_planned_exits(timestamp, market);
        loop {
            let mut advanced = false;
            let mut index = 0usize;
            while index < self.orders.len() {
                let order = self.orders[index].clone();
                if order.is_open() {
                    if order.reduce_only
                        && self.reduce_only_available_exchange_quantity(&order) <= 1e-12
                    {
                        self.cancel_order_index(
                            index,
                            timestamp,
                            "没有可平持仓，reduce-only 订单失效",
                        );
                        advanced = true;
                        index += 1;
                        continue;
                    }
                    if let Some(candle) = market.next_candle_between(
                        &order.symbol,
                        &order.timeframe,
                        order.last_processed_ts,
                        timestamp,
                    ) {
                        self.try_fill_order(index, &order, &candle);
                        if let Some(current) = self.orders.get_mut(index) {
                            current.last_processed_ts =
                                current.last_processed_ts.max(candle.timestamp);
                        }
                        advanced = true;
                    }
                }
                index += 1;
            }
            if self.submit_due_planned_exits(timestamp, market) > 0 {
                advanced = true;
            }
            if !advanced {
                break;
            }
        }
        self.refresh_trading_day(timestamp, market);
    }

    fn settle_funding_until(&mut self, timestamp: i64, market: &HistoricalMarketData) {
        let keys = self.positions.keys().cloned().collect::<Vec<_>>();
        for key in keys {
            let Some(position) = self.positions.get(&key).cloned() else {
                continue;
            };
            if !has_periodic_funding_rate(&position.inst_type) || position.side_dir() == 0 {
                continue;
            }
            if !market.has_funding_series(&position.symbol, &position.inst_type) {
                self.funding_missing_series
                    .insert((norm_symbol(&position.symbol), position.inst_type.clone()));
                continue;
            }
            let points = market.funding_between(
                &position.symbol,
                &position.inst_type,
                position.last_funding_ts,
                timestamp,
            );
            for point in points {
                self.apply_funding_point(&key, &point, market);
            }
        }
    }

    fn apply_funding_point(
        &mut self,
        key: &(String, String),
        point: &super::market_data::HistoricalFundingPoint,
        market: &HistoricalMarketData,
    ) {
        let Some(position) = self.positions.get(key).cloned() else {
            return;
        };
        if point.funding_time <= position.last_funding_ts {
            return;
        }
        let (mark, mark_missing) =
            match market.last_close(&position.symbol, &position.timeframe, point.funding_time) {
                Some(mark) => (mark, false),
                None => {
                    self.funding_mark_price_fallbacks =
                        self.funding_mark_price_fallbacks.saturating_add(1);
                    (position.entry_price, true)
                }
            };
        let mark_price_source = if mark_missing {
            "entry_price_fallback"
        } else {
            "historical_last_close"
        };
        let notional = match self.exchange_quantity_value(
            &position.inst_type,
            &position.symbol,
            position.exchange_quantity,
            mark,
        ) {
            Ok(value) if value.is_finite() && value > 0.0 => value,
            _ => return,
        };
        let funding = -notional * point.funding_rate * position.side_dir() as f64;
        self.cash += funding;
        self.total_funding += funding;
        if let Some(current) = self.positions.get_mut(key) {
            current.last_funding_ts = point.funding_time;
            current.accumulated_funding += funding;
        }
        let symbol = position.symbol.clone();
        let inst_type = position.inst_type.clone();
        let side = position.side.clone();
        let event = json!({
            "source": "historical_live_backtest",
            "mode": "historical_sim",
            "event": "funding",
            "action": "funding",
            "symbol": symbol.clone(),
            "inst_id": symbol.clone(),
            "inst_type": inst_type.clone(),
            "side": side.clone(),
            "pos_side": side.clone(),
            "funding_time": point.funding_time,
            "timestamp": point.funding_time,
            "funding_rate": point.funding_rate,
            "mark_price": round8(mark),
            "mark_price_source": mark_price_source,
            "mark_price_missing": mark_missing,
            "size": round8(position.exchange_quantity),
            "quantity": round8(position.exchange_quantity),
            "base_size": round8(position.quantity),
            "base_quantity": round8(position.quantity),
            "notional": round6(notional),
            "value": round6(notional),
            "funding": round6(funding),
            "cash_after": round6(self.cash),
        });
        self.funding_events.push(event);
        self.trade_records.push(TradeRecord {
            symbol: Some(position.symbol.clone()),
            timestamp: point.funding_time,
            side: "funding".to_string(),
            pos_side: Some(position.side.clone()),
            action: Some("funding".to_string()),
            price: mark,
            quantity: position.quantity,
            exchange_quantity: position.exchange_quantity,
            value: notional,
            commission: 0.0,
            pnl: None,
            funding,
            equity: None,
            reason: format!("funding_rate:{:.8}", point.funding_rate),
        });
    }

    fn try_fill_order(&mut self, order_index: usize, order: &SimOrder, candle: &OkxCandle) {
        if !matches!(order.side.as_str(), "buy" | "sell") {
            self.reject_order_index(
                order_index,
                candle.timestamp,
                format!("订单方向无效: {}，已拒绝以避免默认撮合", order.side),
            );
            return;
        }
        if normalized_position_side(&order.pos_side).is_none() {
            self.reject_order_index(
                order_index,
                candle.timestamp,
                format!(
                    "持仓方向无效: {}，已拒绝以避免默认按 long/short 撮合",
                    order.pos_side
                ),
            );
            return;
        }
        if matches!(order.action, OrderAction::Risk) && order.risk_kind.is_none() {
            self.reject_order_index(
                order_index,
                candle.timestamp,
                "保护单缺少风险类型，已拒绝以避免默认按止损撮合".to_string(),
            );
            return;
        }
        if maker_only_would_cross_on_first_candle(order, candle) {
            self.cancel_maker_only_order(order_index, candle.timestamp);
            return;
        }
        let Some(price) = self.execution_price(order, candle) else {
            if immediate_order_remainder_should_cancel(order) {
                self.cancel_immediate_order_remainder(order_index, candle.timestamp);
            }
            return;
        };
        let rules = match self.settings.instrument_rules_for(&order.symbol) {
            Ok(value) => value,
            Err(error) => {
                self.reject_order_index(order_index, candle.timestamp, error.to_string());
                return;
            }
        };
        let base_capacity = (self.candle_base_capacity(&order.inst_type, candle)
            * self.settings.participation_rate)
            .max(0.0);
        if !base_capacity.is_finite() || base_capacity <= 0.0 {
            self.reject_order_index(
                order_index,
                candle.timestamp,
                "模拟成交失败：K线成交量为 0 或无效".to_string(),
            );
            return;
        }
        let exchange_capacity = match self.exchange_quantity_from_base_quantity(
            &order.inst_type,
            &order.symbol,
            base_capacity,
            price,
        ) {
            Ok(value) => value,
            Err(error) => {
                self.reject_order_index(order_index, candle.timestamp, error.to_string());
                return;
            }
        };
        let exchange_capacity =
            round_down_to_step(exchange_capacity, rules.lot_size.unwrap_or(0.0));
        let exchange_quantity = self.executable_fill_exchange_quantity(
            order,
            order.exchange_remaining().min(exchange_capacity),
        );
        if fill_or_kill_order_type(&order.order_type)
            && exchange_quantity + 1e-12 < order.exchange_remaining()
        {
            self.cancel_fill_or_kill_order(order_index, candle.timestamp);
            return;
        }
        if order.reduce_only && exchange_quantity <= 1e-12 {
            self.cancel_order_index(
                order_index,
                candle.timestamp,
                "没有可平持仓，reduce-only 订单失效",
            );
            return;
        }
        if exchange_quantity > 0.0 {
            let base_quantity = match self.fill_base_quantity(order, exchange_quantity, price) {
                Ok(value) => value,
                Err(error) => {
                    self.reject_order_index(order_index, candle.timestamp, error.to_string());
                    return;
                }
            };
            let value = match self.exchange_quantity_value(
                &order.inst_type,
                &order.symbol,
                exchange_quantity,
                price,
            ) {
                Ok(value) => value,
                Err(error) => {
                    self.reject_order_index(order_index, candle.timestamp, error.to_string());
                    return;
                }
            };
            self.apply_fill(
                order_index,
                order,
                candle.timestamp,
                price,
                SimOrderQuantity {
                    exchange: exchange_quantity,
                    base: base_quantity,
                },
                value,
            );
        }
        if immediate_order_remainder_should_cancel(order) {
            self.cancel_immediate_order_remainder(order_index, candle.timestamp);
        }
    }

    fn fill_base_quantity(
        &self,
        order: &SimOrder,
        exchange_quantity: f64,
        price: f64,
    ) -> AppResult<f64> {
        if matches!(order.action, OrderAction::Close | OrderAction::Risk) || order.reduce_only {
            return self.close_base_quantity_from_position(order, exchange_quantity);
        }
        self.base_quantity_from_exchange_quantity(
            &order.inst_type,
            &order.symbol,
            exchange_quantity,
            price,
        )
    }

    fn close_base_quantity_from_position(
        &self,
        order: &SimOrder,
        exchange_quantity: f64,
    ) -> AppResult<f64> {
        let key = (norm_symbol(&order.symbol), norm_pos_side(&order.pos_side));
        let Some(position) = self.positions.get(&key) else {
            return Err(crate::error::AppError::Validation(
                "没有可平持仓，reduce-only 订单失效".to_string(),
            ));
        };
        self.close_base_quantity_from_position_exchange(position, exchange_quantity)
    }

    fn apply_fill(
        &mut self,
        order_index: usize,
        order: &SimOrder,
        timestamp: i64,
        price: f64,
        quantity: SimOrderQuantity,
        value: f64,
    ) {
        let commission = value * self.settings.fee_rate;
        let contract_order = is_contract_inst_type(&order.inst_type);
        if contract_order && matches!(order.action, OrderAction::Open) {
            self.cash -= commission;
        }
        let mut reduce_only_position_closed = false;
        let pnl = match order.action {
            OrderAction::Open => {
                self.apply_open_fill(order, price, quantity, timestamp);
                None
            }
            OrderAction::Close | OrderAction::Risk => {
                let (pnl, position_closed) =
                    self.apply_close_fill(order, price, quantity, commission);
                reduce_only_position_closed = position_closed;
                pnl
            }
        };
        if !contract_order {
            self.apply_spot_cash(order, value, commission);
        }
        let mut order_filled_after = order.filled + quantity.base;
        let mut order_exchange_filled_after = order.exchange_filled + quantity.exchange;
        if let Some(item) = self.orders.get_mut(order_index) {
            item.filled += quantity.base;
            item.exchange_filled += quantity.exchange;
            item.fill_summary
                .record(timestamp, price, quantity.exchange, value, commission);
            order_filled_after = item.filled;
            order_exchange_filled_after = item.exchange_filled;
            if order.reduce_only && reduce_only_position_closed {
                item.quantity = item.filled;
                item.exchange_quantity = item.exchange_filled;
            }
            item.status = if item.exchange_remaining() <= 1e-12 {
                "filled".to_string()
            } else {
                "partially_filled".to_string()
            };
        }
        self.fills.push(json!({
            "source": "historical_live_backtest",
            "mode": "historical_sim",
            "order_id": order.order_id,
            "client_order_id": order.client_order_id,
            "symbol": order.symbol,
            "inst_id": order.symbol,
            "inst_type": order.inst_type,
            "side": order.side,
            "pos_side": order.pos_side,
            "action": order_action_label(order.action),
            "order_type": order.order_type,
            "price": round8(price),
            "size": round8(quantity.exchange),
            "quantity": round8(quantity.exchange),
            "filled_size": round8(quantity.exchange),
            "filled_quantity": round8(quantity.exchange),
            "base_size": round8(quantity.base),
            "base_quantity": round8(quantity.base),
            "value": round6(value),
            "commission": round6(commission),
            "fee": round6(commission),
            "pnl": pnl.map(round6),
            "success": true,
            "status": "filled",
            "timestamp": timestamp,
        }));
        self.trade_records.push(TradeRecord {
            symbol: Some(order.symbol.clone()),
            timestamp,
            side: order.side.clone(),
            pos_side: Some(order.pos_side.clone()),
            action: Some(order_action_label(order.action).to_string()),
            price,
            quantity: quantity.base,
            exchange_quantity: quantity.exchange,
            value,
            commission,
            pnl,
            funding: 0.0,
            equity: None,
            reason: order.reason.clone(),
        });
        if matches!(order.action, OrderAction::Open) && order_filled_after > 0.0 {
            self.sync_attached_exit_orders(
                order,
                timestamp,
                SimOrderQuantity {
                    exchange: order_exchange_filled_after,
                    base: order_filled_after,
                },
            );
        }
        if reduce_only_position_closed {
            self.cancel_exit_orders_for_position(
                &order.symbol,
                &order.pos_side,
                Some(&order.order_id),
                timestamp,
                "持仓已归零，退出单失效",
            );
            self.cancel_planned_exits_for_position(&order.symbol, &order.pos_side);
        }
    }

    fn executable_fill_exchange_quantity(&self, order: &SimOrder, requested: f64) -> f64 {
        if requested <= 0.0 || !requested.is_finite() {
            return 0.0;
        }
        if !order.reduce_only && !matches!(order.action, OrderAction::Close | OrderAction::Risk) {
            return requested;
        }
        requested.min(self.reduce_only_available_exchange_quantity(order))
    }

    fn reduce_only_available_exchange_quantity(&self, order: &SimOrder) -> f64 {
        self.positions
            .get(&(norm_symbol(&order.symbol), norm_pos_side(&order.pos_side)))
            .map(|position| position.exchange_quantity)
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(0.0)
    }

    fn apply_open_fill(
        &mut self,
        order: &SimOrder,
        price: f64,
        quantity: SimOrderQuantity,
        timestamp: i64,
    ) {
        let key = (norm_symbol(&order.symbol), norm_pos_side(&order.pos_side));
        let entry = self.positions.entry(key).or_insert_with(|| SimPosition {
            symbol: order.symbol.clone(),
            inst_type: order.inst_type.clone(),
            timeframe: order.timeframe.clone(),
            side: norm_pos_side(&order.pos_side),
            exchange_quantity: 0.0,
            quantity: 0.0,
            entry_price: price,
            leverage: order.leverage.max(1.0),
            realized_pnl: 0.0,
            opened_ts: timestamp,
            last_funding_ts: timestamp,
            accumulated_funding: 0.0,
            entry_order_id: order.order_id.clone(),
        });
        let next_quantity = entry.quantity + quantity.base;
        if next_quantity > 0.0 {
            entry.entry_price =
                ((entry.entry_price * entry.quantity) + (price * quantity.base)) / next_quantity;
        }
        entry.quantity = next_quantity;
        entry.exchange_quantity += quantity.exchange;
    }

    fn apply_close_fill(
        &mut self,
        order: &SimOrder,
        price: f64,
        quantity: SimOrderQuantity,
        commission: f64,
    ) -> (Option<f64>, bool) {
        let key = (norm_symbol(&order.symbol), norm_pos_side(&order.pos_side));
        let Some(position) = self.positions.get_mut(&key) else {
            return (None, false);
        };
        let close_exchange = quantity.exchange.min(position.exchange_quantity);
        let close_quantity = quantity.base.min(position.quantity);
        if close_quantity <= 1e-12 {
            return (None, false);
        }
        let pnl = (price - position.entry_price) * close_quantity * position.side_dir() as f64;
        position.exchange_quantity = (position.exchange_quantity - close_exchange).max(0.0);
        position.quantity = (position.quantity - close_quantity).max(0.0);
        position.realized_pnl += pnl - commission;
        if is_contract_inst_type(&order.inst_type) {
            self.cash += pnl - commission;
        }
        let position_closed = position.quantity <= 1e-12 || position.exchange_quantity <= 1e-12;
        if position_closed {
            self.positions.remove(&key);
        }
        (Some(pnl - commission), position_closed)
    }

    fn apply_spot_cash(&mut self, order: &SimOrder, value: f64, commission: f64) {
        match order.side.as_str() {
            "buy" => self.cash -= value + commission,
            "sell" => self.cash += value - commission,
            _ => {}
        }
    }

    fn reject_order_index(&mut self, order_index: usize, timestamp: i64, reason: String) {
        if let Some(order) = self.orders.get_mut(order_index) {
            order.status = "rejected".to_string();
            order.error_message = reason.clone();
            order.last_processed_ts = order.last_processed_ts.max(timestamp);
            self.rejected_orders
                .push(rejected_order_json(order, timestamp, &reason));
        }
    }

    pub(super) fn cancel_order_index(&mut self, order_index: usize, timestamp: i64, reason: &str) {
        self.requeue_planned_exit_remainder(order_index, timestamp);
        if let Some(order) = self.orders.get_mut(order_index) {
            if order.is_open() {
                order.status = "cancelled".to_string();
                order.error_message = reason.to_string();
                order.last_processed_ts = order.last_processed_ts.max(timestamp);
            }
        }
    }

    fn requeue_planned_exit_remainder(&mut self, order_index: usize, timestamp: i64) {
        let Some(order) = self.orders.get(order_index) else {
            return;
        };
        if !matches!(order.action, OrderAction::Close)
            || !order.reduce_only
            || !order.reason.starts_with("planned_exit:")
        {
            return;
        }
        let Some(entry_order_id) = order.entry_order_id.clone() else {
            return;
        };
        let symbol = order.symbol.clone();
        let pos_side = order.pos_side.clone();
        let remaining_exchange = order.exchange_remaining();
        if remaining_exchange <= 1e-12 {
            return;
        }
        let key = (norm_symbol(&symbol), norm_pos_side(&pos_side));
        let Some(position) = self.positions.get(&key) else {
            return;
        };
        let retry_exchange = remaining_exchange.min(position.exchange_quantity);
        if retry_exchange <= 1e-12 {
            return;
        }
        let retry_base =
            match self.close_base_quantity_from_position_exchange(position, retry_exchange) {
                Ok(value) => value,
                Err(error) => {
                    self.rejected_orders.push(rejected_order_json(
                        order,
                        timestamp,
                        &format!("计划退出残量重排失败: {error}"),
                    ));
                    return;
                }
            };
        if retry_base <= 1e-12 {
            return;
        }
        if let Some(plan) = self.planned_exits.iter_mut().find(|plan| {
            plan.entry_order_id.as_deref() == Some(entry_order_id.as_str())
                && norm_symbol(&plan.symbol) == norm_symbol(&symbol)
                && norm_pos_side(&plan.side) == norm_pos_side(&pos_side)
        }) {
            plan.exchange_quantity = retry_exchange;
            plan.quantity = retry_base;
            plan.due_ts = timestamp;
            plan.submitted = false;
        }
    }

    fn cancel_immediate_order_remainder(&mut self, order_index: usize, timestamp: i64) {
        let order_type = self
            .orders
            .get(order_index)
            .map(|order| order.order_type.as_str())
            .unwrap_or("market");
        let reason = format!("{order_type} 未成交剩余数量已按立即成交订单语义取消");
        self.cancel_order_index(order_index, timestamp, &reason);
    }

    fn cancel_fill_or_kill_order(&mut self, order_index: usize, timestamp: i64) {
        self.cancel_order_index(
            order_index,
            timestamp,
            "fok 未能全量成交，已按 FOK 语义取消",
        );
    }

    fn cancel_maker_only_order(&mut self, order_index: usize, timestamp: i64) {
        self.cancel_order_index(
            order_index,
            timestamp,
            "maker-only 订单提交时会立即吃单，已按 post-only 语义取消",
        );
    }
}

fn immediate_order_remainder_should_cancel(order: &SimOrder) -> bool {
    !matches!(order.action, OrderAction::Risk) && immediate_remainder_order_type(&order.order_type)
}

fn maker_only_would_cross_on_first_candle(order: &SimOrder, candle: &OkxCandle) -> bool {
    if !maker_only_order_type(&order.order_type) {
        return false;
    }
    if order.last_processed_ts != order.submitted_ts {
        return false;
    }
    let Some(price) = order.price else {
        return false;
    };
    match order.side.as_str() {
        "buy" => price >= candle.open,
        "sell" => price <= candle.open,
        _ => false,
    }
}
