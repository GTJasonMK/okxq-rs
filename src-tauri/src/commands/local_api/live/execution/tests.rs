use serde_json::{json, Value};

use super::decision::{
    build_execution_decision, build_execution_decision_with_exchange_risk_evidence,
    ExchangeRiskEvidence,
};
use crate::{
    live_strategy::{LiveStrategyConfig, LiveStrategyStatus},
    strategy_executor::types::RuntimeStrategyAction,
};

#[test]
fn execution_decision_reports_ready_exchange_demo_entry() {
    let config = live_config();
    let status = running_status();
    let actions = entry_actions();
    let decision = build_execution_decision(&actions, &config, &status, &latest_candle());

    assert_eq!(decision["verdict"].as_str(), Some("ready"));
    assert_eq!(decision["executable_intent_count"].as_u64(), Some(1));
    assert_eq!(decision["skipped_action_count"].as_u64(), Some(0));
    assert!(!has_blocking_gate(&decision));
}

#[test]
fn execution_decision_counts_attached_risk_actions_as_one_execution_intent() {
    let config = live_config();
    let status = running_status();
    let actions = runtime_actions(json!([
        {
            "action": "open_position",
            "side": "sell",
            "price": 100.0,
            "position_size": 0.25,
            "reason": "unit_test_entry",
            "timestamp": 1_700_000_000_000_i64
        },
        {
            "action": "place_risk_order",
            "side": "buy",
            "order_type": "stop_market",
            "trigger_price": 105.0,
            "reason": "attached_stop",
            "timestamp": 1_700_000_000_000_i64
        }
    ]));

    let decision = build_execution_decision(&actions, &config, &status, &latest_candle());

    assert_eq!(decision["verdict"].as_str(), Some("ready"));
    assert_eq!(decision["executable_intent_count"].as_u64(), Some(1));
    assert_eq!(decision["risk_action_count"].as_u64(), Some(1));
}

#[test]
fn execution_decision_blocks_contract_mode_false_for_contract_runtime() {
    let mut config = live_config();
    config.params = json!({"contract_mode": false, "leverage": 3, "td_mode": "cross"});
    let status = running_status();
    let actions = entry_actions();

    let decision = build_execution_decision(&actions, &config, &status, &latest_candle());

    assert_eq!(decision["verdict"].as_str(), Some("blocked"));
    assert!(has_blocking_gate_key(&decision, "execution_params"));
    assert!(decision["gates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gate| gate["key"] == "execution_params"
            && gate["detail"]
                .as_str()
                .unwrap_or("")
                .contains("contract_mode=false")));
}

#[test]
fn execution_decision_validates_execution_params_after_action_inst_type_override() {
    let mut config = live_config();
    config.params = json!({"contract_mode": true, "leverage": 3, "td_mode": "cross"});
    let status = running_status();
    let actions = runtime_actions(json!([{
        "action": "open_position",
        "symbol": "ETH-USDT",
        "inst_type": "SPOT",
        "side": "long",
        "price": 100.0,
        "position_size": 0.25,
        "reason": "spot_override",
        "timestamp": 1_700_000_000_000_i64
    }]));

    let decision = build_execution_decision(&actions, &config, &status, &latest_candle());

    assert_eq!(decision["verdict"].as_str(), Some("blocked"));
    assert!(has_blocking_gate_key(&decision, "execution_params"));
    assert!(decision["gates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gate| gate["key"] == "execution_params"
            && gate["detail"]
                .as_str()
                .unwrap_or("")
                .contains("ETH-USDT SPOT")
            && gate["detail"]
                .as_str()
                .unwrap_or("")
                .contains("contract_mode=true")));
}

#[test]
fn execution_decision_blocks_invalid_leverage_before_runtime_submit() {
    let mut config = live_config();
    config.params = json!({"contract_mode": true, "leverage": 200, "td_mode": "isolated"});
    let status = running_status();
    let actions = entry_actions();

    let decision = build_execution_decision(&actions, &config, &status, &latest_candle());

    assert_eq!(decision["verdict"].as_str(), Some("blocked"));
    assert!(has_blocking_gate_key(&decision, "execution_params"));
    assert!(decision["gates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gate| gate["key"] == "execution_params"
            && gate["detail"].as_str().unwrap_or("").contains("125")));
}

#[test]
fn execution_decision_does_not_apply_open_leverage_validation_to_close() {
    let mut config = live_config();
    config.params = json!({"contract_mode": true, "leverage": 200, "td_mode": "isolated"});
    let status = running_status();
    let actions = runtime_actions(json!([{
        "action": "close_position",
        "side": "buy",
        "order_type": "market",
        "timestamp": 1_700_000_000_000_i64
    }]));

    let decision = build_execution_decision(&actions, &config, &status, &latest_candle());

    assert_eq!(decision["verdict"].as_str(), Some("ready"));
    assert!(!has_blocking_gate(&decision));
    assert!(decision["gates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gate| gate["key"] == "execution_params" && gate["status"] == "pass"));
}

#[test]
fn execution_decision_skips_trading_params_for_cancel_order() {
    let mut config = live_config();
    config.params = json!({"contract_mode": false, "leverage": 200, "td_mode": "portfolio"});
    let status = running_status();
    let actions = runtime_actions(json!([{
        "action": "cancel_order",
        "order_id": "order-1",
        "timestamp": 1_700_000_000_000_i64
    }]));

    let decision = build_execution_decision(&actions, &config, &status, &latest_candle());

    assert_eq!(decision["verdict"].as_str(), Some("ready"));
    assert_eq!(decision["executable_intent_count"].as_u64(), Some(1));
    assert!(!has_blocking_gate(&decision));
    assert!(decision["gates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gate| gate["key"] == "execution_params" && gate["status"] == "skip"));
}

#[test]
fn execution_decision_reports_hold_without_actions() {
    let config = live_config();
    let status = running_status();
    let actions = vec![];

    let decision = build_execution_decision(&actions, &config, &status, &latest_candle());

    assert_eq!(decision["verdict"].as_str(), Some("hold"));
}

#[test]
fn execution_decision_blocks_stateful_risk_without_okx_evidence() {
    let mut config = live_config();
    config.params = json!({
        "contract_mode": true,
        "leverage": 3,
        "max_same_direction_exposure_pct": 0.80
    });
    let status = running_status();
    let actions = entry_actions();

    let decision = build_execution_decision(&actions, &config, &status, &latest_candle());

    assert_eq!(decision["verdict"].as_str(), Some("blocked"));
    assert!(has_blocking_gate_key(&decision, "risk"));
    assert!(
        decision["gates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|gate| gate["key"] == "risk"
                && gate["detail"].as_str().unwrap_or("").contains("OKX"))
    );
}

#[test]
fn execution_decision_stateful_risk_still_requires_okx_evidence_when_legacy_risk_toggle_off() {
    let mut config = live_config();
    config.risk_control_enabled = false;
    config.params = json!({
        "contract_mode": true,
        "leverage": 3,
        "max_same_direction_exposure_pct": 0.80
    });
    let status = running_status();
    let actions = entry_actions();

    let decision = build_execution_decision(&actions, &config, &status, &latest_candle());

    assert_eq!(decision["verdict"].as_str(), Some("blocked"));
    assert!(has_blocking_gate_key(&decision, "risk"));
}

#[test]
fn execution_decision_accepts_okx_risk_state_evidence_for_same_direction_guard() {
    let mut config = live_config();
    config.params = json!({
        "contract_mode": true,
        "leverage": 3,
        "max_same_direction_exposure_pct": 0.80
    });
    let status = running_status();
    let evidence = ExchangeRiskEvidence::State;
    let actions = entry_actions();

    let decision = build_execution_decision_with_exchange_risk_evidence(
        &actions,
        &config,
        &status,
        &latest_candle(),
        Some(&evidence),
    );

    assert_eq!(decision["verdict"].as_str(), Some("ready"));
    assert!(!decision["gates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gate| gate["key"] == "risk"));
}

#[test]
fn execution_decision_allows_market_close_intent_without_reference_price() {
    let config = live_config();
    let status = running_status();
    let actions = runtime_actions(json!([{
        "action": "close_position",
        "side": "buy",
        "order_type": "market",
        "timestamp": 1_700_000_000_000_i64
    }]));
    let decision = build_execution_decision(&actions, &config, &status, &latest_candle());

    assert_eq!(decision["verdict"].as_str(), Some("ready"));
    assert!(!has_blocking_gate(&decision));
}

#[test]
fn execution_decision_blocks_action_contract_that_runtime_would_skip() {
    let config = live_config();
    let status = running_status();
    let actions = runtime_actions(json!([{
        "action": "open_position",
        "side": "sell",
        "price": 100.0,
        "position_size": 0.25
    }]));
    let decision = build_execution_decision(&actions, &config, &status, &latest_candle());

    assert_eq!(decision["verdict"].as_str(), Some("blocked"));
    assert_eq!(decision["executable_intent_count"].as_u64(), Some(0));
    assert_eq!(decision["skipped_action_count"].as_u64(), Some(1));
    assert!(decision["gates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gate| gate["key"] == "actions"
            && gate["detail"]
                .as_str()
                .unwrap_or("")
                .contains("缺少有效 timestamp")));
}

#[test]
fn execution_decision_zeroes_executable_counts_for_mixed_invalid_batch() {
    let config = live_config();
    let status = running_status();
    let actions = runtime_actions(json!([
        {
            "action": "place_risk_order",
            "side": "sell",
            "order_type": "stop_market",
            "trigger_price": 95.0,
            "stop_loss_bps": 500,
            "timestamp": 1_700_000_000_000_i64,
            "reason": "valid_risk_order"
        },
        {
            "action": "modify_order",
            "side": "hold",
            "order_id": "order-1",
            "new_px": "101.2",
            "timestamp": 1_700_000_000_000_i64,
            "reason": "legacy_alias_blocks_batch"
        }
    ]));
    let decision = build_execution_decision(&actions, &config, &status, &latest_candle());

    assert_eq!(decision["verdict"].as_str(), Some("blocked"));
    assert_eq!(decision["executable_intent_count"].as_u64(), Some(0));
    assert_eq!(decision["risk_action_count"].as_u64(), Some(0));
    assert_eq!(decision["skipped_action_count"].as_u64(), Some(1));
    assert!(has_blocking_gate_key(&decision, "actions"));
}

fn live_config() -> LiveStrategyConfig {
    LiveStrategyConfig {
        strategy_id: "multi_timeframe_dual_v12".to_string(),
        strategy_name: "V20".to_string(),
        symbol: "BTC-USDT-SWAP".to_string(),
        timeframe: "15m".to_string(),
        inst_type: "SWAP".to_string(),
        mode: "simulated".to_string(),
        initial_capital: 1000.0,
        position_size: 0.25,
        stop_loss: 0.02,
        take_profit: 0.0,
        risk_timeframe: "1m".to_string(),
        check_interval: 60,
        params: json!({"contract_mode": true, "leverage": 3}),
        project_root: std::path::PathBuf::new(),
        risk_control_enabled: true,
        max_single_loss_ratio: 0.05,
        max_position_pct: 1.0,
        max_order_value: 10_000.0,
    }
}

fn running_status() -> LiveStrategyStatus {
    LiveStrategyStatus {
        status: "running".to_string(),
        run_id: "run".to_string(),
        mode: "simulated".to_string(),
        strategy_id: "multi_timeframe_dual_v12".to_string(),
        strategy_name: "V20".to_string(),
        symbol: "BTC-USDT-SWAP".to_string(),
        timeframe: "15m".to_string(),
        inst_type: "SWAP".to_string(),
        ..LiveStrategyStatus::default()
    }
}

fn entry_actions() -> Vec<RuntimeStrategyAction> {
    runtime_actions(json!([{
        "action": "open_position",
        "side": "sell",
        "price": 100.0,
        "position_size": 0.25,
        "reason": "unit_test_entry",
        "timestamp": 1_700_000_000_000_i64
    }]))
}

fn runtime_actions(value: Value) -> Vec<RuntimeStrategyAction> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(RuntimeStrategyAction::from_value)
        .collect()
}

fn has_blocking_gate(decision: &Value) -> bool {
    decision["gates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gate| gate["blocking"].as_bool() == Some(true))
}

fn has_blocking_gate_key(decision: &Value, key: &str) -> bool {
    decision["gates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gate| gate["key"].as_str() == Some(key) && gate["blocking"].as_bool() == Some(true))
}

fn latest_candle() -> crate::okx::OkxCandle {
    crate::okx::OkxCandle {
        timestamp: 1_700_000_000_000,
        open: 100.0,
        high: 101.0,
        low: 99.0,
        close: 100.0,
        volume: 1.0,
        volume_ccy: 100.0,
        volume_quote: 100.0,
        confirm: "1".to_string(),
    }
}
