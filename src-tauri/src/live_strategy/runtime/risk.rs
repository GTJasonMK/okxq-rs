use serde_json::Value;

use crate::{
    error::{AppError, AppResult},
    okx::{
        okx_account_equity, okx_finite_value, okx_position_notional, okx_position_side_dir,
        okx_value_text, OkxPrivateClient,
    },
    risk_controls::{self, PositionRiskSnapshot, RuntimeRiskCheckConfig, RuntimeRiskState},
};

use super::super::{types::LiveStrategyConfig, LiveStrategyRuntime};

impl LiveStrategyRuntime {
    #[cfg(test)]
    pub(crate) fn check_risk_controls(
        config: &LiveStrategyConfig,
        side: &str,
        price: f64,
        quantity: f64,
    ) -> (bool, String) {
        Self::check_risk_controls_with_state(
            config,
            side,
            price,
            quantity,
            &RuntimeRiskState::default(),
        )
    }

    pub(crate) fn check_risk_controls_with_state(
        config: &LiveStrategyConfig,
        side: &str,
        price: f64,
        quantity: f64,
        state: &RuntimeRiskState,
    ) -> (bool, String) {
        risk_controls::check_runtime_risk_controls(
            runtime_risk_check_config(config),
            side,
            price,
            quantity,
            state,
        )
    }

    pub(crate) async fn exchange_runtime_risk_state(
        config: &LiveStrategyConfig,
        private_client: &OkxPrivateClient,
    ) -> AppResult<RuntimeRiskState> {
        let initial_capital = positive(config.initial_capital).unwrap_or(0.0);
        let mut state = RuntimeRiskState {
            initial_capital,
            day_start_equity: initial_capital,
            current_equity: initial_capital,
            positions: Vec::new(),
        };

        if risk_controls::max_daily_loss_ratio(&config.params).is_some() {
            let account_items = private_client.get_account_balance().await?;
            state.current_equity = okx_account_equity(&account_items).ok_or_else(|| {
                AppError::Runtime("OKX 账户权益缺失，无法评估日内亏损风控".to_string())
            })?;
        }

        if risk_controls::max_same_direction_exposure_pct(&config.params).is_some() {
            let inst_type = config.inst_type.trim().to_uppercase();
            let positions = private_client
                .get_positions((!inst_type.is_empty()).then_some(inst_type.as_str()), None)
                .await?;
            state.positions = position_risk_snapshots(&positions)?;
        }

        Ok(state)
    }
}

fn position_risk_snapshots(items: &[Value]) -> AppResult<Vec<PositionRiskSnapshot>> {
    let mut snapshots = Vec::new();
    for item in items {
        let symbol = okx_value_text(item, "instId").trim().to_uppercase();
        if symbol.is_empty() {
            continue;
        }
        let pos = okx_finite_value(item, "pos").ok_or_else(|| {
            AppError::Runtime(format!("OKX 持仓 {symbol} 缺少有效 pos，无法评估敞口风控"))
        })?;
        if pos.abs() <= f64::EPSILON {
            continue;
        }
        let notional =
            okx_position_notional(item, pos.abs(), &["markPx", "avgPx"]).ok_or_else(|| {
                AppError::Runtime(format!(
                    "OKX 持仓 {symbol} 缺少有效 notionalUsd/markPx/avgPx，无法评估敞口风控"
                ))
            })?;
        snapshots.push(PositionRiskSnapshot {
            symbol,
            side_dir: okx_position_side_dir(item, pos),
            notional,
        });
    }
    Ok(snapshots)
}

fn runtime_risk_check_config(config: &LiveStrategyConfig) -> RuntimeRiskCheckConfig<'_> {
    RuntimeRiskCheckConfig::from_strategy_params(
        &config.params,
        &config.symbol,
        &config.inst_type,
        config.initial_capital,
        config.stop_loss,
    )
    .with_runtime_limits(
        config.risk_control_enabled,
        config.max_single_loss_ratio,
        config.max_position_pct,
        config.max_order_value,
    )
}

fn positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::risk_controls::PositionRiskSnapshot;
    use serde_json::json;

    use super::*;

    #[test]
    fn single_loss_risk_uses_order_notional_not_raw_stop_loss_pct() {
        let config = config_with_risk(0.10, 0.05);

        let (passed, reason) = LiveStrategyRuntime::check_risk_controls(&config, "buy", 100.0, 1.0);

        assert!(passed, "small order should pass: {reason}");
    }

    #[test]
    fn single_loss_risk_blocks_when_notional_loss_exceeds_budget() {
        let config = config_with_risk(0.10, 0.05);

        let (passed, reason) = LiveStrategyRuntime::check_risk_controls(&config, "buy", 100.0, 6.0);

        assert!(!passed);
        assert!(reason.contains("潜在亏损比例 6.00%"));
    }

    #[test]
    fn risk_blocks_when_stop_loss_is_required_but_missing() {
        let mut config = config_with_risk(0.0, 0.05);
        config.params = json!({"require_stop_loss": true});

        let (passed, reason) = LiveStrategyRuntime::check_risk_controls(&config, "buy", 100.0, 1.0);

        assert!(!passed);
        assert!(reason.contains("保护性止损"));
    }

    #[test]
    fn risk_blocks_when_configured_leverage_exceeds_limit() {
        let mut config = config_with_risk(0.02, 0.05);
        config.params = json!({"contract_mode": true, "leverage": 5, "max_leverage": 3});

        let (passed, reason) = LiveStrategyRuntime::check_risk_controls(&config, "buy", 100.0, 1.0);

        assert!(!passed);
        assert!(reason.contains("杠杆 5.00x"));
    }

    #[test]
    fn risk_blocks_non_finite_or_empty_orders_before_optional_limits() {
        let mut config = config_with_risk(0.02, 0.05);
        config.risk_control_enabled = false;

        let (nan_price_passed, nan_price_reason) =
            LiveStrategyRuntime::check_risk_controls(&config, "buy", f64::NAN, 1.0);
        let (zero_quantity_passed, zero_quantity_reason) =
            LiveStrategyRuntime::check_risk_controls(&config, "buy", 100.0, 0.0);

        assert!(!nan_price_passed);
        assert!(nan_price_reason.contains("订单价格或数量无效"));
        assert!(!zero_quantity_passed);
        assert!(zero_quantity_reason.contains("订单价格或数量无效"));
    }

    #[test]
    fn risk_blocks_when_daily_loss_limit_is_reached() {
        let mut config = config_with_risk(0.02, 0.05);
        config.params = json!({"max_daily_loss_ratio": 0.05});
        let state = RuntimeRiskState {
            initial_capital: 1_000.0,
            day_start_equity: 1_000.0,
            current_equity: 940.0,
            positions: Vec::new(),
        };

        let (passed, reason) =
            LiveStrategyRuntime::check_risk_controls_with_state(&config, "buy", 100.0, 1.0, &state);

        assert!(!passed);
        assert!(reason.contains("日内亏损比例 6.00%"));
    }

    #[test]
    fn risk_blocks_when_same_direction_exposure_exceeds_limit() {
        let mut config = config_with_risk(0.02, 0.05);
        config.params = json!({"max_same_direction_exposure_pct": 0.60});
        let state = RuntimeRiskState {
            initial_capital: 1_000.0,
            day_start_equity: 1_000.0,
            current_equity: 1_000.0,
            positions: vec![PositionRiskSnapshot {
                symbol: "ETH-USDT-SWAP".to_string(),
                side_dir: 1,
                notional: 500.0,
            }],
        };

        let (passed, reason) =
            LiveStrategyRuntime::check_risk_controls_with_state(&config, "buy", 100.0, 2.0, &state);

        assert!(!passed);
        assert!(reason.contains("同向相关敞口比例 70.00%"));
    }

    #[test]
    fn position_risk_prefers_okx_notional_usd_for_contract_positions() {
        let snapshots = position_risk_snapshots(&[json!({
            "instId": "BTC-USDT-SWAP",
            "posSide": "long",
            "pos": "5",
            "markPx": "100",
            "notionalUsd": "50"
        })])
        .expect("position risk snapshot should parse");

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].notional, 50.0);
        assert_eq!(snapshots[0].side_dir, 1);
    }

    fn config_with_risk(stop_loss: f64, max_single_loss_ratio: f64) -> LiveStrategyConfig {
        LiveStrategyConfig {
            strategy_id: "risk_test".to_string(),
            strategy_name: "Risk Test".to_string(),
            symbol: "BTC-USDT-SWAP".to_string(),
            timeframe: "15m".to_string(),
            inst_type: "SWAP".to_string(),
            mode: "simulated".to_string(),
            initial_capital: 1_000.0,
            position_size: 0.1,
            stop_loss,
            take_profit: 0.0,
            risk_timeframe: "1m".to_string(),
            check_interval: 60,
            params: json!({}),
            project_root: PathBuf::new(),
            risk_control_enabled: true,
            max_single_loss_ratio,
            max_position_pct: 1.0,
            max_order_value: 10_000.0,
        }
    }
}
