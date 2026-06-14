use std::fs;

use serde_json::json;

use super::helpers::{
    prepare_python_runner, rising_candles, runtime_strategy_source, temp_project_root,
    write_runtime_strategy,
};
use crate::strategy_executor::{
    compute_runtime_decision, compute_runtime_decision_with_context,
    compute_runtime_decision_with_context_and_events,
    compute_runtime_decision_with_context_and_progress, compute_runtime_diagnostics,
    PythonRunnerSession,
};

const STRATEGY_FILE: &str = "runtime/runtime_contract_fixture.py";

#[test]
fn runtime_compute_injects_request_context_over_stale_params() {
    let root = runtime_fixture_root("runtime_compute_injects_request_context_over_stale_params");
    let config = json!({
        "strategy_id": "runtime_contract_fixture",
        "symbol": "ETH-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "3m",
        "params": {
            "_runtime_symbol": "BTC-USDT-SWAP",
            "_runtime_timeframe": "15m"
        }
    });
    let candles = rising_candles(1_780_290_000_000_i64, 3, 25);

    let decision = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles).unwrap();

    assert_eq!(decision.actions.len(), 1);
    assert_eq!(decision.actions[0].side, "long");
    assert_eq!(decision.actions[0].reason, "3m:ETH");
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_prefers_evaluate_context_actions() {
    let root = temp_project_root("runtime_decision_prefers_evaluate_context_actions");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candles = context["candles"][symbol][timeframe]
    orders = context["orders"]
    return {
        "actions": [
            {
                "action": "close_position",
                "symbol": symbol,
                "side": "flat",
                "order_type": "market",
                "reason": "evaluate_close",
                "strength": 0.4,
                "timestamp": candles[-1]["timestamp"],
            },
            {
                "action": "open_position",
                "symbol": symbol,
                "side": "long",
                "order_type": "market",
                "price": candles[-1]["close"],
                "position_size": 0.33,
                "reason": f"evaluate:{symbol}:{timeframe}",
                "strength": 0.9,
                "timestamp": candles[-1]["timestamp"],
                "planned_exit_time": candles[-1]["timestamp"] + 3600000,
                "planned_exit_reason": "max_hold_bars",
                "planned_hold_bars": 4,
                "hold_bars": 4,
                "entry_time": candles[-1]["timestamp"],
                "layer_id": "runner_contract_layer",
                "candidate_source": "runner_contract_source",
            },
        ],
        "diagnostics": {
            "summary": "evaluate path",
            "context_symbol": symbol,
            "has_order_state": "open" in orders and "recent_fills" in orders,
        },
        "indicators": {"seen_bars": [len(candles)]},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let decision = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles).unwrap();

    assert_eq!(decision.actions.len(), 2);
    assert_eq!(decision.actions[0].action, "close_position");
    assert_eq!(decision.actions[1].side, "long");
    assert_eq!(decision.actions[1].position_size, Some(0.33));
    assert_eq!(
        decision.actions[1].raw["planned_exit_time"].as_i64(),
        Some(1_780_290_000_000_i64 + 3 * 15 * 60_000 + 3_600_000)
    );
    assert_eq!(
        decision.actions[1].raw["planned_exit_reason"].as_str(),
        Some("max_hold_bars")
    );
    assert_eq!(
        decision.actions[1].raw["planned_hold_bars"].as_i64(),
        Some(4)
    );
    assert_eq!(
        decision.actions[1].raw["layer_id"].as_str(),
        Some("runner_contract_layer")
    );
    assert_eq!(
        decision.actions[1].raw["candidate_source"].as_str(),
        Some("runner_contract_source")
    );
    assert_eq!(
        decision.diagnostics["context_symbol"].as_str(),
        Some("BTC-USDT-SWAP")
    );
    assert_eq!(
        decision.diagnostics["has_order_state"].as_bool(),
        Some(true)
    );
    assert_eq!(decision.indicators["seen_bars"][0].as_i64(), Some(4));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_marks_default_symbol_as_not_explicit() {
    let root = temp_project_root("runtime_decision_marks_default_symbol_as_not_explicit");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candle = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [
            {
                "action": "cancel_order",
                "order_id": "default-scope-order",
                "target_order_kind": "any",
                "reason": "cancel_without_symbol",
                "timestamp": candle["timestamp"],
            },
            {
                "action": "modify_order",
                "symbol": "ETH-USDT-SWAP",
                "order_id": "explicit-scope-order",
                "client_order_id": "explicit-scope-client",
                "new_price": "101.25",
                "new_size": "2",
                "target_order_kind": "algo",
                "target_order_type": "stop_market",
                "reason": "modify_with_symbol",
                "timestamp": candle["timestamp"],
            },
        ],
        "diagnostics": {},
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let decision = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles).unwrap();

    assert_eq!(decision.actions.len(), 2);
    assert_eq!(decision.actions[0].symbol, "BTC-USDT-SWAP");
    assert_eq!(
        decision.actions[0].raw["_symbol_explicit"].as_bool(),
        Some(false)
    );
    assert_eq!(
        decision.actions[0].raw["target_order_kind"].as_str(),
        Some("any")
    );
    assert_eq!(decision.actions[1].symbol, "ETH-USDT-SWAP");
    assert_eq!(
        decision.actions[1].raw["_symbol_explicit"].as_bool(),
        Some(true)
    );
    assert_eq!(
        decision.actions[1].raw["client_order_id"].as_str(),
        Some("explicit-scope-client")
    );
    assert_eq!(decision.actions[1].raw["new_size"].as_str(), Some("2"));
    assert_eq!(
        decision.actions[1].raw["target_order_kind"].as_str(),
        Some("algo")
    );
    assert_eq!(
        decision.actions[1].raw["target_order_type"].as_str(),
        Some("stop_market")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_normalizes_order_type_hyphen_aliases_like_execution_layer() {
    let root = temp_project_root("runtime_decision_normalizes_order_type_hyphen_aliases");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    latest = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [
            {
                "action": "open_position",
                "symbol": symbol,
                "side": "long",
                "order_type": "post-only",
                "position_size": 0.1,
                "reason": "hyphen_order_type_requires_price",
                "timestamp": latest["timestamp"],
            },
            {
                "action": "modify_order",
                "symbol": symbol,
                "client_order_id": "target-algo-client",
                "new_price": "94.25",
                "target_order_kind": "algo",
                "target_order_type": "stop-market",
                "reason": "hyphen_target_order_type",
                "timestamp": latest["timestamp"],
            },
        ],
        "diagnostics": {},
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let decision = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles).unwrap();

    assert_eq!(decision.actions.len(), 2);
    assert_eq!(
        decision.actions[0].raw["order_type"].as_str(),
        Some("post_only")
    );
    assert_eq!(
        decision.actions[0].raw["_missing_required_price"].as_bool(),
        Some(true)
    );
    assert_eq!(
        decision.actions[0].raw["price_source"].as_str(),
        Some("missing_required")
    );
    assert_eq!(
        decision.actions[1].raw["target_order_type"].as_str(),
        Some("stop_market")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_non_string_order_management_target_kind() {
    let root = temp_project_root("runtime_decision_rejects_non_string_order_target_kind");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candle = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [{
            "action": "cancel_order",
            "symbol": symbol,
            "order_id": "target-order",
            "target_order_kind": 1,
            "reason": "invalid_target_kind_type",
            "timestamp": candle["timestamp"],
        }],
        "diagnostics": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.actions[].target_order_kind 必须是 JSON string"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_unknown_order_management_target_kind() {
    let root = temp_project_root("runtime_decision_rejects_unknown_order_target_kind");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candle = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [{
            "action": "cancel_order",
            "symbol": symbol,
            "order_id": "target-order",
            "target_order_kind": "paper",
            "reason": "invalid_target_kind_value",
            "timestamp": candle["timestamp"],
        }],
        "diagnostics": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.actions[].target_order_kind=paper 不受支持"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_unknown_order_management_target_type() {
    let root = temp_project_root("runtime_decision_rejects_unknown_order_target_type");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candle = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [{
            "action": "modify_order",
            "symbol": symbol,
            "client_order_id": "target-algo-order",
            "target_order_kind": "algo",
            "target_order_type": "take_profit_limit",
            "new_price": "101.5",
            "reason": "invalid_target_type_value",
            "timestamp": candle["timestamp"],
        }],
        "diagnostics": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("StrategyDecision.actions[].target_order_type=take_profit_limit 不受支持")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_action_alias_fields() {
    let root = temp_project_root("runtime_decision_rejects_action_alias_fields");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candle = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [
            {
                "intent_action": "exit",
                "symbol": symbol,
                "side": "short",
                "reason": "alias_exit_short",
                "timestamp": candle["timestamp"],
            },
            {
                "action": "open_position",
                "symbol": symbol,
                "side": "long",
                "price": candle["close"],
                "position_size": 0.21,
                "reason": "alias_entry_long",
                "timestamp": candle["timestamp"],
            },
            {
                "type": "risk_order",
                "symbol": symbol,
                "side": "sell",
                "order_type": "stop_market",
                "trigger_price": candle["close"] * 0.94,
                "reason": "alias_risk_order",
                "timestamp": candle["timestamp"],
            },
            {
                "type": "cancel",
                "symbol": symbol,
                "order_id": "cancel-target",
                "reason": "alias_cancel_order",
                "timestamp": candle["timestamp"],
            },
            {
                "intent_action": "amend",
                "symbol": symbol,
                "client_order_id": "modify-target",
                "new_px": "101.25",
                "new_sz": "2",
                "reason": "alias_modify_order",
                "timestamp": candle["timestamp"],
            },
        ],
        "diagnostics": {},
        "indicators": {},
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("intent_action 旧动作别名已删除"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_engine_controlled_action_fields() {
    let root = temp_project_root("runtime_decision_rejects_engine_controlled_action_fields");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candle = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "price": candle["close"],
            "position_size": 0.21,
            "leverage": 7,
            "reason": "engine_controlled_leverage",
            "timestamp": candle["timestamp"],
        }],
        "diagnostics": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("leverage 是交易引擎控制字段"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_exposes_strategy_execution_logs_and_events() {
    let root = temp_project_root("runtime_decision_exposes_strategy_execution_logs_and_events");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
import json
import sys

def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candles = context["candles"][symbol][timeframe]
    if context["runtime"].get("strategy_log_events"):
        sys.stdout.write(json.dumps({
            "event": "strategy_log",
            "stage": "strategy_input",
            "level": "warn",
            "message": "strategy emitted live log",
            "details": {"candle_count": len(candles)}
        }) + "\n")
        sys.stdout.flush()
    return {
        "actions": [],
        "diagnostics": {"summary": "no action"},
        "indicators": {},
        "execution_logs": [{
            "stage": "strategy_decision",
            "level": "success",
            "message": "strategy returned decision log",
            "details": {"symbol": symbol, "timeframe": timeframe}
        }],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);
    let mut events = Vec::new();

    let decision = compute_runtime_decision_with_context_and_events(
        &root,
        STRATEGY_FILE,
        &config,
        &candles,
        &json!({}),
        |event| events.push(event.clone()),
    )
    .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"].as_str(), Some("strategy_log"));
    assert_eq!(
        events[0]["message"].as_str(),
        Some("strategy emitted live log")
    );
    assert_eq!(decision.execution_logs.len(), 1);
    assert_eq!(decision.execution_logs[0].stage, "strategy_decision");
    assert_eq!(decision.execution_logs[0].level, "success");
    assert_eq!(
        decision.execution_logs[0].message,
        "strategy returned decision log"
    );
    assert_eq!(
        decision.execution_logs[0].details["symbol"].as_str(),
        Some("BTC-USDT-SWAP")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_execution_log_alias_fields() {
    let root = temp_project_root("runtime_decision_rejects_execution_log_alias_fields");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    return {
        "actions": [],
        "diagnostics": {},
        "indicators": {},
        "execution_logs": [{
            "phase": "strategy_decision",
            "severity": "info",
            "summary": "old log shorthand",
            "data": {"source": "legacy"}
        }],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.execution_logs[0].summary 字段别名已删除"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_non_string_execution_log_fields() {
    let root = temp_project_root("runtime_decision_rejects_non_string_execution_log_fields");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    return {
        "actions": [],
        "diagnostics": {},
        "indicators": {},
        "execution_logs": [{
            "stage": "strategy_decision",
            "level": "info",
            "message": 42,
            "details": {}
        }],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.execution_logs[0].message 必须是非空字符串"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_requires_execution_log_details() {
    let root = temp_project_root("runtime_decision_requires_execution_log_details");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    return {
        "actions": [],
        "diagnostics": {},
        "indicators": {},
        "execution_logs": [{
            "stage": "strategy_decision",
            "level": "info",
            "message": "missing details should fail"
        }],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.execution_logs[0].details 必须显式返回 dict"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_action_size_alias_fields() {
    let root = temp_project_root("runtime_decision_rejects_action_size_alias_fields");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candle = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [
            {
                "action": "open_position",
                "symbol": symbol,
                "side": "long",
                "price": candle["close"],
                "quantity": 0.44,
                "reason": "quantity_alias_open",
                "timestamp": candle["timestamp"],
            },
            {
                "action": "close_position",
                "symbol": symbol,
                "side": "flat",
                "sz": 0.12,
                "reason": "sz_alias_close",
                "timestamp": candle["timestamp"],
            },
        ],
        "diagnostics": {},
        "indicators": {},
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.actions[].quantity 字段别名已删除"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_planned_exit_alias_fields() {
    let root = temp_project_root("runtime_decision_rejects_planned_exit_alias_fields");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candle = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "price": candle["close"],
            "position_size": 0.2,
            "timestamp": candle["timestamp"],
            "exit_time": candle["timestamp"] + 900000,
            "exit_reason": "max_hold_bars",
            "action_contract_version": "old_planned_exit_contract",
        }],
        "diagnostics": {},
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.actions[].exit_time 字段别名已删除"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_legacy_order_lists_as_actions_protocol_error() {
    let root = temp_project_root("runtime_decision_rejects_legacy_order_lists");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    btc_1m = context["candles"]["BTC-USDT-SWAP"]["1m"]
    eth_15m = context["candles"]["ETH-USDT-SWAP"]["15m"]
    assert context["positions"]["ETH-USDT-SWAP"]["side"] == "short"
    assert "open" in context["orders"]
    return {
        "orders": [{
            "symbol": "ETH-USDT-SWAP",
            "side": "buy",
            "order_type": "market",
            "position_size": 0.12,
            "reason": f"multi_context:{len(btc_1m)}:{len(eth_15m)}",
            "timestamp": eth_15m[-1]["timestamp"],
        }],
        "risk_orders": [{
            "symbol": "ETH-USDT-SWAP",
            "side": "sell",
            "order_type": "stop_market",
            "stop_loss_bps": 600,
            "reason": "protective_stop",
            "timestamp": eth_15m[-1]["timestamp"],
        }],
        "diagnostics": {"btc_1m": len(btc_1m), "eth_15m": len(eth_15m)},
        "indicators": {"eth_close": [item["close"] for item in eth_15m]},
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);
    let context = json!({
        "candles": {
            "BTC-USDT-SWAP": {
                "1m": rising_candles(1_780_290_000_000_i64, 1, 10)
            },
            "ETH-USDT-SWAP": {
                "15m": candles
            }
        },
        "positions": {
            "ETH-USDT-SWAP": {"side": "short", "entry_price": 2000.0, "quantity": 1.5}
        },
        "account": {"equity": 1000.0},
        "orders": {"open": [], "recent_fills": [], "recent_rejections": []},
        "time": {"timestamp": 1_780_290_000_000_i64 + 45 * 60_000, "timeframe": "15m"},
        "runtime": {
            "strategy_id": "runtime_contract_fixture",
            "symbol": "ETH-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "15m"
        }
    });
    let primary = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error =
        compute_runtime_decision_with_context(&root, STRATEGY_FILE, &config, &primary, &context)
            .unwrap_err()
            .to_string();

    assert!(error.contains("orders/risk_orders 旧合约已删除"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_removed_signal_and_portfolio_fields() {
    let root = temp_project_root("runtime_decision_rejects_removed_signal_and_portfolio_fields");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candle = context["candles"][symbol][timeframe][-1]
    return {
        "signals": [{
            "action": "buy",
            "symbol": symbol,
            "side": "long",
            "price": candle["close"],
            "timestamp": candle["timestamp"],
        }],
        "portfolio_layers": [{"symbol": symbol, "weight": 1.0}],
        "diagnostics": {},
        "indicators": {},
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("signals/portfolio_layers 旧合约已删除"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_requires_explicit_actions_field() {
    let root = temp_project_root("runtime_decision_requires_explicit_actions_field");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    return {
        "diagnostics": {"summary": "missing actions should fail"},
        "indicators": {},
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.actions 是必填 list"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_requires_explicit_diagnostics_field() {
    let root = temp_project_root("runtime_decision_requires_explicit_diagnostics_field");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    return {
        "actions": [],
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.diagnostics 是必填 dict"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_requires_explicit_indicators_field() {
    let root = temp_project_root("runtime_decision_requires_explicit_indicators_field");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    return {
        "actions": [],
        "diagnostics": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.indicators 是必填 dict"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_requires_explicit_execution_logs_field() {
    let root = temp_project_root("runtime_decision_requires_explicit_execution_logs_field");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    return {
        "actions": [],
        "diagnostics": {},
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.execution_logs 是必填 list"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_persistent_session_matches_single_call_decision() {
    let root = temp_project_root("runtime_persistent_session_matches_single_call_decision");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candles = context["candles"][symbol][timeframe]
    latest = candles[-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "order_type": "market",
            "price": latest["close"],
            "position_size": 0.2,
            "reason": f"session:{len(candles)}:{context['time']['timestamp']}",
            "timestamp": latest["timestamp"],
        }],
        "diagnostics": {
            "seen": len(candles),
            "timestamp": context["time"]["timestamp"],
        },
        "indicators": {"seen_bars": [len(candles)]},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);
    let context = json!({
        "candles": {
            "BTC-USDT-SWAP": {
                "15m": candles
            }
        },
        "orders": {"open": [], "recent_fills": [], "recent_rejections": []},
        "runtime": {
            "strategy_id": "runtime_contract_fixture",
            "symbol": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "15m"
        },
        "time": {"timestamp": 1_780_290_000_000_i64 + 45 * 60_000, "timeframe": "15m"}
    });
    let expected =
        compute_runtime_decision_with_context(&root, STRATEGY_FILE, &config, &candles, &context)
            .unwrap();

    let mut session = PythonRunnerSession::new(&root).unwrap();
    let actual = session
        .compute_runtime_decision_with_context_and_events(
            STRATEGY_FILE,
            &config,
            &candles,
            &context,
            |_| {},
        )
        .unwrap();

    assert_eq!(actual.actions.len(), expected.actions.len());
    assert_eq!(actual.actions[0].reason, expected.actions[0].reason);
    assert_eq!(actual.diagnostics["seen"], expected.diagnostics["seen"]);
    assert_eq!(actual.indicators, expected.indicators);

    let shorter = rising_candles(1_780_290_000_000_i64, 15, 2);
    let shorter_context = json!({
        "candles": {
            "BTC-USDT-SWAP": {
                "15m": shorter
            }
        },
        "runtime": {
            "strategy_id": "runtime_contract_fixture",
            "symbol": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "15m"
        },
        "time": {"timestamp": 1_780_290_000_000_i64 + 15 * 60_000, "timeframe": "15m"}
    });
    let second = session
        .compute_runtime_decision_with_context_and_events(
            STRATEGY_FILE,
            &config,
            &shorter,
            &shorter_context,
            |_| {},
        )
        .unwrap();
    assert_eq!(second.diagnostics["seen"].as_i64(), Some(2));
    assert!(second.actions[0].reason.starts_with("session:2:"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_persistent_session_context_ref_matches_point_in_time_context() {
    let root =
        temp_project_root("runtime_persistent_session_context_ref_matches_point_in_time_context");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    timestamp = int(context["time"]["timestamp"])
    candles = context["candles"][symbol][timeframe]
    funding = context.get("funding", {}).get(symbol, {})
    funding_history = funding.get("history", [])
    latest_funding = funding.get("latest", {})
    return {
        "actions": [],
        "diagnostics": {
            "seen": len(candles),
            "last_close": candles[-1]["close"],
            "future_seen": any(int(row.get("timestamp", 0) or 0) > timestamp for row in candles),
            "funding_history": len(funding_history),
            "funding_latest_time": latest_funding.get("funding_time"),
            "equity": context.get("account", {}).get("equity"),
            "open_orders": len(context.get("orders", {}).get("open", [])),
        },
        "indicators": {"timestamps": [row["timestamp"] for row in candles]},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);
    let current = vec![candles[1].clone()];
    let first_funding_time = candles[0]["timestamp"].as_i64().unwrap();
    let future_funding_time = candles[2]["timestamp"].as_i64().unwrap();
    let point_context = json!({
        "candles": {
            "BTC-USDT-SWAP": {
                "15m": candles[..2].to_vec()
            }
        },
        "funding": {
            "BTC-USDT-SWAP": {
                "source": "fixture",
                "history": [{
                    "funding_time": first_funding_time,
                    "funding_rate": 0.001
                }],
                "latest": {
                    "funding_time": first_funding_time,
                    "funding_rate": 0.001
                }
            }
        },
        "account": {"equity": 123.45},
        "orders": {"open": [{"id": "open-1"}], "recent_fills": [], "recent_rejections": []},
        "time": {"timestamp": candles[1]["timestamp"].as_i64().unwrap(), "timeframe": "15m"}
    });
    let full_cached_context = json!({
        "candles": {
            "BTC-USDT-SWAP": {
                "15m": candles
            }
        },
        "funding": {
            "BTC-USDT-SWAP": {
                "source": "fixture",
                "_history_limit": 20,
                "history": [
                    {
                        "funding_time": first_funding_time,
                        "funding_rate": 0.001
                    },
                    {
                        "funding_time": future_funding_time,
                        "funding_rate": 0.002
                    }
                ],
                "latest": {
                    "funding_time": future_funding_time,
                    "funding_rate": 0.002
                }
            }
        }
    });

    let expected = compute_runtime_decision_with_context(
        &root,
        STRATEGY_FILE,
        &config,
        &current,
        &point_context,
    )
    .unwrap();

    let mut session = PythonRunnerSession::new(&root).unwrap();
    let cache_result = session
        .cache_runtime_context("point-in-time-context", &full_cached_context)
        .unwrap();
    assert_eq!(cache_result["ok"].as_bool(), Some(true));
    let overlay = json!({
        "account": {"equity": 123.45},
        "orders": {"open": [{"id": "open-1"}], "recent_fills": [], "recent_rejections": []},
    });
    let actual = session
        .compute_runtime_decision_with_context_ref_and_events(
            STRATEGY_FILE,
            &config,
            &current,
            "point-in-time-context",
            &overlay,
            |_| {},
        )
        .unwrap();

    assert_eq!(actual.diagnostics, expected.diagnostics);
    assert_eq!(actual.indicators, expected.indicators);
    assert_eq!(actual.diagnostics["seen"].as_i64(), Some(2));
    assert_eq!(actual.diagnostics["future_seen"].as_bool(), Some(false));
    assert_eq!(
        actual.diagnostics["funding_latest_time"].as_i64(),
        Some(first_funding_time)
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_evaluate_history_uses_context_ref_as_historical_context() {
    let root = temp_project_root("runtime_evaluate_history_uses_context_ref_as_historical_context");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    candle = context["candles"][symbol][context["runtime"]["timeframe"]][-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "reason": "fallback_evaluate",
            "timestamp": candle["timestamp"],
        }],
        "diagnostics": {"path": "evaluate"},
        "indicators": {},
        "execution_logs": [],
    }

def evaluate_history(context, params, candles):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    context_candles = context["candles"][symbol][timeframe]
    latest = candles[-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "reason": f"history_once:{len(candles)}:{len(context_candles)}",
            "timestamp": latest["timestamp"],
        }],
        "diagnostics": {
            "path": "evaluate_history",
            "candles_arg": len(candles),
            "context_candles": len(context_candles),
        },
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let mut config = runtime_config();
    config["compute_scope"] = json!("history");
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);
    let full_cached_context = json!({
        "candles": {
            "BTC-USDT-SWAP": {
                "15m": candles.clone()
            }
        }
    });

    let mut session = PythonRunnerSession::new(&root).unwrap();
    session
        .cache_runtime_context("history-context", &full_cached_context)
        .unwrap();
    let decision = session
        .compute_runtime_decision_with_context_ref_and_events(
            STRATEGY_FILE,
            &config,
            &candles,
            "history-context",
            &json!({}),
            |_| {},
        )
        .unwrap();

    assert_eq!(decision.actions.len(), 1);
    assert_eq!(decision.actions[0].reason, "history_once:4:4");
    assert_eq!(
        decision.diagnostics["path"].as_str(),
        Some("evaluate_history")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_persistent_session_exposes_strategy_log_events() {
    let root = temp_project_root("runtime_persistent_session_exposes_strategy_log_events");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
import json
import sys

def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candles = context["candles"][symbol][timeframe]
    if context["runtime"].get("strategy_log_events"):
        sys.stdout.write(json.dumps({
            "event": "strategy_log",
            "stage": "session_strategy",
            "level": "info",
            "message": f"persistent session saw {len(candles)} candles",
            "details": {"candle_count": len(candles)}
        }) + "\n")
        sys.stdout.flush()
    return {
        "actions": [],
        "diagnostics": {"seen": len(candles)},
        "indicators": {},
        "execution_logs": [{
            "stage": "strategy_decision",
            "level": "success",
            "message": "persistent session returned decision",
            "details": {"symbol": symbol}
        }],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);
    let context = json!({
        "candles": {
            "BTC-USDT-SWAP": {
                "15m": candles
            }
        },
        "runtime": {
            "strategy_id": "runtime_contract_fixture",
            "symbol": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "15m"
        },
        "time": {"timestamp": 1_780_290_000_000_i64 + 45 * 60_000, "timeframe": "15m"}
    });

    let mut session = PythonRunnerSession::new(&root).unwrap();
    let mut events = Vec::new();
    let decision = session
        .compute_runtime_decision_with_context_and_events(
            STRATEGY_FILE,
            &config,
            &candles,
            &context,
            |event| events.push(event.clone()),
        )
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"].as_str(), Some("strategy_log"));
    assert_eq!(events[0]["stage"].as_str(), Some("session_strategy"));
    assert_eq!(
        events[0]["message"].as_str(),
        Some("persistent session saw 4 candles")
    );
    assert_eq!(decision.diagnostics["seen"].as_i64(), Some(4));
    assert_eq!(decision.execution_logs.len(), 1);
    assert_eq!(
        decision.execution_logs[0].message,
        "persistent session returned decision"
    );

    let next = session
        .compute_runtime_decision_with_context_and_events(
            STRATEGY_FILE,
            &config,
            &candles,
            &context,
            |_| {},
        )
        .unwrap();
    assert_eq!(next.diagnostics["seen"].as_i64(), Some(4));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_normalizes_partial_order_context() {
    let root = temp_project_root("runtime_decision_normalizes_partial_order_context");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    orders = context["orders"]
    assert isinstance(orders["open"], list)
    assert isinstance(orders["recent_fills"], list)
    assert isinstance(orders["recent_rejections"], list)
    symbol = context["runtime"]["symbol"]
    candle = context["candles"][symbol][context["runtime"]["timeframe"]][-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "order_type": "market",
            "position_size": 0.1,
            "reason": f"orders_shape:{len(orders['open'])}:{len(orders['recent_fills'])}:{len(orders['recent_rejections'])}",
            "timestamp": candle["timestamp"],
        }],
        "diagnostics": {
            "open": len(orders["open"]),
            "recent_fills": len(orders["recent_fills"]),
            "recent_rejections": len(orders["recent_rejections"]),
            "total_orders": orders.get("total_orders"),
        },
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);
    let context = json!({
        "candles": {
            "BTC-USDT-SWAP": {
                "15m": candles
            }
        },
        "orders": {
            "open": [{"id": "risk-1"}],
            "recent_fills": "bad-shape",
            "total_orders": 3
        },
        "runtime": {
            "strategy_id": "runtime_contract_fixture",
            "symbol": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "15m"
        }
    });
    let primary = rising_candles(1_780_290_000_000_i64, 15, 4);

    let decision =
        compute_runtime_decision_with_context(&root, STRATEGY_FILE, &config, &primary, &context)
            .unwrap();

    assert_eq!(decision.actions.len(), 1);
    assert_eq!(decision.actions[0].reason, "orders_shape:1:0:0");
    assert_eq!(decision.diagnostics["open"].as_i64(), Some(1));
    assert_eq!(decision.diagnostics["recent_fills"].as_i64(), Some(0));
    assert_eq!(decision.diagnostics["recent_rejections"].as_i64(), Some(0));
    assert_eq!(decision.diagnostics["total_orders"].as_i64(), Some(3));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_history_evaluate_returns_all_action_rows() {
    let root = temp_project_root("runtime_history_evaluate_returns_all_action_rows");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candles = context["candles"][symbol][timeframe]
    latest = candles[-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "order_type": "market",
            "price": latest["close"],
            "position_size": 0.1,
            "reason": f"history_action:{len(candles)}",
            "timestamp": latest["timestamp"],
        }],
        "diagnostics": {"seen": len(candles)},
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let mut config = runtime_config();
    config["compute_scope"] = json!("history");
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let decision = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles).unwrap();

    assert_eq!(decision.actions.len(), 4);
    assert_eq!(decision.actions[0].reason, "history_action:1");
    assert_eq!(decision.actions[3].reason, "history_action:4");
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_evaluate_marks_explicit_invalid_action_price_without_close_fallback() {
    let root = temp_project_root(
        "runtime_evaluate_marks_explicit_invalid_action_price_without_close_fallback",
    );
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    latest = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "order_type": "market",
            "price": "bad-price",
            "position_size": 0.1,
            "reason": "bad_price_must_not_use_close",
            "timestamp": latest["timestamp"],
        }],
        "diagnostics": {},
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let decision = compute_runtime_decision(&root, STRATEGY_FILE, &runtime_config(), &candles)
        .expect("runtime strategy should compute");

    assert_eq!(decision.actions.len(), 1);
    assert_eq!(decision.actions[0].price, None);
    assert_eq!(decision.actions[0].reference_price, None);
    assert_eq!(
        decision.actions[0].raw["_invalid_price"].as_bool(),
        Some(true)
    );
    assert_eq!(
        decision.actions[0].raw["price_source"].as_str(),
        Some("invalid_explicit")
    );
    assert!(decision.actions[0].raw.get("_price_source").is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_evaluate_rejects_string_numeric_action_position_size() {
    let root = temp_project_root("runtime_evaluate_rejects_string_numeric_action_position_size");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    latest = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "order_type": "market",
            "price": latest["close"],
            "position_size": "0.1",
            "reason": "string_position_size_must_fail",
            "timestamp": latest["timestamp"],
        }],
        "diagnostics": {},
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &runtime_config(), &candles)
        .expect_err("string numeric action position_size must be rejected");

    assert!(error
        .to_string()
        .contains("position_size 必须是 JSON number"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_evaluate_does_not_fill_limit_action_price_from_latest_close() {
    let root =
        temp_project_root("runtime_evaluate_does_not_fill_limit_action_price_from_latest_close");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    latest = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "order_type": "limit",
            "position_size": 0.1,
            "reason": "limit_price_must_be_explicit",
            "timestamp": latest["timestamp"],
        }],
        "diagnostics": {},
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let decision = compute_runtime_decision(&root, STRATEGY_FILE, &runtime_config(), &candles)
        .expect("runtime strategy should compute");

    assert_eq!(decision.actions.len(), 1);
    assert_eq!(decision.actions[0].price, None);
    assert_eq!(decision.actions[0].reference_price, None);
    assert_eq!(
        decision.actions[0].raw["_missing_required_price"].as_bool(),
        Some(true)
    );
    assert_eq!(
        decision.actions[0].raw["price_source"].as_str(),
        Some("missing_required")
    );
    assert!(decision.actions[0].raw.get("_price_source").is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_evaluate_market_action_without_price_does_not_infer_reference_price() {
    let root = temp_project_root(
        "runtime_evaluate_market_action_without_price_does_not_infer_reference_price",
    );
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    latest = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "order_type": "market",
            "position_size": 0.1,
            "reason": "market_reference_price_only",
            "timestamp": latest["timestamp"],
        }],
        "diagnostics": {},
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let decision = compute_runtime_decision(&root, STRATEGY_FILE, &runtime_config(), &candles)
        .expect("runtime strategy should compute");

    assert_eq!(decision.actions.len(), 1);
    assert_eq!(decision.actions[0].price, None);
    assert_eq!(decision.actions[0].reference_price, None);
    assert!(decision.actions[0].raw.get("price").is_none());
    assert!(decision.actions[0].raw.get("reference_price").is_none());
    assert!(decision.actions[0].raw.get("price_source").is_none());
    assert!(decision.actions[0].raw.get("_price_source").is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_exchange_size_alias_fields() {
    let root = temp_project_root("runtime_decision_rejects_exchange_size_alias_fields");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candle = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [
            {
                "action": "open_position",
                "symbol": symbol,
                "side": "long",
                "price": candle["close"],
                "okx_sz": "2.03",
                "reason": "okx_size_alias_open",
                "timestamp": candle["timestamp"],
            },
            {
                "action": "close_position",
                "symbol": symbol,
                "side": "flat",
                "order_sz": 1.25,
                "reason": "order_size_alias_close",
                "timestamp": candle["timestamp"],
            },
        ],
        "diagnostics": {},
        "indicators": {},
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.actions[].okx_sz 字段别名已删除"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_non_string_action_text_fields() {
    let root = temp_project_root("runtime_decision_rejects_non_string_action_text_fields");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candle = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [{
            "action": "cancel_order",
            "symbol": symbol,
            "order_id": 12345,
            "reason": "numeric_order_id_must_fail",
            "timestamp": candle["timestamp"],
        }],
        "diagnostics": {},
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.actions[].order_id 必须是 JSON string"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_decision_rejects_non_string_order_type_before_market_default() {
    let root = temp_project_root("runtime_decision_rejects_non_string_order_type");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candle = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "order_type": 123,
            "price": candle["close"],
            "position_size": 0.1,
            "reason": "numeric_order_type_must_fail",
            "timestamp": candle["timestamp"],
        }],
        "diagnostics": {},
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &config, &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("StrategyDecision.actions[].order_type 必须是 JSON string"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_evaluate_history_preserves_action_extension_fields() {
    let root = temp_project_root("runtime_evaluate_history_preserves_action_extension_fields");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    return {"actions": [], "diagnostics": {}, "indicators": {}, "execution_logs": []}

def evaluate_history(context, params, candles):
    symbol = context["runtime"]["symbol"]
    latest = candles[-1]
    planned_exit_time = latest["timestamp"] + 7200000
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "order_type": "market",
            "price": latest["close"],
            "position_size": 0.2,
            "reason": "history_planned_exit_contract",
            "timestamp": latest["timestamp"],
            "planned_exit_time": planned_exit_time,
            "planned_exit_reason": "max_hold_bars",
            "planned_hold_bars": 8,
            "hold_bars": 8,
            "entry_time": latest["timestamp"],
            "layer_id": "evaluate_history_layer",
            "candidate_source": "evaluate_history_source",
        }],
        "diagnostics": {
            "history_action_contract": {
                "status": "planned_exit_complete",
                "open_action_count": 1,
                "open_actions_with_planned_exit": 1,
            }
        },
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let mut config = runtime_config();
    config["compute_scope"] = json!("history");
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);
    let context = json!({
        "candles": {
            "BTC-USDT-SWAP": {
                "15m": candles.clone()
            }
        }
    });

    let decision =
        compute_runtime_decision_with_context(&root, STRATEGY_FILE, &config, &candles, &context)
            .unwrap();

    assert_eq!(decision.actions.len(), 1);
    assert_eq!(
        decision.actions[0].raw["planned_exit_time"].as_i64(),
        Some(1_780_290_000_000_i64 + 3 * 15 * 60_000 + 7_200_000)
    );
    assert_eq!(
        decision.actions[0].raw["planned_hold_bars"].as_i64(),
        Some(8)
    );
    assert_eq!(
        decision.actions[0].raw["layer_id"].as_str(),
        Some("evaluate_history_layer")
    );
    assert_eq!(
        decision.diagnostics["history_action_contract"]["status"].as_str(),
        Some("planned_exit_complete")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_history_evaluate_exposes_backtest_progress_context() {
    let root = temp_project_root("runtime_history_evaluate_exposes_backtest_progress_context");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candles = context["candles"][symbol][timeframe]
    progress = context.get("backtest", {})
    latest = candles[-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "order_type": "market",
            "price": latest["close"],
            "reason": "progress_context_action",
            "timestamp": latest["timestamp"],
        }],
        "diagnostics": {
            "seen": len(candles),
            "backtest_progress": progress,
        },
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let mut config = runtime_config();
    config["compute_scope"] = json!("history");
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);
    let mut events = Vec::new();

    let decision = compute_runtime_decision_with_context_and_progress(
        &root,
        STRATEGY_FILE,
        &config,
        &candles,
        &json!({}),
        |event| events.push(event.clone()),
    )
    .unwrap();

    assert_eq!(decision.actions.len(), 4);
    assert_eq!(decision.diagnostics["seen"].as_i64(), Some(4));
    assert_eq!(
        decision.diagnostics["backtest_progress"]["processed_candles"].as_i64(),
        Some(4)
    );
    assert_eq!(
        decision.diagnostics["backtest_progress"]["total_candles"].as_i64(),
        Some(4)
    );
    assert!(!events.is_empty());
    assert_eq!(events.last().unwrap()["processed"].as_i64(), Some(4));
    assert_eq!(events.last().unwrap()["total"].as_i64(), Some(4));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_compute_requires_evaluate_actions_protocol() {
    let root = temp_project_root("runtime_compute_requires_evaluate_actions_protocol");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def legacy_only():
    return None
"#,
        ),
    );
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let error = compute_runtime_decision(&root, STRATEGY_FILE, &runtime_config(), &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("未找到 evaluate 函数"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_history_context_trims_funding_at_each_bar() {
    let root = temp_project_root("runtime_history_context_trims_funding_at_each_bar");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candles = context["candles"][symbol][timeframe]
    latest = candles[-1]
    funding = context["funding"][symbol]["history"]
    latest_rate = funding[-1]["funding_rate"] if funding else 0.0
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "order_type": "market",
            "price": latest["close"],
            "position_size": 0.1,
            "reason": f"funding_seen:{len(funding)}:{latest_rate:.4f}",
            "timestamp": latest["timestamp"],
        }],
        "diagnostics": {},
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let mut config = runtime_config();
    config["compute_scope"] = json!("history");
    let candles = rising_candles(1_780_290_000_000_i64, 15, 3);
    let context = json!({
        "candles": {
            "BTC-USDT-SWAP": {"15m": candles}
        },
        "funding": {
            "BTC-USDT-SWAP": {
                "source": "okx_funding_rates",
                "latest": {"funding_time": candles[2]["timestamp"], "funding_rate": 0.0003},
                "history": [
                    {"funding_time": candles[0]["timestamp"], "funding_rate": 0.0001},
                    {"funding_time": candles[1]["timestamp"], "funding_rate": 0.0002},
                    {"funding_time": candles[2]["timestamp"], "funding_rate": 0.0003}
                ]
            }
        },
        "orders": {"open": [], "recent_fills": [], "recent_rejections": []},
        "positions": {},
        "account": {},
        "time": {"timestamp": candles[2]["timestamp"], "timeframe": "15m"},
        "runtime": {
            "strategy_id": "runtime_contract_fixture",
            "symbol": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "15m"
        }
    });

    let decision =
        compute_runtime_decision_with_context(&root, STRATEGY_FILE, &config, &candles, &context)
            .unwrap();

    assert_eq!(decision.actions.len(), 3);
    assert_eq!(decision.actions[0].reason, "funding_seen:1:0.0001");
    assert_eq!(decision.actions[1].reason, "funding_seen:2:0.0002");
    assert_eq!(decision.actions[2].reason, "funding_seen:3:0.0003");
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_diagnostics_use_evaluate_decision_protocol() {
    let root = runtime_fixture_root("runtime_diagnostics_use_evaluate_decision_protocol");
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 25);

    let diagnostics = compute_runtime_diagnostics(&root, STRATEGY_FILE, &config, &candles).unwrap();

    assert!(diagnostics.get("supported").is_none());
    assert_eq!(
        diagnostics["summary"].as_str(),
        Some("fixture evaluate diagnostics")
    );
    assert_eq!(
        diagnostics["decision_protocol"].as_str(),
        Some("actions_v1")
    );
    assert_eq!(
        diagnostics["action_summary"]["open_position"].as_i64(),
        Some(1)
    );
    assert_eq!(
        diagnostics["actions"][0]["reason"].as_str(),
        Some("15m:BTC")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_diagnostics_overwrite_diagnostics_protocol_fields() {
    let root = temp_project_root("runtime_diagnostics_overwrite_diagnostics_protocol_fields");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = context["runtime"]["symbol"]
    timeframe = context["runtime"]["timeframe"]
    candle = context["candles"][symbol][timeframe][-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": symbol,
            "side": "long",
            "order_type": "market",
            "price": candle["close"],
            "position_size": 0.25,
            "reason": "canonical_action",
            "timestamp": candle["timestamp"],
        }],
        "diagnostics": {
            "actions": [{"action": "hold", "reason": "stale_diagnostics_action"}],
            "action_summary": {"hold": 1, "total": 1},
            "execution_logs": [{"message": "stale diagnostics log"}],
            "selected_symbols": ["STALE-USDT-SWAP"],
            "decision_protocol": "legacy_protocol",
            "decision": {"protocol": "legacy_protocol"},
        },
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    let config = runtime_config();
    let candles = rising_candles(1_780_290_000_000_i64, 15, 4);

    let diagnostics = compute_runtime_diagnostics(&root, STRATEGY_FILE, &config, &candles).unwrap();

    assert_eq!(
        diagnostics["decision_protocol"].as_str(),
        Some("actions_v1")
    );
    assert_eq!(
        diagnostics["summary"].as_str(),
        Some("策略返回动作：开仓 1")
    );
    assert!(diagnostics.get("timestamp").is_none());
    assert!(diagnostics.get("price").is_none());
    assert!(diagnostics.get("supported").is_none());
    assert_eq!(
        diagnostics["actions"][0]["action"].as_str(),
        Some("open_position")
    );
    assert_eq!(
        diagnostics["actions"][0]["reason"].as_str(),
        Some("canonical_action")
    );
    assert_eq!(
        diagnostics["action_summary"]["open_position"].as_i64(),
        Some(1)
    );
    assert_eq!(
        diagnostics["selected_symbols"][0].as_str(),
        Some("BTC-USDT-SWAP")
    );
    assert_eq!(
        diagnostics["execution_logs"].as_array().map(Vec::len),
        Some(0)
    );
    assert_eq!(
        diagnostics["decision"]["protocol"].as_str(),
        Some("actions_v1")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn runtime_diagnostics_require_evaluate_actions_protocol() {
    let root = temp_project_root("runtime_diagnostics_require_evaluate_actions_protocol");
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def legacy_only():
    return None
"#,
        ),
    );

    let candles = rising_candles(1_780_290_000_000_i64, 15, 3);
    let error = compute_runtime_diagnostics(&root, STRATEGY_FILE, &runtime_config(), &candles)
        .unwrap_err()
        .to_string();

    assert!(error.contains("未找到 evaluate 函数"));
    fs::remove_dir_all(root).ok();
}

fn runtime_fixture_root(name: &str) -> std::path::PathBuf {
    let root = temp_project_root(name);
    prepare_python_runner(&root);
    write_runtime_strategy(
        &root,
        STRATEGY_FILE,
        &runtime_strategy_source(
            "runtime_contract_fixture",
            "Runtime Contract Fixture",
            r#"
def evaluate(context, params):
    symbol = str(params.get("_runtime_symbol") or "").split("-")[0]
    timeframe = str(params.get("_runtime_timeframe") or "")
    candle = context["candles"][context["runtime"]["symbol"]][context["runtime"]["timeframe"]][-1]
    return {
        "actions": [{
            "action": "open_position",
            "symbol": context["runtime"]["symbol"],
            "side": "long",
            "order_type": "market",
            "price": candle["close"],
            "reason": f"{timeframe}:{symbol}",
            "strength": 1.0,
            "timestamp": candle["timestamp"],
        }],
        "diagnostics": {
            "summary": "fixture evaluate diagnostics",
        },
        "indicators": {},
        "execution_logs": [],
    }
"#,
        ),
    );
    root
}

fn runtime_config() -> serde_json::Value {
    json!({
        "strategy_id": "runtime_contract_fixture",
        "symbol": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "15m",
        "params": {}
    })
}
