use serde_json::Value;

use crate::trading_semantics::{action_position_side, is_contract_inst_type};

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeRiskState {
    pub(crate) initial_capital: f64,
    pub(crate) day_start_equity: f64,
    pub(crate) current_equity: f64,
    pub(crate) positions: Vec<PositionRiskSnapshot>,
}

#[derive(Clone, Debug)]
pub(crate) struct PositionRiskSnapshot {
    pub(crate) symbol: String,
    pub(crate) side_dir: i32,
    pub(crate) notional: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeRiskCheckConfig<'a> {
    pub(crate) params: &'a Value,
    pub(crate) symbol: &'a str,
    pub(crate) inst_type: &'a str,
    pub(crate) initial_capital: f64,
    pub(crate) stop_loss: f64,
    pub(crate) risk_control_enabled: bool,
    pub(crate) max_single_loss_ratio: f64,
    pub(crate) max_position_pct: f64,
    pub(crate) max_order_value: f64,
}

impl<'a> RuntimeRiskCheckConfig<'a> {
    pub(crate) fn from_strategy_params(
        params: &'a Value,
        symbol: &'a str,
        inst_type: &'a str,
        initial_capital: f64,
        stop_loss: f64,
    ) -> Self {
        Self {
            params,
            symbol,
            inst_type,
            initial_capital,
            stop_loss,
            risk_control_enabled: bool_param(params, "risk_control_enabled").unwrap_or(true),
            max_single_loss_ratio: numeric_param(params, "max_single_loss_ratio").unwrap_or(0.0),
            max_position_pct: max_symbol_exposure_pct(params).unwrap_or(0.0),
            max_order_value: numeric_param(params, "max_order_value").unwrap_or(0.0),
        }
    }

    pub(crate) fn with_runtime_limits(
        mut self,
        risk_control_enabled: bool,
        max_single_loss_ratio: f64,
        max_position_pct: f64,
        max_order_value: f64,
    ) -> Self {
        self.risk_control_enabled = risk_control_enabled;
        self.max_single_loss_ratio = max_single_loss_ratio;
        self.max_position_pct = max_position_pct;
        self.max_order_value = max_order_value;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PositionSizeMode {
    MarginFraction,
    NotionalFraction,
}

pub(crate) fn check_runtime_risk_controls(
    config: RuntimeRiskCheckConfig<'_>,
    side: &str,
    price: f64,
    quantity: f64,
    state: &RuntimeRiskState,
) -> (bool, String) {
    if !price.is_finite() || price <= 0.0 || !quantity.is_finite() || quantity <= 0.0 {
        return (false, "订单价格或数量无效".to_string());
    }
    let Some((_, side_dir)) = action_position_side(side) else {
        return (
            false,
            format!("交易方向 {side} 无法用于风控检查，已拦截订单"),
        );
    };

    let stop_loss = normalized_stop_loss(config.stop_loss);
    if require_stop_loss(config.params) && stop_loss <= 0.0 {
        return (
            false,
            "策略要求保护性止损，但当前开仓意图没有 stop_loss/stop_loss_bps 或保护性风险订单"
                .to_string(),
        );
    }

    let leverage = configured_leverage(config.params, config.inst_type);
    if let Some(max_leverage) = max_leverage(config.params) {
        if leverage > max_leverage {
            return (
                false,
                format!("杠杆 {:.2}x 超过风控上限 {:.2}x", leverage, max_leverage),
            );
        }
    }

    let order_value = quantity * price;
    if let Some(max_daily_loss_ratio) = max_daily_loss_ratio(config.params) {
        let day_start = positive(state.day_start_equity)
            .or_else(|| positive(config.initial_capital))
            .unwrap_or(0.0);
        let current_equity = positive(state.current_equity)
            .or_else(|| positive(day_start))
            .unwrap_or(0.0);
        if day_start > 0.0 {
            let daily_loss_ratio = ((day_start - current_equity).max(0.0)) / day_start;
            if daily_loss_ratio >= max_daily_loss_ratio {
                return (
                    false,
                    format!(
                        "日内亏损比例 {:.2}% 达到风控上限 {:.2}%",
                        daily_loss_ratio * 100.0,
                        max_daily_loss_ratio * 100.0
                    ),
                );
            }
        }
    }

    if let Some(max_same_direction_exposure_pct) = max_same_direction_exposure_pct(config.params) {
        let capital = positive(state.initial_capital)
            .or_else(|| positive(config.initial_capital))
            .unwrap_or(0.0);
        if side_dir != 0 && capital > 0.0 {
            let existing = same_direction_exposure_notional(
                config.params,
                &state.positions,
                config.symbol,
                side_dir,
            );
            let exposure_ratio = (existing + order_value) / capital;
            if exposure_ratio > max_same_direction_exposure_pct {
                return (
                    false,
                    format!(
                        "同向相关敞口比例 {:.2}% 超过风控上限 {:.2}%",
                        exposure_ratio * 100.0,
                        max_same_direction_exposure_pct * 100.0
                    ),
                );
            }
        }
    }

    if !config.risk_control_enabled {
        return (true, String::new());
    }

    let position_ratio = order_value / config.initial_capital;
    let max_position_pct = max_symbol_exposure_pct(config.params)
        .unwrap_or(config.max_position_pct)
        .clamp(0.0, 100.0);
    if position_ratio > max_position_pct && max_position_pct > 0.0 {
        return (
            false,
            format!(
                "单币敞口比例 {:.2}% 超过风控上限 {:.2}%",
                position_ratio * 100.0,
                max_position_pct * 100.0
            ),
        );
    }

    if config.max_order_value > 0.0 && order_value > config.max_order_value {
        return (
            false,
            format!(
                "订单金额 {:.2} USDT 超过风控上限 {:.2} USDT",
                order_value, config.max_order_value
            ),
        );
    }

    let potential_loss_ratio = if config.initial_capital > 0.0 && stop_loss > 0.0 {
        order_value * stop_loss / config.initial_capital
    } else {
        0.0
    };
    if potential_loss_ratio > config.max_single_loss_ratio && config.max_single_loss_ratio > 0.0 {
        return (
            false,
            format!(
                "潜在亏损比例 {:.2}% 超过风控单笔最大亏损 {:.2}%",
                potential_loss_ratio * 100.0,
                config.max_single_loss_ratio * 100.0
            ),
        );
    }

    (true, String::new())
}

pub(crate) fn configured_leverage(params: &Value, inst_type: &str) -> f64 {
    let contract_mode =
        bool_param(params, "contract_mode").unwrap_or_else(|| is_contract_inst_type(inst_type));
    if contract_mode {
        numeric_param(params, "leverage")
            .unwrap_or(1.0)
            .clamp(1.0, 125.0)
    } else {
        1.0
    }
}

pub(crate) fn allow_short(params: &Value, inst_type: &str) -> bool {
    bool_param(params, "allow_short").unwrap_or_else(|| is_contract_inst_type(inst_type))
}

pub(crate) fn position_size_mode(params: &Value) -> PositionSizeMode {
    if bool_param(params, "position_size_is_notional")
        .or_else(|| bool_param(params, "position_size_is_effective_notional"))
        .unwrap_or(false)
    {
        return PositionSizeMode::NotionalFraction;
    }
    let mode = params
        .get("position_size_mode")
        .or_else(|| params.get("position_sizing"))
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();
    match mode.as_str() {
        "notional" | "effective_notional" | "effective_exposure" | "exposure" => {
            PositionSizeMode::NotionalFraction
        }
        _ => PositionSizeMode::MarginFraction,
    }
}

fn normalized_stop_loss(stop_loss: f64) -> f64 {
    if stop_loss.is_finite() {
        stop_loss.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

pub(crate) fn effective_position_size(requested: Option<f64>, configured: f64) -> f64 {
    let value = requested
        .filter(|value| value.is_finite())
        .or_else(|| configured.is_finite().then_some(configured))
        .unwrap_or(0.2);
    value.clamp(0.01, 1.0)
}

pub(crate) fn position_notional(
    params: &Value,
    inst_type: &str,
    capital: f64,
    position_size: f64,
) -> f64 {
    match position_size_mode(params) {
        PositionSizeMode::MarginFraction => {
            capital * position_size * configured_leverage(params, inst_type)
        }
        PositionSizeMode::NotionalFraction => capital * position_size,
    }
}

pub(crate) fn runtime_order_base_quantity(
    params: &Value,
    inst_type: &str,
    initial_capital: f64,
    configured_position_size: f64,
    action_position_size: Option<f64>,
    price: f64,
) -> Option<f64> {
    if !price.is_finite() || price <= 0.0 {
        return None;
    }
    let position_size = effective_position_size(action_position_size, configured_position_size);
    let quantity = position_notional(params, inst_type, initial_capital, position_size) / price;
    (quantity.is_finite() && quantity > 0.0).then_some(quantity)
}

pub(crate) fn max_leverage(params: &Value) -> Option<f64> {
    numeric_param_any(params, &["max_leverage", "max_allowed_leverage"])
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.clamp(1.0, 125.0))
}

pub(crate) fn max_symbol_exposure_pct(params: &Value) -> Option<f64> {
    numeric_param(params, "max_symbol_exposure_pct")
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.clamp(0.0, 100.0))
}

pub(crate) fn max_same_direction_exposure_pct(params: &Value) -> Option<f64> {
    numeric_param_any(
        params,
        &[
            "max_same_direction_exposure_pct",
            "max_correlated_exposure_pct",
            "max_same_side_exposure_pct",
        ],
    )
    .filter(|value| value.is_finite() && *value > 0.0)
    .map(|value| value.clamp(0.0, 100.0))
}

pub(crate) fn max_daily_loss_ratio(params: &Value) -> Option<f64> {
    numeric_param_any(
        params,
        &[
            "max_daily_loss_ratio",
            "daily_loss_limit_ratio",
            "daily_loss_limit",
        ],
    )
    .or_else(|| numeric_param(params, "max_daily_loss_bps").map(|value| value / 10_000.0))
    .filter(|value| value.is_finite() && *value > 0.0)
    .map(|value| value.clamp(0.0, 1.0))
}

pub(crate) fn require_stop_loss(params: &Value) -> bool {
    bool_param_any(
        params,
        &[
            "require_stop_loss",
            "require_protective_stop",
            "require_protective_risk_order",
            "stop_loss_required",
        ],
    )
    .unwrap_or(false)
}

pub(crate) fn risk_decimal(
    value: &Value,
    bps_key: &str,
    pct_key: &str,
    decimal_key: &str,
) -> Option<f64> {
    numeric_param(value, bps_key)
        .map(|value| value / 10_000.0)
        .or_else(|| numeric_param(value, pct_key))
        .or_else(|| numeric_param(value, decimal_key))
        .filter(|value| value.is_finite() && *value > 0.0)
}

pub(crate) fn same_direction_exposure_notional(
    params: &Value,
    positions: &[PositionRiskSnapshot],
    symbol: &str,
    side_dir: i32,
) -> f64 {
    if side_dir == 0 {
        return 0.0;
    }
    let target_group = correlation_group(params, symbol);
    positions
        .iter()
        .filter(|position| position.side_dir == side_dir)
        .filter(|position| {
            if let Some(group) = &target_group {
                correlation_group(params, &position.symbol).as_ref() == Some(group)
            } else {
                true
            }
        })
        .filter_map(|position| {
            (position.notional.is_finite() && position.notional > 0.0).then_some(position.notional)
        })
        .sum()
}

pub(crate) fn trading_day(timestamp: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(timestamp)
        .map(|dt| {
            (dt + chrono::Duration::hours(8))
                .format("%Y-%m-%d")
                .to_string()
        })
        .unwrap_or_else(|| {
            (chrono::Utc::now() + chrono::Duration::hours(8))
                .format("%Y-%m-%d")
                .to_string()
        })
}

fn correlation_group(params: &Value, symbol: &str) -> Option<String> {
    let symbol = symbol.trim().to_ascii_uppercase();
    if symbol.is_empty() {
        return None;
    }
    let groups = params.get("correlation_groups")?;
    if let Some(object) = groups.as_object() {
        for (group, value) in object {
            if value_matches_symbol(value, &symbol) {
                return Some(group.to_ascii_lowercase());
            }
            if group.eq_ignore_ascii_case(&symbol) {
                if let Some(group_name) = value.as_str().filter(|item| !item.trim().is_empty()) {
                    return Some(group_name.trim().to_ascii_lowercase());
                }
            }
        }
    }
    if let Some(items) = groups.as_array() {
        for item in items {
            let Some(object) = item.as_object() else {
                continue;
            };
            let Some(group_name) = object
                .get("name")
                .or_else(|| object.get("group"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
            else {
                continue;
            };
            if object
                .get("symbols")
                .is_some_and(|value| value_matches_symbol(value, &symbol))
            {
                return Some(group_name.trim().to_ascii_lowercase());
            }
        }
    }
    None
}

fn value_matches_symbol(value: &Value, symbol: &str) -> bool {
    if let Some(items) = value.as_array() {
        return items.iter().any(|item| {
            item.as_str()
                .is_some_and(|text| text.trim().eq_ignore_ascii_case(symbol))
        });
    }
    if let Some(object) = value.as_object() {
        return object
            .get("symbols")
            .is_some_and(|items| value_matches_symbol(items, symbol));
    }
    value
        .as_str()
        .is_some_and(|text| text.trim().eq_ignore_ascii_case(symbol))
}

pub(crate) fn numeric_param_any(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| numeric_param(value, key))
}

pub(crate) fn numeric_param(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(|item| {
            item.as_f64()
                .or_else(|| item.as_i64().map(|value| value as f64))
                .or_else(|| item.as_u64().map(|value| value as f64))
                .or_else(|| item.as_str()?.trim().parse::<f64>().ok())
        })
        .filter(|value| value.is_finite())
}

pub(crate) fn bool_param_any(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| bool_param(value, key))
}

pub(crate) fn bool_param(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(|item| {
        item.as_bool().or_else(|| {
            let text = item.as_str()?.trim().to_ascii_lowercase();
            match text.as_str() {
                "true" | "1" | "yes" | "on" => Some(true),
                "false" | "0" | "no" | "off" => Some(false),
                _ => None,
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn futures_uses_contract_leverage_without_contract_mode_override() {
        let params = json!({"leverage": 3});

        assert_eq!(configured_leverage(&params, "FUTURES"), 3.0);
        assert_eq!(position_notional(&params, "FUTURES", 1_000.0, 0.25), 750.0);
    }

    #[test]
    fn explicit_contract_mode_false_keeps_futures_unleveraged() {
        let params = json!({"contract_mode": false, "leverage": 3});

        assert_eq!(configured_leverage(&params, "FUTURES"), 1.0);
        assert_eq!(position_notional(&params, "FUTURES", 1_000.0, 0.25), 250.0);
    }

    #[test]
    fn spot_does_not_use_leverage_without_contract_mode_override() {
        let params = json!({"leverage": 3});

        assert_eq!(configured_leverage(&params, "SPOT"), 1.0);
        assert_eq!(position_notional(&params, "SPOT", 1_000.0, 0.25), 250.0);
    }

    #[test]
    fn runtime_order_base_quantity_uses_action_position_size_override() {
        let params = json!({"leverage": 3});

        assert_eq!(
            runtime_order_base_quantity(&params, "SWAP", 1_000.0, 0.25, Some(0.5), 100.0),
            Some(15.0)
        );
    }

    #[test]
    fn runtime_order_base_quantity_rejects_invalid_price() {
        let params = json!({"leverage": 3});

        assert_eq!(
            runtime_order_base_quantity(&params, "SWAP", 1_000.0, 0.25, None, 0.0),
            None
        );
    }

    #[test]
    fn runtime_risk_controls_reject_invalid_side_instead_of_skipping_directional_checks() {
        let params = json!({"max_same_direction_exposure_pct": 0.5});
        let config = RuntimeRiskCheckConfig {
            params: &params,
            symbol: "BTC-USDT-SWAP",
            inst_type: "SWAP",
            initial_capital: 1_000.0,
            stop_loss: 0.0,
            risk_control_enabled: true,
            max_single_loss_ratio: 0.0,
            max_position_pct: 0.0,
            max_order_value: 0.0,
        };

        let (passed, reason) = check_runtime_risk_controls(
            config,
            "sideways",
            100.0,
            1.0,
            &RuntimeRiskState::default(),
        );

        assert!(!passed);
        assert!(reason.contains("交易方向 sideways 无法用于风控检查"));
    }

    #[test]
    fn allow_short_defaults_to_contract_instruments_and_accepts_string_override() {
        assert!(allow_short(&json!({}), "SWAP"));
        assert!(allow_short(&json!({}), "FUTURES"));
        assert!(!allow_short(&json!({}), "SPOT"));
        assert!(!allow_short(&json!({"allow_short": "false"}), "SWAP"));
        assert!(allow_short(&json!({"allow_short": "true"}), "SPOT"));
    }
}
