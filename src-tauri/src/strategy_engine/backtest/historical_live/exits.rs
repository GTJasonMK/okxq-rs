use serde_json::json;

use crate::{
    strategy_execution_contract::StrategyRiskOrderIntent,
    trading_semantics::{
        planned_exit_entry_remainder_cancel_needed,
        planned_exit_reference_price as shared_planned_exit_reference_price,
        resolve_valid_protective_trigger_price,
    },
};

use super::{
    market_data::HistoricalMarketData,
    state::{OrderAction, PlannedExit, RiskKind, SimOrder, SimOrderQuantity},
    values::{expected_close_side, norm_pos_side, norm_symbol, risk_kind, risk_order_type},
    HistoricalLiveBacktest,
};

impl HistoricalLiveBacktest {
    pub(super) fn sync_attached_exit_orders(
        &mut self,
        entry_order: &SimOrder,
        timestamp: i64,
        quantity: SimOrderQuantity,
    ) {
        self.sync_attached_risk_orders(entry_order, timestamp, quantity);
        self.sync_planned_exit(entry_order, quantity);
    }

    fn sync_planned_exit(&mut self, entry_order: &SimOrder, quantity: SimOrderQuantity) {
        if quantity.base <= 0.0 || quantity.exchange <= 0.0 {
            return;
        }
        if let Some(planned_exit) = entry_order.planned_exit.clone() {
            if let Some(existing) = self
                .planned_exits
                .iter_mut()
                .find(|plan| plan.entry_order_id.as_deref() == Some(entry_order.order_id.as_str()))
            {
                existing.exchange_quantity = quantity.exchange;
                existing.quantity = quantity.base;
                existing.due_ts = planned_exit.timestamp;
                existing.reason = planned_exit.reason;
                existing.contract = planned_exit.contract;
                return;
            }
            self.planned_exits.push(PlannedExit {
                symbol: entry_order.symbol.clone(),
                inst_type: entry_order.inst_type.clone(),
                timeframe: entry_order.timeframe.clone(),
                side: entry_order.pos_side.clone(),
                exchange_quantity: quantity.exchange,
                quantity: quantity.base,
                due_ts: planned_exit.timestamp,
                entry_order_id: Some(entry_order.order_id.clone()),
                reason: planned_exit.reason,
                contract: planned_exit.contract,
                submitted: false,
            });
        }
    }

    fn sync_attached_risk_orders(
        &mut self,
        entry_order: &SimOrder,
        timestamp: i64,
        quantity: SimOrderQuantity,
    ) {
        for (risk_index, risk) in entry_order.attached_risk_orders.iter().enumerate() {
            let risk_identity = attached_risk_identity(entry_order, risk_index);
            let kind = match risk_kind(risk) {
                Ok(value) => value,
                Err(error) => {
                    self.reject_attached_risk_order(entry_order, risk, timestamp, &error);
                    continue;
                }
            };
            let trigger_price = match resolve_valid_protective_trigger_price(
                kind,
                Some(&entry_order.pos_side),
                Some(entry_order.reference_price),
                risk.trigger_price,
                risk.stop_loss,
                risk.take_profit,
                "保护单缺少有效触发价",
            ) {
                Ok(value) => value,
                Err(error) => {
                    self.reject_attached_risk_order(
                        entry_order,
                        risk,
                        timestamp,
                        &error.to_string(),
                    );
                    continue;
                }
            };
            if let Some(existing) = self.orders.iter_mut().find(|order| {
                order.is_open()
                    && matches!(order.action, OrderAction::Risk)
                    && order.entry_order_id.as_deref() == Some(entry_order.order_id.as_str())
                    && order.attached_risk_identity.as_deref() == Some(risk_identity.as_str())
            }) {
                existing.exchange_quantity = quantity.exchange.max(existing.exchange_filled);
                existing.quantity = quantity.base.max(existing.filled);
                // The order was already live before this fill. Resizing must not mark the
                // current candle as processed, otherwise an intrabar stop/take-profit spike is skipped.
                continue;
            }
            let order_id = self.next_order_id();
            // Without intrabar order, scan the entry candle for new stops but not new take profits.
            let last_processed_ts = if matches!(kind, RiskKind::StopLoss) {
                entry_order.last_processed_ts
            } else {
                timestamp
            };
            let close_side = match expected_close_side(&entry_order.pos_side) {
                Some(side) => side,
                None => {
                    let reason = format!(
                        "无法根据入场持仓方向 {} 推导保护单平仓方向",
                        entry_order.pos_side
                    );
                    self.reject_attached_risk_order(entry_order, risk, timestamp, &reason);
                    continue;
                }
            };
            self.orders.push(SimOrder {
                client_order_id: format!("bt-cl-{order_id}"),
                order_id,
                symbol: entry_order.symbol.clone(),
                inst_type: entry_order.inst_type.clone(),
                timeframe: entry_order.timeframe.clone(),
                action: OrderAction::Risk,
                order_type: risk_order_type(risk),
                side: close_side.to_string(),
                pos_side: entry_order.pos_side.clone(),
                leverage: entry_order.leverage,
                exchange_quantity: quantity.exchange,
                exchange_filled: 0.0,
                quantity: quantity.base,
                filled: 0.0,
                fill_summary: Default::default(),
                price: None,
                reference_price: entry_order.reference_price,
                reference_price_source: entry_order.reference_price_source.clone(),
                reference_price_missing: entry_order.reference_price_missing,
                trigger_price: Some(trigger_price),
                risk_kind: Some(kind),
                status: "open".to_string(),
                reason: risk.reason.clone(),
                submitted_ts: timestamp,
                last_processed_ts,
                action_ts: timestamp,
                reduce_only: true,
                entry_order_id: Some(entry_order.order_id.clone()),
                attached_risk_identity: Some(risk_identity),
                attached_risk_orders: Vec::new(),
                planned_exit: None,
                error_message: String::new(),
            });
        }
    }

    fn reject_attached_risk_order(
        &mut self,
        entry_order: &SimOrder,
        risk: &StrategyRiskOrderIntent,
        timestamp: i64,
        reason: &str,
    ) {
        let side = risk.side.trim().to_ascii_lowercase();
        let side = if side.is_empty() {
            expected_close_side(&entry_order.pos_side)
                .unwrap_or("unknown")
                .to_string()
        } else {
            side
        };
        self.rejected_orders.push(json!({
            "source": "historical_live_backtest",
            "mode": "historical_sim",
            "order_id": "",
            "client_order_id": "",
            "symbol": entry_order.symbol,
            "inst_id": entry_order.symbol,
            "inst_type": entry_order.inst_type,
            "side": side,
            "pos_side": entry_order.pos_side,
            "action": "place_risk_order",
            "order_type": risk_order_type(risk),
            "price": Option::<f64>::None,
            "reference_price": entry_order.reference_price,
            "reference_price_source": entry_order.reference_price_source,
            "reference_price_missing": entry_order.reference_price_missing,
            "trigger_price": risk.trigger_price,
            "status": "rejected",
            "success": false,
            "reason": reason,
            "error_message": reason,
            "risk_order": risk.reason,
            "submitted_ts": timestamp,
            "action_timestamp": entry_order.action_ts,
            "timestamp": timestamp,
            "updated_ts": timestamp,
        }));
    }

    pub(super) fn submit_due_planned_exits(
        &mut self,
        timestamp: i64,
        market: &HistoricalMarketData,
    ) -> usize {
        let due = self
            .planned_exits
            .iter_mut()
            .filter(|plan| !plan.submitted && plan.due_ts <= timestamp)
            .map(|plan| {
                plan.submitted = true;
                plan.clone()
            })
            .collect::<Vec<_>>();
        let submitted = due.len();
        for plan in due {
            let close_side = match expected_close_side(&plan.side) {
                Some(side) => side.to_string(),
                None => {
                    let reason = format!("计划退出持仓方向无效: {}", plan.side);
                    self.reject_planned_exit(&plan, timestamp, &reason);
                    continue;
                }
            };
            self.cancel_entry_remainder_for_planned_exit(&plan);
            let key = (norm_symbol(&plan.symbol), norm_pos_side(&plan.side));
            let Some(position) = self.positions.get(&key).cloned() else {
                self.reject_planned_exit(&plan, timestamp, "计划退出没有可平持仓");
                continue;
            };
            let close_exchange = plan.exchange_quantity.min(position.exchange_quantity);
            if close_exchange <= 1e-12 {
                self.reject_planned_exit(&plan, timestamp, "计划退出可平数量为 0");
                continue;
            }
            let close_base =
                match self.close_base_quantity_from_position_exchange(&position, close_exchange) {
                    Ok(value) => value,
                    Err(error) => {
                        self.reject_planned_exit(
                            &plan,
                            timestamp,
                            &format!("计划退出数量换算失败: {error}"),
                        );
                        continue;
                    }
                };
            let (reference_price, reference_price_fallback) = match self
                .planned_exit_reference_price(&plan, market, &close_side, position.entry_price)
            {
                Ok(value) => value,
                Err(reason) => {
                    self.reject_planned_exit(&plan, timestamp, &reason);
                    continue;
                }
            };
            let position_leverage = position.leverage;
            if reference_price_fallback {
                self.planned_exit_reference_price_fallbacks = self
                    .planned_exit_reference_price_fallbacks
                    .saturating_add(1);
            }
            let reference_price_source = if reference_price_fallback {
                "entry_price_fallback"
            } else {
                "historical_last_close"
            };
            let order_id = self.next_order_id();
            self.orders.push(SimOrder {
                client_order_id: format!("bt-cl-{order_id}"),
                order_id,
                symbol: plan.symbol.clone(),
                inst_type: plan.inst_type.clone(),
                timeframe: plan.timeframe.clone(),
                action: OrderAction::Close,
                order_type: "market".to_string(),
                side: close_side,
                pos_side: plan.side.clone(),
                leverage: position_leverage,
                exchange_quantity: close_exchange,
                exchange_filled: 0.0,
                quantity: close_base,
                filled: 0.0,
                fill_summary: Default::default(),
                price: None,
                reference_price,
                reference_price_source: reference_price_source.to_string(),
                reference_price_missing: reference_price_fallback,
                trigger_price: None,
                risk_kind: None,
                status: "open".to_string(),
                reason: format!("planned_exit:{}:{}", plan.contract, plan.reason),
                submitted_ts: plan.due_ts,
                last_processed_ts: plan.due_ts,
                action_ts: plan.due_ts,
                reduce_only: true,
                entry_order_id: plan.entry_order_id,
                attached_risk_identity: None,
                attached_risk_orders: Vec::new(),
                planned_exit: None,
                error_message: String::new(),
            });
        }
        submitted
    }

    fn planned_exit_reference_price(
        &self,
        plan: &PlannedExit,
        market: &HistoricalMarketData,
        close_side: &str,
        entry_price: f64,
    ) -> Result<(f64, bool), String> {
        let historical_close = market.last_close(&plan.symbol, &plan.timeframe, plan.due_ts);
        let reference_price = shared_planned_exit_reference_price(
            historical_close,
            None,
            None,
            close_side,
            entry_price,
        )
        .ok_or_else(|| "计划退出参考价无效，已拒绝平仓以避免 0 价格".to_string())?;
        Ok((reference_price, historical_close.is_none()))
    }

    fn reject_planned_exit(&mut self, plan: &PlannedExit, timestamp: i64, reason: &str) {
        let event_ts = timestamp.max(plan.due_ts);
        let order_id = self.next_order_id();
        self.rejected_orders.push(json!({
            "source": "historical_live_backtest",
            "mode": "historical_sim",
            "order_id": order_id,
            "client_order_id": format!("bt-cl-{order_id}"),
            "symbol": plan.symbol,
            "inst_id": plan.symbol,
            "inst_type": plan.inst_type,
            "timeframe": plan.timeframe,
            "action": "close_position",
            "side": "unknown",
            "pos_side": plan.side,
            "order_type": "market",
            "price": Option::<f64>::None,
            "size": plan.exchange_quantity,
            "quantity": plan.exchange_quantity,
            "base_size": plan.quantity,
            "base_quantity": plan.quantity,
            "status": "rejected",
            "success": false,
            "reduce_only": true,
            "reason": reason,
            "error_message": reason,
            "submitted_ts": plan.due_ts,
            "action_timestamp": plan.due_ts,
            "timestamp": event_ts,
            "updated_ts": event_ts,
        }));
    }

    fn cancel_entry_remainder_for_planned_exit(&mut self, plan: &PlannedExit) {
        let Some(entry_order_id) = plan.entry_order_id.as_deref() else {
            return;
        };
        let Some(order) = self.orders.iter_mut().find(|order| {
            matches!(order.action, OrderAction::Open)
                && order.order_id == entry_order_id
                && planned_exit_entry_remainder_cancel_needed(&order.status, &order.order_type)
                && order.exchange_remaining() > 1e-12
        }) else {
            return;
        };
        order.status = "cancelled".to_string();
        order.error_message = "计划退出到期，入口未成交剩余数量已取消".to_string();
        order.last_processed_ts = order.last_processed_ts.max(plan.due_ts);
    }

    pub(super) fn cancel_exit_orders_for_position(
        &mut self,
        symbol: &str,
        pos_side: &str,
        except_order_id: Option<&str>,
        timestamp: i64,
        reason: &str,
    ) {
        let symbol = norm_symbol(symbol);
        let pos_side = norm_pos_side(pos_side);
        for order in &mut self.orders {
            if !order.is_open()
                || !order.reduce_only
                || norm_symbol(&order.symbol) != symbol
                || norm_pos_side(&order.pos_side) != pos_side
                || except_order_id == Some(order.order_id.as_str())
            {
                continue;
            }
            order.status = "cancelled".to_string();
            order.error_message = reason.to_string();
            order.last_processed_ts = order.last_processed_ts.max(timestamp);
        }
    }

    pub(super) fn cancel_planned_exits_for_position(&mut self, symbol: &str, pos_side: &str) {
        let symbol = norm_symbol(symbol);
        let pos_side = norm_pos_side(pos_side);
        for plan in &mut self.planned_exits {
            if norm_symbol(&plan.symbol) == symbol && norm_pos_side(&plan.side) == pos_side {
                plan.submitted = true;
            }
        }
    }
}

fn attached_risk_identity(entry_order: &SimOrder, risk_index: usize) -> String {
    format!("{}:{risk_index}", entry_order.order_id)
}
