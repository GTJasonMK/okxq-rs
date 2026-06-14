use std::collections::HashMap;

use serde_json::{json, Map, Value};

pub(super) use crate::trading_semantics::round_down_to_step;

use crate::{
    strategy_execution_contract::{
        StrategyExecutionIntent, StrategyPlannedExitIntent, StrategyRiskOrderIntent,
    },
    trading_semantics::{
        action_position_side, close_target_position_side as shared_close_target_position_side,
        entry_order_side_for_action_side, expected_protective_close_side,
        normalize_runtime_order_type_text, normalized_exchange_order_type,
        resolve_protective_order_kind, standalone_risk_order_type as shared_risk_order_type,
    },
};

use super::super::super::numbers::{round6, round8};
use super::state::{OrderAction, RiskKind, SimOrder, SimOrderQuantity, SimPosition};

pub(super) struct NewOrder<'a> {
    pub(super) intent: &'a StrategyExecutionIntent,
    pub(super) timestamp: i64,
    pub(super) action: OrderAction,
    pub(super) order_type: Option<String>,
    pub(super) side: String,
    pub(super) pos_side: String,
    pub(super) leverage: f64,
    pub(super) quantity: SimOrderQuantity,
    pub(super) reference_price: Option<f64>,
    pub(super) reference_price_source: Option<&'static str>,
    pub(super) reduce_only: bool,
    pub(super) trigger_price: Option<f64>,
    pub(super) risk_kind: Option<RiskKind>,
    pub(super) attached_risk_orders: Vec<StrategyRiskOrderIntent>,
    pub(super) planned_exit: Option<StrategyPlannedExitIntent>,
    pub(super) reason: String,
}

pub(super) fn order_json(order: &SimOrder) -> Value {
    let mut row = Map::new();
    row.insert("source".to_string(), json!("historical_live_backtest"));
    row.insert("mode".to_string(), json!("historical_sim"));
    row.insert("order_id".to_string(), json!(order.order_id));
    row.insert("client_order_id".to_string(), json!(order.client_order_id));
    row.insert("symbol".to_string(), json!(order.symbol));
    row.insert("inst_id".to_string(), json!(order.symbol));
    row.insert("inst_type".to_string(), json!(order.inst_type));
    row.insert("timeframe".to_string(), json!(order.timeframe));
    row.insert(
        "action".to_string(),
        json!(order_action_label(order.action)),
    );
    row.insert("side".to_string(), json!(order.side));
    row.insert("pos_side".to_string(), json!(order.pos_side));
    row.insert("leverage".to_string(), json!(round6(order.leverage)));
    row.insert("order_type".to_string(), json!(order.order_type));
    row.insert("price".to_string(), json!(order.price));
    row.insert(
        "reference_price".to_string(),
        json!(positive_price_value(order.reference_price)),
    );
    row.insert(
        "reference_price_source".to_string(),
        json!(order.reference_price_source),
    );
    row.insert(
        "reference_price_missing".to_string(),
        json!(order.reference_price_missing),
    );
    row.insert("trigger_price".to_string(), json!(order.trigger_price));
    row.insert("size".to_string(), json!(round8(order.exchange_quantity)));
    row.insert(
        "quantity".to_string(),
        json!(round8(order.exchange_quantity)),
    );
    row.insert("fill_count".to_string(), json!(order.fill_summary.count));
    row.insert(
        "filled_size".to_string(),
        json!(round8(order.exchange_filled)),
    );
    row.insert(
        "filled_quantity".to_string(),
        json!(round8(order.exchange_filled)),
    );
    row.insert(
        "avg_fill_price".to_string(),
        json!(order.fill_summary.avg_fill_price().map(round8)),
    );
    row.insert(
        "fill_notional".to_string(),
        json!(order.fill_summary.fill_notional().map(round6)),
    );
    row.insert(
        "remaining_size".to_string(),
        json!(round8(order.exchange_remaining())),
    );
    row.insert(
        "total_fee".to_string(),
        json!(order.fill_summary.total_fee().map(round6)),
    );
    row.insert(
        "first_fill_ts".to_string(),
        json!(order.fill_summary.first_ts),
    );
    row.insert(
        "last_fill_ts".to_string(),
        json!(order.fill_summary.last_ts),
    );
    row.insert("base_size".to_string(), json!(round8(order.quantity)));
    row.insert("base_quantity".to_string(), json!(round8(order.quantity)));
    row.insert("base_filled_size".to_string(), json!(round8(order.filled)));
    row.insert(
        "base_remaining_size".to_string(),
        json!(round8(order.remaining())),
    );
    row.insert("status".to_string(), json!(order.status));
    row.insert("success".to_string(), json!(order_success(order)));
    row.insert("reduce_only".to_string(), json!(order.reduce_only));
    row.insert("reason".to_string(), json!(order.reason));
    row.insert("error_message".to_string(), json!(order.error_message));
    row.insert("submitted_ts".to_string(), json!(order.submitted_ts));
    row.insert("action_timestamp".to_string(), json!(order.action_ts));
    row.insert("timestamp".to_string(), json!(order.last_processed_ts));
    row.insert("updated_ts".to_string(), json!(order.last_processed_ts));
    Value::Object(row)
}

fn positive_price_value(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(round8(value))
}

pub(super) fn rejected_order_json(order: &SimOrder, timestamp: i64, reason: &str) -> Value {
    let event_ts = timestamp
        .max(order.last_processed_ts)
        .max(order.submitted_ts)
        .max(order.action_ts);
    let mut row = order_json(order);
    if let Some(object) = row.as_object_mut() {
        object.insert("status".to_string(), json!("rejected"));
        object.insert("success".to_string(), json!(false));
        object.insert("error_message".to_string(), json!(reason));
        object.insert("timestamp".to_string(), json!(event_ts));
        object.insert("updated_ts".to_string(), json!(event_ts));
        object.insert("created_ts".to_string(), json!(order.action_ts));
    }
    row
}

fn order_success(order: &SimOrder) -> bool {
    order.exchange_filled > 0.0
        || matches!(
            order.status.as_str(),
            "filled" | "partially_filled" | "open"
        )
}

pub(super) fn order_action_label(action: OrderAction) -> &'static str {
    match action {
        OrderAction::Open => "open_position",
        OrderAction::Close => "close_position",
        OrderAction::Risk => "place_risk_order",
    }
}

pub(super) fn norm_symbol(symbol: &str) -> String {
    symbol.trim().to_ascii_uppercase()
}

pub(super) fn norm_timeframe(timeframe: &str) -> String {
    timeframe.trim().to_ascii_lowercase()
}

pub(super) fn norm_pos_side(side: &str) -> String {
    let normalized = side.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "long" | "buy" => "long".to_string(),
        "short" | "sell" => "short".to_string(),
        _ => normalized,
    }
}

pub(super) fn normalized_position_side(side: &str) -> Option<&'static str> {
    match side.trim().to_ascii_lowercase().as_str() {
        "long" | "buy" => Some("long"),
        "short" | "sell" => Some("short"),
        _ => None,
    }
}

pub(super) fn position_side_from_action_side(side: &str) -> String {
    action_position_side(side)
        .map(|(pos_side, _)| pos_side.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub(super) fn entry_order_side(side: &str) -> String {
    entry_order_side_for_action_side(side)
        .unwrap_or("unknown")
        .to_string()
}

pub(super) fn expected_close_side(position_side: &str) -> Option<&'static str> {
    expected_protective_close_side(position_side)
}

pub(super) fn close_target_position_side(order_side: &str) -> Option<&'static str> {
    shared_close_target_position_side(order_side).map(|side| side.as_str())
}

pub(super) fn aggregate_position_side(
    positions: &HashMap<(String, String), SimPosition>,
) -> &'static str {
    let has_long = positions
        .values()
        .any(|item| item.quantity > 0.0 && item.side == "long");
    let has_short = positions
        .values()
        .any(|item| item.quantity > 0.0 && item.side == "short");
    match (has_long, has_short) {
        (true, true) => "mixed",
        (true, false) => "long",
        (false, true) => "short",
        (false, false) => "flat",
    }
}

pub(super) fn normalized_order_type(order_type: &str) -> String {
    let value = normalize_runtime_order_type_text(order_type);
    normalized_exchange_order_type(&value)
        .map(ToString::to_string)
        .unwrap_or(value)
}

pub(super) fn risk_order_type(risk: &StrategyRiskOrderIntent) -> String {
    shared_risk_order_type(&risk.order_type)
}

pub(super) fn risk_kind(risk: &StrategyRiskOrderIntent) -> Result<RiskKind, String> {
    resolve_protective_order_kind(&risk.order_type, risk.stop_loss, risk.take_profit)
        .map_err(|error| error.to_string())
}
