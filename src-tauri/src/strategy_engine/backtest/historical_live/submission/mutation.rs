use serde_json::json;

use crate::{
    strategy_execution_contract::{StrategyExecutionIntent, StrategyOrderTargetKind},
    strategy_execution_semantics::{
        order_management_identity_matches, order_management_scope_matches,
        order_management_target_kind_allows_class, OrderManagementTargetClass,
    },
    trading_semantics::{
        order_management_any_target_kind_collision_reason, validate_modify_order_has_change,
        EXCHANGE_MODIFY_ORDER_FIELDS,
    },
};

use super::super::{
    state::{OrderAction, SimOrder},
    HistoricalLiveBacktest,
};

impl HistoricalLiveBacktest {
    pub(super) fn cancel_order(&mut self, intent: &StrategyExecutionIntent, timestamp: i64) {
        let Some(cancel) = intent.cancel_order.as_ref() else {
            self.reject_management_intent(intent, timestamp, "cancel_order 动作缺少可撤订单身份");
            return;
        };
        if let Some(reason) = ambiguous_management_target_reason(
            &self.orders,
            cancel.target_kind,
            &cancel.order_id,
            &cancel.client_order_id,
            cancel.scope_explicit,
            "cancel_order",
        ) {
            self.reject_management_intent(intent, timestamp, &reason);
            return;
        }
        if any_target_kind_hits_exchange_and_active_algo(
            &self.orders,
            cancel.target_kind,
            &cancel.order_id,
            &cancel.client_order_id,
            cancel.scope_explicit,
            &intent.symbol,
        ) {
            self.reject_management_intent(
                intent,
                timestamp,
                &order_management_any_target_kind_collision_reason("cancel_order"),
            );
            return;
        }
        for order in &mut self.orders {
            if order.is_open()
                && management_target_matches_order(order.action, cancel.target_kind)
                && management_target_scope_matches(order, &intent.symbol, cancel.scope_explicit)
                && order_management_identity_matches(
                    &order.order_id,
                    &order.client_order_id,
                    &cancel.order_id,
                    &cancel.client_order_id,
                )
            {
                order.status = "cancelled".to_string();
                order.error_message = intent.action_record.reason.clone();
                order.last_processed_ts = order.last_processed_ts.max(timestamp);
                return;
            }
        }
        self.reject_management_intent(
            intent,
            timestamp,
            "cancel_order 回测未找到目标订单，已拒绝撤单以避免静默成功",
        );
    }

    pub(super) fn modify_order(&mut self, intent: &StrategyExecutionIntent, timestamp: i64) {
        let Some(modify) = intent.modify_order.as_ref() else {
            self.reject_management_intent(
                intent,
                timestamp,
                "modify_order 动作缺少可改单订单身份或改单参数",
            );
            return;
        };
        if let Err(error) = validate_modify_order_has_change(
            modify.new_size.as_deref(),
            modify.new_price.as_deref(),
            EXCHANGE_MODIFY_ORDER_FIELDS,
        ) {
            self.reject_management_intent(intent, timestamp, &error.to_string());
            return;
        };
        if let Some(reason) = ambiguous_management_target_reason(
            &self.orders,
            modify.target_kind,
            &modify.order_id,
            &modify.client_order_id,
            modify.scope_explicit,
            "modify_order",
        ) {
            self.reject_management_intent(intent, timestamp, &reason);
            return;
        }
        if any_target_kind_hits_exchange_and_active_algo(
            &self.orders,
            modify.target_kind,
            &modify.order_id,
            &modify.client_order_id,
            modify.scope_explicit,
            &intent.symbol,
        ) {
            self.reject_management_intent(
                intent,
                timestamp,
                &order_management_any_target_kind_collision_reason("modify_order"),
            );
            return;
        }
        for index in 0..self.orders.len() {
            let matches = {
                let order = &self.orders[index];
                order.is_open()
                    && management_target_matches_order(order.action, modify.target_kind)
                    && management_target_scope_matches(order, &intent.symbol, modify.scope_explicit)
                    && order_management_identity_matches(
                        &order.order_id,
                        &order.client_order_id,
                        &modify.order_id,
                        &modify.client_order_id,
                    )
            };
            if !matches {
                continue;
            }
            let price_update = if let Some(price) = modify.new_price.as_deref() {
                match self.modify_order_price(&self.orders[index], price) {
                    Ok(value) => Some(value),
                    Err(error) => {
                        let reason = error.to_string();
                        self.reject_management_intent(intent, timestamp, &reason);
                        self.orders[index].error_message = reason;
                        return;
                    }
                }
            } else {
                None
            };
            let size_update = if let Some(size) = modify.new_size.as_deref() {
                match self.modify_order_quantity(&self.orders[index], size, price_update) {
                    Ok(value) => Some(value),
                    Err(error) => {
                        let reason = error.to_string();
                        self.reject_management_intent(intent, timestamp, &reason);
                        self.orders[index].error_message = reason;
                        return;
                    }
                }
            } else {
                None
            };
            if let Some(value) = size_update {
                self.orders[index].exchange_quantity =
                    value.exchange.max(self.orders[index].exchange_filled);
                self.orders[index].quantity = value.base.max(self.orders[index].filled);
            }
            if let Some(value) = price_update {
                if matches!(self.orders[index].action, OrderAction::Risk) {
                    self.orders[index].trigger_price = Some(value);
                } else {
                    self.orders[index].price = Some(value);
                }
            }
            self.orders[index].reason = intent.action_record.reason.clone();
            self.orders[index].last_processed_ts =
                self.orders[index].last_processed_ts.max(timestamp);
            return;
        }
        self.reject_management_intent(
            intent,
            timestamp,
            "modify_order 回测未找到目标订单，已拒绝改单以避免静默成功",
        );
    }

    fn reject_management_intent(
        &mut self,
        intent: &StrategyExecutionIntent,
        timestamp: i64,
        reason: &str,
    ) {
        let (target_order_id, target_client_order_id) = management_target_identity(intent);
        let target_order_kind = management_target_kind(intent);
        let target_order_type = intent
            .modify_order
            .as_ref()
            .and_then(|modify| modify.target_order_type.as_deref());
        self.rejected_orders.push(json!({
            "source": "historical_live_backtest",
            "mode": "historical_sim",
            "symbol": intent.symbol,
            "inst_id": intent.symbol,
            "inst_type": intent.inst_type,
            "side": intent.action_record.side,
            "action": intent.action.as_str(),
            "order_type": intent.order_type,
            "price": intent.action_record.price,
            "status": "rejected",
            "success": false,
            "target_order_id": target_order_id,
            "target_client_order_id": target_client_order_id,
            "target_order_kind": target_order_kind.as_str(),
            "target_order_type": target_order_type,
            "error_message": reason,
            "submitted_ts": timestamp,
            "action_timestamp": intent.action_record.timestamp,
            "timestamp": timestamp,
            "updated_ts": timestamp,
        }));
    }
}

fn ambiguous_management_target_reason(
    orders: &[SimOrder],
    target_kind: StrategyOrderTargetKind,
    target_order_id: &str,
    target_client_order_id: &str,
    scope_explicit: bool,
    action_name: &str,
) -> Option<String> {
    if scope_explicit {
        return None;
    }
    let mut exchange_symbols = Vec::new();
    let mut active_algo_symbols = Vec::new();
    for order in orders {
        if !order_management_identity_matches(
            &order.order_id,
            &order.client_order_id,
            target_order_id,
            target_client_order_id,
        ) {
            continue;
        }
        match order.action {
            OrderAction::Risk if order.is_open() => {
                push_unique_symbol(&mut active_algo_symbols, &order.symbol);
            }
            OrderAction::Open | OrderAction::Close => {
                push_unique_symbol(&mut exchange_symbols, &order.symbol);
            }
            OrderAction::Risk => {}
        }
    }
    if target_kind.allows_algo() && active_algo_symbols.len() > 1 {
        return Some(format!(
            "{action_name} 未显式提供 symbol 且目标保护单身份同时命中多个交易对: {}，已拒绝以避免误撤或误改",
            active_algo_symbols.join(", ")
        ));
    }
    if target_kind.allows_exchange() && exchange_symbols.len() > 1 {
        return Some(format!(
            "{action_name} 未显式提供 symbol 且目标普通订单身份同时命中多个交易对: {}，已拒绝以避免误撤或误改",
            exchange_symbols.join(", ")
        ));
    }
    None
}

fn any_target_kind_hits_exchange_and_active_algo(
    orders: &[SimOrder],
    target_kind: StrategyOrderTargetKind,
    target_order_id: &str,
    target_client_order_id: &str,
    scope_explicit: bool,
    intent_symbol: &str,
) -> bool {
    if target_kind != StrategyOrderTargetKind::Any {
        return false;
    }
    let mut active_algo = false;
    let mut exchange = false;
    for order in orders {
        if !order_management_identity_matches(
            &order.order_id,
            &order.client_order_id,
            target_order_id,
            target_client_order_id,
        ) {
            continue;
        }
        if !management_target_scope_matches(order, intent_symbol, scope_explicit) {
            continue;
        }
        match order.action {
            OrderAction::Risk if order.is_open() => active_algo = true,
            OrderAction::Open | OrderAction::Close => exchange = true,
            OrderAction::Risk => {}
        }
        if active_algo && exchange {
            return true;
        }
    }
    false
}

fn management_target_scope_matches(
    order: &SimOrder,
    intent_symbol: &str,
    scope_explicit: bool,
) -> bool {
    order_management_scope_matches(&order.symbol, intent_symbol, scope_explicit)
}

fn push_unique_symbol(symbols: &mut Vec<String>, symbol: &str) {
    let symbol = symbol.trim().to_ascii_uppercase();
    if symbol.is_empty() || symbols.iter().any(|item| item == &symbol) {
        return;
    }
    symbols.push(symbol);
}

fn management_target_matches_order(
    action: OrderAction,
    target_kind: StrategyOrderTargetKind,
) -> bool {
    order_management_target_kind_allows_class(target_kind.as_str(), order_target_class(action))
}

fn order_target_class(action: OrderAction) -> OrderManagementTargetClass {
    match action {
        OrderAction::Risk => OrderManagementTargetClass::Algo,
        OrderAction::Open | OrderAction::Close => OrderManagementTargetClass::Exchange,
    }
}

fn management_target_identity(intent: &StrategyExecutionIntent) -> (String, String) {
    if let Some(cancel) = intent.cancel_order.as_ref() {
        return (cancel.order_id.clone(), cancel.client_order_id.clone());
    }
    if let Some(modify) = intent.modify_order.as_ref() {
        return (modify.order_id.clone(), modify.client_order_id.clone());
    }
    (String::new(), String::new())
}

fn management_target_kind(intent: &StrategyExecutionIntent) -> StrategyOrderTargetKind {
    if let Some(cancel) = intent.cancel_order.as_ref() {
        return cancel.target_kind;
    }
    if let Some(modify) = intent.modify_order.as_ref() {
        return modify.target_kind;
    }
    StrategyOrderTargetKind::Any
}
