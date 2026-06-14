use serde_json::json;

use crate::{
    strategy_execution_contract::{StrategyExecutionIntent, StrategyIntentAction},
    trading_semantics::exchange_order_type_requires_price_safe,
};

use super::super::{
    state::{OrderAction, SimOrder},
    values::{
        close_target_position_side, entry_order_side, normalized_order_type, order_action_label,
        position_side_from_action_side, NewOrder,
    },
    HistoricalLiveBacktest,
};

impl HistoricalLiveBacktest {
    pub(super) fn create_order(&mut self, input: NewOrder<'_>) {
        let order_id = self.next_order_id();
        let order_type = input
            .order_type
            .as_deref()
            .map(normalized_order_type)
            .unwrap_or_else(|| normalized_order_type(&input.intent.order_type));
        let price = exchange_order_type_requires_price_safe(&order_type)
            .then_some(input.intent.action_record.price);
        let reference_price = input
            .reference_price
            .unwrap_or(input.intent.action_record.price);
        let reference_price_source = input
            .reference_price_source
            .unwrap_or("strategy_action_price");
        self.orders.push(SimOrder {
            client_order_id: format!("bt-cl-{order_id}"),
            order_id,
            symbol: input.intent.symbol.clone(),
            inst_type: input.intent.inst_type.clone(),
            timeframe: input.intent.timeframe.clone(),
            action: input.action,
            order_type,
            side: input.side,
            pos_side: input.pos_side,
            leverage: input.leverage,
            exchange_quantity: input.quantity.exchange,
            exchange_filled: 0.0,
            quantity: input.quantity.base,
            filled: 0.0,
            fill_summary: Default::default(),
            price,
            reference_price,
            reference_price_source: reference_price_source.to_string(),
            reference_price_missing: !is_positive_price(reference_price),
            trigger_price: input.trigger_price,
            risk_kind: input.risk_kind,
            status: "open".to_string(),
            reason: input.reason,
            submitted_ts: input.timestamp,
            last_processed_ts: input.timestamp,
            action_ts: input.intent.action_record.timestamp,
            reduce_only: input.reduce_only,
            entry_order_id: None,
            attached_risk_identity: None,
            attached_risk_orders: input.attached_risk_orders,
            planned_exit: input.planned_exit,
            error_message: String::new(),
        });
    }

    pub(super) fn reject_intent(
        &mut self,
        intent: &StrategyExecutionIntent,
        timestamp: i64,
        reason: &str,
    ) {
        let order_id = self.next_order_id();
        let action = rejected_order_action(intent);
        let side = rejected_order_side(intent);
        let pos_side = rejected_order_pos_side(intent, &side);
        let order_type = normalized_order_type(&intent.order_type);
        let price = exchange_order_type_requires_price_safe(&order_type)
            .then_some(intent.action_record.price);
        self.rejected_orders.push(json!({
            "source": "historical_live_backtest",
            "mode": "historical_sim",
            "order_id": order_id,
            "client_order_id": format!("bt-cl-{order_id}"),
            "symbol": intent.symbol,
            "inst_id": intent.symbol,
            "inst_type": intent.inst_type,
            "side": side,
            "pos_side": pos_side,
            "action": order_action_label(action),
            "order_type": order_type.clone(),
            "price": price,
            "status": "rejected",
            "success": false,
            "error_message": reason,
            "submitted_ts": timestamp,
            "action_timestamp": intent.action_record.timestamp,
            "timestamp": timestamp,
            "updated_ts": timestamp,
        }));
        self.orders.push(SimOrder {
            client_order_id: format!("bt-cl-{order_id}"),
            order_id,
            symbol: intent.symbol.clone(),
            inst_type: intent.inst_type.clone(),
            timeframe: intent.timeframe.clone(),
            action,
            order_type,
            side,
            pos_side,
            leverage: 1.0,
            exchange_quantity: 0.0,
            exchange_filled: 0.0,
            quantity: 0.0,
            filled: 0.0,
            fill_summary: Default::default(),
            price,
            reference_price: intent.action_record.price,
            reference_price_source: "strategy_action_price".to_string(),
            reference_price_missing: !is_positive_price(intent.action_record.price),
            trigger_price: None,
            risk_kind: None,
            status: "rejected".to_string(),
            reason: intent.action_record.reason.clone(),
            submitted_ts: timestamp,
            last_processed_ts: timestamp,
            action_ts: intent.action_record.timestamp,
            reduce_only: false,
            entry_order_id: None,
            attached_risk_identity: None,
            attached_risk_orders: Vec::new(),
            planned_exit: None,
            error_message: reason.to_string(),
        });
    }
}

fn is_positive_price(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn rejected_order_action(intent: &StrategyExecutionIntent) -> OrderAction {
    match intent.action {
        StrategyIntentAction::ClosePosition => OrderAction::Close,
        StrategyIntentAction::PlaceRiskOrder => OrderAction::Risk,
        _ => OrderAction::Open,
    }
}

fn rejected_order_side(intent: &StrategyExecutionIntent) -> String {
    if matches!(intent.action, StrategyIntentAction::OpenPosition) {
        return entry_order_side(&intent.action_record.side);
    }
    intent
        .order_side
        .as_deref()
        .map(str::trim)
        .filter(|side| !side.is_empty())
        .or_else(|| {
            if matches!(intent.action, StrategyIntentAction::PlaceRiskOrder) {
                intent
                    .attached_risk_orders
                    .first()
                    .map(|risk| risk.side.trim())
                    .filter(|side| !side.is_empty())
            } else {
                None
            }
        })
        .unwrap_or("unknown")
        .to_ascii_lowercase()
}

fn rejected_order_pos_side(intent: &StrategyExecutionIntent, side: &str) -> String {
    if matches!(intent.action, StrategyIntentAction::OpenPosition) {
        return position_side_from_action_side(&intent.action_record.side);
    }
    close_target_position_side(side)
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".to_string())
}
