use serde_json::{json, Map, Value};

use crate::{
    live_strategy::{
        decision::{
            StrategyCancelOrderIntent, StrategyIntentAction, StrategyModifyOrderIntent,
            StrategyRiskOrderIntent,
        },
        types::LiveStrategyConfig,
    },
    strategy_engine::{StrategyActionRecord, StrategyConfig},
    strategy_execution_semantics::{
        action_dedupe_identity as shared_action_dedupe_identity,
        action_submission_key as shared_action_submission_key, ActionDedupeIdentityInput,
        ActionSubmissionKeyInput, OrderManagementCancelIdentity, OrderManagementModifyIdentity,
        RiskOrderIdentity,
    },
    timeframes::{normalize_okx_timeframe, normalized_okx_timeframe_to_ms},
    trading_semantics::{
        action_position_side as shared_action_position_side, close_order_side_from_position_text,
        entry_order_side_for_action_side,
    },
};

const ENGINE_ONLY_STRATEGY_PARAM_KEYS: &[&str] = &[
    "contract_mode",
    "fee_rate",
    "commission_rate",
    "slippage_rate",
    "funding_rate_8h",
    "leverage",
    "max_allowed_leverage",
    "td_mode",
    "mgn_mode",
    "margin_mode",
    "position_size",
    "position_size_mode",
    "position_sizing",
    "position_size_is_notional",
    "position_size_is_effective_notional",
    "allow_short",
    "max_hold_bars",
    "maintenance_margin_rate",
    "execution_timing",
    "execution_model",
    "execution_delay_bars",
    "execution_price",
    "stop_loss",
    "take_profit",
    "max_slippage",
    "max_slippage_pct",
    "max_slippage_bps",
    "_runtime_max_slippage",
    "max_single_loss_ratio",
    "max_symbol_exposure_pct",
    "max_order_value",
    "require_stop_loss",
    "require_protective_stop",
    "require_protective_risk_order",
    "stop_loss_required",
    "risk_control_enabled",
    "max_leverage",
    "max_daily_loss_ratio",
    "daily_loss_limit_ratio",
    "daily_loss_limit",
    "max_daily_loss_bps",
    "max_same_direction_exposure_pct",
    "max_correlated_exposure_pct",
    "max_same_side_exposure_pct",
    "backtest_instrument_rules_source",
    "instrument_rules_source",
    "historical_instrument_rules_source",
    "ctVal",
    "ctValCcy",
    "ct_val",
    "ct_val_ccy",
    "contract_value",
    "contract_value_ccy",
    "lotSz",
    "minSz",
    "tickSz",
    "lot_size",
    "min_size",
    "tick_size",
    "live_fill_sync_symbol_limit",
    "fill_sync_symbol_limit",
];

pub(crate) fn strategy_visible_params(params: &Value) -> Value {
    let Some(object) = params.as_object() else {
        return params.clone();
    };
    let visible = object
        .iter()
        .filter(|(key, _)| !is_engine_only_strategy_param_key(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Map<String, Value>>();
    Value::Object(visible)
}

pub(crate) fn strategy_config_json_for_evaluate(config: &StrategyConfig) -> Value {
    json!({
        "strategy_id": config.strategy_id,
        "strategy_name": config.strategy_name,
        "symbol": config.symbol,
        "inst_type": config.inst_type,
        "timeframe": config.timeframe,
        "initial_capital": config.initial_capital,
        "position_size": config.position_size,
        "stop_loss": config.stop_loss,
        "take_profit": config.take_profit,
        "params": strategy_visible_params(&config.params)
    })
}

fn is_engine_only_strategy_param_key(key: &str) -> bool {
    ENGINE_ONLY_STRATEGY_PARAM_KEYS.contains(&key)
}

pub fn required_action_candle_count_for_timeframe(params: &Value, timeframe: &str) -> usize {
    let suffix = timeframe_suffix(timeframe);
    let fast = scoped_json_param_usize(params, "fast_window", suffix, 12).max(1);
    let slow = scoped_json_param_usize(params, "slow_window", suffix, 48).max(fast + 1);
    let momentum = scoped_json_param_usize(params, "momentum_window", suffix, 24);
    let vol = scoped_json_param_usize(params, "vol_window", suffix, 72);
    let breakout = scoped_json_param_usize(params, "breakout_window", suffix, 36);
    let aux_breakout = scoped_json_param_usize(params, "aux_breakout_window", suffix, 48);
    let aux_momentum = scoped_json_param_usize(params, "aux_momentum_window", suffix, 48);
    let threshold_lookback = scoped_json_param_usize(params, "threshold_lookback", suffix, 0);
    let min_threshold_samples = scoped_json_param_usize(params, "min_threshold_samples", suffix, 0);
    let warmup = slow
        .max(momentum)
        .max(vol)
        .max(breakout)
        .max(aux_breakout)
        .max(aux_momentum)
        .max(min_threshold_samples);
    let quantile_history = threshold_lookback.max(min_threshold_samples);
    warmup
        .saturating_add(quantile_history)
        .saturating_add(1)
        .max(160)
}

pub(in crate::live_strategy) fn finite_or(value: f64, default_value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        default_value
    }
}

pub(in crate::live_strategy) fn canonical_timeframe(value: &str) -> Option<&'static str> {
    normalize_okx_timeframe(value)
}

pub(in crate::live_strategy) fn normalize_timeframe(value: &str) -> String {
    canonical_timeframe(value).unwrap_or("1m").to_string()
}

pub(in crate::live_strategy) fn timeframe_millis(value: &str) -> i64 {
    let trimmed = canonical_timeframe(value).unwrap_or_else(|| value.trim());
    if let Some(milliseconds) = normalized_okx_timeframe_to_ms(trimmed) {
        return milliseconds;
    }
    if let Some(minutes) = trimmed.strip_suffix('m') {
        return minutes.parse::<i64>().unwrap_or(0).saturating_mul(60_000);
    }
    if let Some(hours) = trimmed.strip_suffix('H') {
        return hours.parse::<i64>().unwrap_or(0).saturating_mul(3_600_000);
    }
    if let Some(days) = trimmed.strip_suffix('D') {
        return days.parse::<i64>().unwrap_or(0).saturating_mul(86_400_000);
    }
    if let Some(weeks) = trimmed.strip_suffix('W') {
        return weeks
            .parse::<i64>()
            .unwrap_or(0)
            .saturating_mul(604_800_000);
    }
    0
}

pub(in crate::live_strategy) fn timestamp_to_text(timestamp: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(timestamp)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339())
}

pub(crate) struct LiveActionSubmissionKeyInput<'a> {
    pub(crate) symbol: &'a str,
    pub(crate) action: StrategyIntentAction,
    pub(crate) order_type: &'a str,
    pub(crate) order_side: Option<&'a str>,
    pub(crate) exchange_size: Option<&'a str>,
    pub(crate) planned_exit_timestamp: Option<i64>,
    pub(crate) action_identity: Option<&'a str>,
    pub(crate) action_index: usize,
    pub(crate) action_record: &'a StrategyActionRecord,
}

pub(crate) fn action_submission_key(input: LiveActionSubmissionKeyInput<'_>) -> String {
    let action_record = input.action_record;
    shared_action_submission_key(ActionSubmissionKeyInput {
        symbol: input.symbol,
        action: input.action.as_str(),
        order_type: input.order_type,
        action_side: &action_record.side,
        action_price: action_record.price,
        action_position_size: action_record.position_size,
        action_timestamp: action_record.timestamp,
        order_side: input.order_side,
        exchange_size: input.exchange_size,
        planned_exit_timestamp: input.planned_exit_timestamp,
        action_identity: input.action_identity,
        action_index: input.action_index,
    })
}

pub(crate) fn action_dedupe_identity(
    action: StrategyIntentAction,
    cancel_order: Option<&StrategyCancelOrderIntent>,
    modify_order: Option<&StrategyModifyOrderIntent>,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    max_slippage: Option<f64>,
    attached_risk_orders: &[StrategyRiskOrderIntent],
) -> Option<String> {
    let cancel_order = cancel_order.map(cancel_order_identity);
    let modify_order = modify_order.map(modify_order_identity);
    let attached_risk_order_identities = risk_order_identities(attached_risk_orders);
    shared_action_dedupe_identity(ActionDedupeIdentityInput {
        action: action.as_str(),
        cancel_order,
        modify_order,
        stop_loss,
        take_profit,
        max_slippage,
        attached_risk_orders: &attached_risk_order_identities,
    })
}

#[cfg(test)]
pub(crate) fn order_management_action_identity(
    cancel_order: Option<&StrategyCancelOrderIntent>,
    modify_order: Option<&StrategyModifyOrderIntent>,
) -> Option<String> {
    crate::strategy_execution_semantics::order_management_action_identity(
        cancel_order.map(cancel_order_identity),
        modify_order.map(modify_order_identity),
    )
}

pub(in crate::live_strategy) fn order_quantity(
    config: &LiveStrategyConfig,
    action_record: &StrategyActionRecord,
) -> f64 {
    crate::risk_controls::runtime_order_base_quantity(
        &config.params,
        &config.inst_type,
        config.initial_capital,
        config.position_size,
        action_record.position_size,
        action_record.price,
    )
    .unwrap_or(0.0)
}

pub(crate) fn action_position_side(side: &str) -> Option<(&'static str, i32)> {
    shared_action_position_side(side)
}

pub(crate) fn close_order_side_from_text(side: &str) -> Option<&'static str> {
    close_order_side_from_position_text(side)
}

pub(in crate::live_strategy) fn entry_order_side(side: &str) -> &'static str {
    entry_order_side_for_action_side(side).unwrap_or("unknown")
}

fn cancel_order_identity(cancel: &StrategyCancelOrderIntent) -> OrderManagementCancelIdentity<'_> {
    OrderManagementCancelIdentity {
        target_kind: cancel.target_kind.as_str(),
        order_id: &cancel.order_id,
        client_order_id: &cancel.client_order_id,
    }
}

fn modify_order_identity(modify: &StrategyModifyOrderIntent) -> OrderManagementModifyIdentity<'_> {
    OrderManagementModifyIdentity {
        target_kind: modify.target_kind.as_str(),
        target_order_type: modify.target_order_type.as_deref(),
        order_id: &modify.order_id,
        client_order_id: &modify.client_order_id,
        new_size: modify.new_size.as_deref(),
        new_price: modify.new_price.as_deref(),
        cancel_on_fail: modify.cancel_on_fail,
    }
}

fn risk_order_identities(
    attached_risk_orders: &[StrategyRiskOrderIntent],
) -> Vec<RiskOrderIdentity<'_>> {
    attached_risk_orders
        .iter()
        .map(|risk| RiskOrderIdentity {
            symbol: &risk.symbol,
            side: &risk.side,
            order_type: &risk.order_type,
            trigger_price: risk.trigger_price,
            stop_loss: risk.stop_loss,
            take_profit: risk.take_profit,
        })
        .collect()
}

fn scoped_json_param_usize(
    params: &Value,
    key: &str,
    suffix: Option<&'static str>,
    default: usize,
) -> usize {
    if let Some(suffix) = suffix {
        let scoped_key = format!("{key}_{suffix}");
        if let Some(value) = json_param_usize_optional(params, &scoped_key) {
            return value;
        }
    }
    json_param_usize_optional(params, key).unwrap_or(default)
}

fn timeframe_suffix(timeframe: &str) -> Option<&'static str> {
    match timeframe.trim().to_ascii_lowercase().as_str() {
        "15m" => Some("15m"),
        "5m" => Some("5m"),
        "3m" => Some("3m"),
        _ => None,
    }
}

fn json_param_usize_optional(params: &Value, key: &str) -> Option<usize> {
    params
        .get(key)
        .and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_i64().map(|item| item as f64))
                .or_else(|| value.as_u64().map(|item| item as f64))
        })
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| value.round() as usize)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;

    fn live_config(params: Value) -> LiveStrategyConfig {
        LiveStrategyConfig {
            strategy_id: "test".to_string(),
            strategy_name: "Test".to_string(),
            symbol: "BTC-USDT-SWAP".to_string(),
            timeframe: "15m".to_string(),
            inst_type: "SWAP".to_string(),
            mode: "simulated".to_string(),
            initial_capital: 1_000.0,
            position_size: 0.25,
            stop_loss: 0.02,
            take_profit: 0.0,
            risk_timeframe: "1m".to_string(),
            check_interval: 60,
            params,
            project_root: std::path::PathBuf::new(),
            risk_control_enabled: true,
            max_single_loss_ratio: 0.05,
            max_position_pct: 1.0,
            max_order_value: 10_000.0,
        }
    }

    fn strategy_config(params: Value) -> StrategyConfig {
        StrategyConfig {
            strategy_id: "test".to_string(),
            strategy_name: "Test".to_string(),
            symbol: "BTC-USDT-SWAP".to_string(),
            inst_type: "SWAP".to_string(),
            timeframe: "15m".to_string(),
            initial_capital: 1_000.0,
            position_size: 0.25,
            stop_loss: 0.02,
            take_profit: 0.0,
            params,
        }
    }

    fn action_record(position_size: Option<f64>) -> StrategyActionRecord {
        StrategyActionRecord {
            action: "open_position".to_string(),
            side: "buy".to_string(),
            price: 100.0,
            reason: "test".to_string(),
            strength: 1.0,
            timestamp: 1,
            position_size,
        }
    }

    #[test]
    fn strategy_config_json_hides_engine_params_from_python_strategy() {
        let config = strategy_config(json!({
            "fast_window": 12,
            "threshold_lookback_15m": 20,
            "contract_mode": true,
            "leverage": 7,
            "td_mode": "isolated",
            "max_slippage_bps": 20,
            "risk_control_enabled": true,
            "max_same_direction_exposure_pct": 0.6,
            "fill_sync_symbol_limit": 5,
            "portfolio_selector": "topk"
        }));

        let payload = strategy_config_json_for_evaluate(&config);

        assert_eq!(payload["position_size"], 0.25);
        assert_eq!(payload["stop_loss"], 0.02);
        assert_eq!(payload["params"]["fast_window"], 12);
        assert_eq!(payload["params"]["threshold_lookback_15m"], 20);
        assert_eq!(payload["params"]["portfolio_selector"], "topk");
        assert!(payload["params"]["contract_mode"].is_null());
        assert!(payload["params"]["leverage"].is_null());
        assert!(payload["params"]["td_mode"].is_null());
        assert!(payload["params"]["max_slippage_bps"].is_null());
        assert!(payload["params"]["risk_control_enabled"].is_null());
        assert!(payload["params"]["max_same_direction_exposure_pct"].is_null());
        assert!(payload["params"]["fill_sync_symbol_limit"].is_null());
    }

    #[test]
    fn margin_fraction_position_size_uses_leveraged_notional() {
        let config = live_config(json!({"contract_mode": true, "leverage": 3}));

        let quantity = order_quantity(&config, &action_record(Some(0.25)));

        assert_eq!(quantity, 7.5);
    }

    #[test]
    fn notional_position_size_mode_does_not_multiply_order_value_by_leverage() {
        let config = live_config(json!({
            "contract_mode": true,
            "leverage": 3,
            "position_size_mode": "notional"
        }));

        let quantity = order_quantity(&config, &action_record(Some(0.25)));

        assert_eq!(quantity, 2.5);
    }

    #[test]
    fn invalid_entry_side_does_not_default_to_buy() {
        assert_eq!(entry_order_side("invalid-side"), "unknown");
    }
}
