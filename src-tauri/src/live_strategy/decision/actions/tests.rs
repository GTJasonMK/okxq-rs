use serde_json::json;

use crate::{
    okx::OkxCandle, strategy_engine::StrategyConfig,
    strategy_executor::types::RuntimeStrategyAction,
};

use super::*;

#[test]
fn runtime_action_with_invalid_explicit_price_marker_is_skipped() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "BTC-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "_invalid_price": true,
        "position_size": 0.1,
        "reason": "bad_price_must_not_use_close",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, _risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert_eq!(intents.len(), 0);
    assert_eq!(skipped_actions.len(), 1);
    assert_eq!(
        skipped_actions[0]["reason"].as_str(),
        Some("bad_price_must_not_use_close")
    );
}

#[test]
fn runtime_action_with_string_position_size_is_skipped_instead_of_using_config_size() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "BTC-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": "0.1",
        "reason": "string_position_size",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, _risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert!(intents.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .unwrap_or_default()
        .contains("position_size 必须是 JSON number"));
}

#[test]
fn hold_action_is_idle_not_executable_or_skipped() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "hold",
        "symbol": "BTC-USDT-SWAP",
        "side": "hold",
        "reason": "no_trade",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert!(skipped_actions.is_empty());
    assert_eq!(idle_actions.len(), 1);
    assert_eq!(idle_actions[0]["action"].as_str(), Some("hold"));
    assert_eq!(idle_actions[0]["reason"].as_str(), Some("no_trade"));
}

#[test]
fn runtime_risk_order_attaches_to_matching_open_intent() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let open = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.1,
        "timestamp": 1_700_000_000_000_i64
    }));
    let risk = RuntimeStrategyAction::from_value(&json!({
        "action": "place_risk_order",
        "symbol": "ETH-USDT-SWAP",
        "side": "sell",
        "order_type": "stop_market",
        "trigger_price": 94.0,
        "stop_loss_bps": 600,
        "reason": "protective_stop",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[open, risk], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert_eq!(risk_orders.len(), 1);
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[0].symbol, "ETH-USDT-SWAP");
    assert_eq!(intents[0].stop_loss, Some(0.06));
    assert_eq!(intents[0].attached_risk_orders.len(), 1);
    assert_eq!(intents[0].attached_risk_orders[0].side, "sell");
    assert_eq!(intents[0].attached_risk_orders[0].trigger_price, Some(94.0));
}

#[test]
fn runtime_risk_order_normalizes_order_type_alias_before_attach() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let open = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.1,
        "timestamp": 1_700_000_000_000_i64
    }));
    let risk = RuntimeStrategyAction::from_value(&json!({
        "action": "place_risk_order",
        "symbol": "ETH-USDT-SWAP",
        "side": "sell",
        "order_type": "stop-market",
        "trigger_price": 94.0,
        "stop_loss_bps": 600,
        "reason": "protective_stop_alias",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[open, risk], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert_eq!(risk_orders.len(), 1);
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[0].attached_risk_orders.len(), 1);
    assert_eq!(intents[0].attached_risk_orders[0].order_type, "stop_market");
}

#[test]
fn runtime_standalone_risk_order_becomes_executable_intent() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let risk = RuntimeStrategyAction::from_value(&json!({
        "action": "place_risk_order",
        "symbol": "ETH-USDT-SWAP",
        "side": "sell",
        "order_type": "stop_market",
        "trigger_price": 94.0,
        "reference_price": 100.0,
        "stop_loss_bps": 600,
        "reason": "protect_existing_long",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[risk], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert_eq!(risk_orders.len(), 1);
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[0].action, StrategyIntentAction::PlaceRiskOrder);
    assert_eq!(intents[0].action_record.action, "place_risk_order");
    assert_eq!(intents[0].action_record.side, "sell");
    assert_eq!(intents[0].order_side.as_deref(), Some("sell"));
    assert_eq!(intents[0].attached_risk_orders.len(), 1);
    assert_eq!(intents[0].attached_risk_orders[0].trigger_price, Some(94.0));
}

#[test]
fn execution_plan_blocks_entire_batch_when_any_action_is_skipped() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let open = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.1,
        "timestamp": 1_700_000_000_000_i64
    }));
    let risk = RuntimeStrategyAction::from_value(&json!({
        "action": "place_risk_order",
        "symbol": "ETH-USDT-SWAP",
        "side": "sell",
        "order_type": "stop_market",
        "trigger_price": 94.0,
        "stop_loss_bps": 600,
        "reason": "protective_stop",
        "timestamp": 1_700_000_000_000_i64
    }));
    let invalid = RuntimeStrategyAction::from_value(&json!({
        "action": "modify_order",
        "symbol": "ETH-USDT-SWAP",
        "side": "hold",
        "order_id": "order-1",
        "new_px": "101.2",
        "timestamp": 1_700_000_000_000_i64,
        "reason": "legacy_alias_should_block_batch"
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions_for_execution(&[open, risk, invalid], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .unwrap_or_default()
        .contains("已删除字段别名 new_px"));
}

#[test]
fn runtime_standalone_risk_order_without_reference_price_uses_exchange_position_context() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let risk = RuntimeStrategyAction::from_value(&json!({
        "action": "place_risk_order",
        "symbol": "ETH-USDT-SWAP",
        "side": "sell",
        "order_type": "stop_market",
        "trigger_price": 94.0,
        "stop_loss_bps": 600,
        "reason": "protect_existing_long_without_reference_price",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[risk], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert_eq!(risk_orders.len(), 1);
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[0].action, StrategyIntentAction::PlaceRiskOrder);
    assert_eq!(intents[0].action_record.price, 0.0);
    assert_eq!(intents[0].attached_risk_orders.len(), 1);
}

#[test]
fn runtime_standalone_risk_order_with_position_size_requires_reference_price() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let risk = RuntimeStrategyAction::from_value(&json!({
        "action": "place_risk_order",
        "symbol": "ETH-USDT-SWAP",
        "side": "sell",
        "order_type": "stop_market",
        "trigger_price": 94.0,
        "position_size": 0.25,
        "reason": "protect_existing_long_sized_without_reference_price",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[risk], &latest, &config);

    assert!(intents.is_empty());
    assert_eq!(risk_orders.len(), 1);
    assert_eq!(skipped_actions.len(), 1);
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("缺少有效 price/reference_price")));
}

#[test]
fn runtime_market_close_action_without_reference_price_uses_exchange_position_context() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let close = RuntimeStrategyAction::from_value(&json!({
        "action": "close_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "sell",
        "order_type": "market",
        "reason": "close_existing_long_without_reference_price",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[close], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert!(risk_orders.is_empty());
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[0].action, StrategyIntentAction::ClosePosition);
    assert_eq!(intents[0].action_record.price, 0.0);
    assert_eq!(intents[0].order_side.as_deref(), Some("sell"));
}

#[test]
fn runtime_market_close_action_with_position_size_requires_reference_price() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let close = RuntimeStrategyAction::from_value(&json!({
        "action": "close_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "sell",
        "order_type": "market",
        "position_size": 0.25,
        "reason": "close_existing_long_sized_without_reference_price",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[close], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("缺少有效 price/reference_price")));
}

#[test]
fn runtime_risk_order_ignores_generic_order_price_for_trigger() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let open = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.1,
        "timestamp": 1_700_000_000_000_i64
    }));
    let risk = RuntimeStrategyAction::from_value(&json!({
        "action": "place_risk_order",
        "symbol": "ETH-USDT-SWAP",
        "side": "sell",
        "order_type": "stop_market",
        "price": 99.0,
        "trigger_price": 94.0,
        "stop_loss_bps": 600,
        "reason": "protective_stop_with_limit_price",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[open, risk], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert_eq!(risk_orders.len(), 1);
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[0].attached_risk_orders.len(), 1);
    assert_eq!(intents[0].attached_risk_orders[0].trigger_price, Some(94.0));
}

#[test]
fn runtime_risk_order_generic_price_is_not_trigger_price() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let open = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.1,
        "timestamp": 1_700_000_000_000_i64
    }));
    let risk = RuntimeStrategyAction::from_value(&json!({
        "action": "place_risk_order",
        "symbol": "ETH-USDT-SWAP",
        "side": "sell",
        "order_type": "stop_market",
        "price": 94.0,
        "reason": "generic_price_must_not_be_trigger",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[open, risk], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert_eq!(risk_orders.len(), 1);
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[0].attached_risk_orders.len(), 1);
    assert_eq!(intents[0].attached_risk_orders[0].trigger_price, None);
}

#[test]
fn runtime_risk_order_rejects_removed_alias_before_risk_order_collection() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let risk = RuntimeStrategyAction::from_value(&json!({
        "action": "place_risk_order",
        "symbol": "ETH-USDT-SWAP",
        "side": "sell",
        "order_type": "stop_market",
        "trigger_price": 94.0,
        "sz": "0.1",
        "reason": "old_size_alias",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[risk], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .is_some_and(|reason| {
            reason.contains("已删除字段别名 sz") && reason.contains("position_size")
        }));
}

#[test]
fn invalid_attached_risk_action_blocks_matching_open_position() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let open = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.1,
        "reason": "open_requires_protection",
        "timestamp": 1_700_000_000_000_i64
    }));
    let risk = RuntimeStrategyAction::from_value(&json!({
        "action": "place_risk_order",
        "symbol": "ETH-USDT",
        "inst_type": "SWAP",
        "side": "sell",
        "order_type": "stop_market",
        "trigger_price": 94.0,
        "sz": "0.1",
        "reason": "invalid_protective_stop",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[open, risk], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 2);
    assert!(skipped_actions.iter().any(|action| action["action"]
        .as_str()
        .is_some_and(|value| value == "place_risk_order")
        && action["_execution_skip_reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("已删除字段别名 sz"))));
    assert!(skipped_actions.iter().any(|action| action["action"]
        .as_str()
        .is_some_and(|value| value == "open_position")
        && action["_execution_skip_reason"]
            .as_str()
            .is_some_and(|reason| {
                reason.contains("保护单动作未通过合约校验")
                    && reason.contains("拒绝裸开仓")
                    && reason.contains("已删除字段别名 sz")
            })));
}

#[test]
fn invalid_risk_action_does_not_block_unrelated_open_symbol() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let open = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "BTC-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.1,
        "reason": "btc_open_without_eth_protection",
        "timestamp": 1_700_000_000_000_i64
    }));
    let risk = RuntimeStrategyAction::from_value(&json!({
        "action": "place_risk_order",
        "symbol": "ETH-USDT-SWAP",
        "side": "sell",
        "order_type": "stop_market",
        "trigger_price": 94.0,
        "sz": "0.1",
        "reason": "invalid_eth_protective_stop",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[open, risk], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].symbol, "BTC-USDT-SWAP");
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert_eq!(
        skipped_actions[0]["action"].as_str(),
        Some("place_risk_order")
    );
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("已删除字段别名 sz")));
}

#[test]
fn runtime_open_action_preserves_planned_exit_contract() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.1,
        "timestamp": 1_700_000_000_000_i64,
        "planned_exit_time": 1_700_000_900_000_i64,
        "planned_exit_reason": "hold_bars_elapsed",
        "planned_exit_contract": "planned_exit_time_v1"
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert!(risk_orders.is_empty());
    assert!(skipped_actions.is_empty());
    assert_eq!(
        intents[0].planned_exit,
        Some(StrategyPlannedExitIntent {
            timestamp: 1_700_000_900_000,
            reason: "hold_bars_elapsed".to_string(),
            contract: "planned_exit_time_v1".to_string(),
        })
    );
}

#[test]
fn runtime_open_action_rejects_removed_planned_exit_aliases() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.1,
        "timestamp": 1_700_000_000_000_i64,
        "exit_time": 1_700_000_900_000_i64,
        "exit_reason": "hold_bars_elapsed",
        "action_contract_version": "planned_exit_time_v1"
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .is_some_and(|reason| {
            reason.contains("已删除字段别名 exit_time") && reason.contains("planned_exit_time")
        }));
}

#[test]
fn runtime_open_action_preserves_explicit_order_type() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "limit",
        "price": 101.25,
        "position_size": 0.1,
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert!(risk_orders.is_empty());
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[0].order_type, "limit");
}

#[test]
fn runtime_open_action_normalizes_order_type_alias() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "post-only",
        "price": 101.25,
        "position_size": 0.1,
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert!(risk_orders.is_empty());
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[0].order_type, "post_only");
}

#[test]
fn runtime_open_action_rejects_removed_size_alias() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "quantity": 0.31,
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("已删除字段别名 quantity")));
}

#[test]
fn runtime_open_action_preserves_explicit_exchange_size() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.31,
        "exchange_size": "2.03",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert!(risk_orders.is_empty());
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[0].action_record.position_size, Some(0.31));
    assert_eq!(intents[0].exchange_size.as_deref(), Some("2.03"));
}

#[test]
fn runtime_action_normalizes_symbol_with_action_inst_type() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "eth-usdt",
        "inst_type": "SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.31,
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert!(risk_orders.is_empty());
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[0].symbol, "ETH-USDT-SWAP");
    assert_eq!(intents[0].inst_type, "SWAP");
}

#[test]
fn runtime_action_infers_swap_inst_type_from_explicit_symbol_suffix() {
    let mut config = test_strategy_config();
    config.symbol = "BTC-USDT".to_string();
    config.inst_type = "SPOT".to_string();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "eth-usdt-swap",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.31,
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert!(risk_orders.is_empty());
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[0].symbol, "ETH-USDT-SWAP");
    assert_eq!(intents[0].inst_type, "SWAP");
}

#[test]
fn runtime_risk_order_attaches_after_symbol_inst_type_normalization() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let open = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "eth-usdt",
        "inst_type": "SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.1,
        "timestamp": 1_700_000_000_000_i64
    }));
    let risk = RuntimeStrategyAction::from_value(&json!({
        "action": "place_risk_order",
        "symbol": "ETH-USDT",
        "inst_type": "SWAP",
        "side": "sell",
        "order_type": "stop_market",
        "trigger_price": 94.0,
        "stop_loss_bps": 600,
        "reason": "protective_stop",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[open, risk], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert_eq!(risk_orders.len(), 1);
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[0].symbol, "ETH-USDT-SWAP");
    assert_eq!(intents[0].attached_risk_orders.len(), 1);
    assert_eq!(intents[0].attached_risk_orders[0].symbol, "ETH-USDT-SWAP");
    assert_eq!(intents[0].attached_risk_orders[0].trigger_price, Some(94.0));
}

#[test]
fn runtime_risk_order_uses_target_order_type_for_protective_kind() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let open = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "BTC-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "reference_price": 100.0,
        "position_size": 0.1,
        "timestamp": 1_700_000_000_000_i64
    }));
    let risk = RuntimeStrategyAction::from_value(&json!({
        "action": "place_risk_order",
        "symbol": "BTC-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "target_order_type": "stop_loss_market",
        "stop_loss_pct": 0.05,
        "take_profit_pct": 0.12,
        "reason": "robust_strategy_style_stop",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[open, risk], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert_eq!(risk_orders.len(), 1);
    assert!(skipped_actions.is_empty());
    let attached = &intents[0].attached_risk_orders[0];
    assert_eq!(attached.side, "sell");
    assert_eq!(attached.order_type, "stop_loss_market");
    assert_eq!(attached.stop_loss, Some(0.05));
    assert_eq!(attached.take_profit, Some(0.12));
}

#[test]
fn price_required_order_type_requires_explicit_action_price() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "limit",
        "position_size": 0.1,
        "reason": "limit_without_price_must_not_use_close",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert_eq!(
        skipped_actions[0]["reason"].as_str(),
        Some("limit_without_price_must_not_use_close")
    );
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("需要显式 price")));
}

#[test]
fn market_action_requires_explicit_reference_price() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "position_size": 0.1,
        "reason": "market_without_reference_price_must_not_use_close",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert_eq!(
        skipped_actions[0]["reason"].as_str(),
        Some("market_without_reference_price_must_not_use_close")
    );
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("缺少有效 price/reference_price")));
}

#[test]
fn runtime_close_action_alias_is_rejected() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "close",
        "symbol": "ETH-USDT-SWAP",
        "side": "short",
        "order_type": "market",
        "price": 100.0,
        "reason": "alias_close_short",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("暂不支持动作 close")));
}

#[test]
fn cancel_and_modify_order_actions_are_executable() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let open = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.1,
        "timestamp": 1_700_000_000_000_i64
    }));
    let cancel = RuntimeStrategyAction::from_value(&json!({
        "action": "cancel_order",
        "symbol": "ETH-USDT-SWAP",
        "order_id": "live-order-1",
        "reason": "strategy_cancel_request",
        "timestamp": 1_700_000_000_000_i64
    }));
    let modify = RuntimeStrategyAction::from_value(&json!({
        "action": "modify_order",
        "symbol": "ETH-USDT-SWAP",
        "order_id": "live-order-2",
        "new_price": "101.25",
        "new_size": "2",
        "cancel_on_fail": true,
        "request_id": "modify-req-1",
        "reason": "strategy_modify_request",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[open, cancel, modify], &latest, &config);

    assert_eq!(intents.len(), 3);
    assert!(risk_orders.is_empty());
    assert!(skipped_actions.is_empty());
    assert_eq!(intents[1].action, StrategyIntentAction::CancelOrder);
    assert_eq!(
        intents[1].cancel_order,
        Some(StrategyCancelOrderIntent {
            order_id: "live-order-1".to_string(),
            client_order_id: String::new(),
            scope_explicit: true,
            target_kind: StrategyOrderTargetKind::Any,
        })
    );
    assert_eq!(intents[1].action_record.action, "cancel_order");
    assert_eq!(intents[1].action_record.side, "hold");
    assert_eq!(intents[2].action, StrategyIntentAction::ModifyOrder);
    assert_eq!(
        intents[2].modify_order,
        Some(StrategyModifyOrderIntent {
            order_id: "live-order-2".to_string(),
            client_order_id: String::new(),
            new_size: Some("2".to_string()),
            new_price: Some("101.25".to_string()),
            cancel_on_fail: true,
            request_id: "modify-req-1".to_string(),
            scope_explicit: true,
            target_kind: StrategyOrderTargetKind::Any,
            target_order_type: None,
        })
    );
    assert_eq!(intents[2].action_record.action, "modify_order");
    assert_eq!(intents[2].action_record.side, "hold");
}

#[test]
fn cancel_order_scope_explicit_uses_runner_symbol_marker() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let cancel = RuntimeStrategyAction::from_value(&json!({
        "action": "cancel_order",
        "symbol": "BTC-USDT-SWAP",
        "_symbol_explicit": false,
        "order_id": "live-order-1",
        "reason": "strategy_cancel_default_symbol",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[cancel], &latest, &config);

    assert_eq!(intents.len(), 1);
    assert!(risk_orders.is_empty());
    assert!(skipped_actions.is_empty());
    assert_eq!(
        intents[0].cancel_order,
        Some(StrategyCancelOrderIntent {
            order_id: "live-order-1".to_string(),
            client_order_id: String::new(),
            scope_explicit: false,
            target_kind: StrategyOrderTargetKind::Any,
        })
    );
}

#[test]
fn order_management_actions_parse_explicit_target_order_kind() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let cancel = RuntimeStrategyAction::from_value(&json!({
        "action": "cancel_order",
        "symbol": "ETH-USDT-SWAP",
        "client_order_id": "algo-client-1",
        "target_order_kind": "algo",
        "reason": "cancel_external_algo",
        "timestamp": 1_700_000_000_000_i64
    }));
    let modify = RuntimeStrategyAction::from_value(&json!({
        "action": "modify_order",
        "symbol": "ETH-USDT-SWAP",
        "client_order_id": "algo-client-2",
        "target_order_kind": "algo",
        "target_order_type": "stop-market",
        "new_price": "99.5",
        "reason": "modify_external_algo",
        "timestamp": 1_700_000_000_000_i64
    }));
    let any_cancel = RuntimeStrategyAction::from_value(&json!({
        "action": "cancel_order",
        "symbol": "ETH-USDT-SWAP",
        "client_order_id": "auto-match-client",
        "target_order_kind": "any",
        "reason": "cancel_auto_match",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[cancel, modify, any_cancel], &latest, &config);

    assert_eq!(intents.len(), 3);
    assert!(risk_orders.is_empty());
    assert!(skipped_actions.is_empty());
    assert_eq!(
        intents[0].cancel_order,
        Some(StrategyCancelOrderIntent {
            order_id: String::new(),
            client_order_id: "algo-client-1".to_string(),
            scope_explicit: true,
            target_kind: StrategyOrderTargetKind::Algo,
        })
    );
    assert_eq!(
        intents[1].modify_order,
        Some(StrategyModifyOrderIntent {
            order_id: String::new(),
            client_order_id: "algo-client-2".to_string(),
            new_size: None,
            new_price: Some("99.5".to_string()),
            cancel_on_fail: false,
            request_id: String::new(),
            scope_explicit: true,
            target_kind: StrategyOrderTargetKind::Algo,
            target_order_type: Some("stop_market".to_string()),
        })
    );
    assert_eq!(
        intents[2].cancel_order,
        Some(StrategyCancelOrderIntent {
            order_id: String::new(),
            client_order_id: "auto-match-client".to_string(),
            scope_explicit: true,
            target_kind: StrategyOrderTargetKind::Any,
        })
    );
}

#[test]
fn order_management_target_order_kind_must_be_known() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "cancel_order",
        "symbol": "ETH-USDT-SWAP",
        "order_id": "live-order-1",
        "target_order_kind": "paper",
        "reason": "invalid_target_order_kind",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("target_order_kind")));
}

#[test]
fn order_management_target_order_type_must_be_supported_protective_market_type() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "modify_order",
        "symbol": "ETH-USDT-SWAP",
        "client_order_id": "algo-client-2",
        "target_order_kind": "algo",
        "target_order_type": "take_profit_limit",
        "new_price": "101.5",
        "reason": "invalid_target_order_type",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("target_order_type")));
}

#[test]
fn modify_order_requires_explicit_amend_fields_not_generic_price() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let modify = RuntimeStrategyAction::from_value(&json!({
        "action": "modify_order",
        "symbol": "ETH-USDT-SWAP",
        "order_id": "live-order-2",
        "price": 101.25,
        "reason": "generic_price_must_not_be_amend_price",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[modify], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert_eq!(skipped_actions[0]["action"].as_str(), Some("modify_order"));
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .unwrap_or_default()
        .contains("缺少显式 new_size"));
}

#[test]
fn runtime_action_rejects_non_string_order_management_text_fields() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let cancel = RuntimeStrategyAction::from_value(&json!({
        "action": "cancel_order",
        "symbol": "ETH-USDT-SWAP",
        "order_id": 12345,
        "reason": "numeric_order_id_must_not_be_coerced",
        "timestamp": 1_700_000_000_000_i64
    }));
    let modify = RuntimeStrategyAction::from_value(&json!({
        "action": "modify_order",
        "symbol": "ETH-USDT-SWAP",
        "order_id": "live-order-2",
        "new_price": 101.25,
        "reason": "numeric_new_price_must_not_be_coerced",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[cancel, modify], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 2);
    assert!(skipped_actions
        .iter()
        .any(|action| action["_execution_skip_reason"]
            .as_str()
            .unwrap_or_default()
            .contains("order_id 必须是 JSON string")));
    assert!(skipped_actions
        .iter()
        .any(|action| action["_execution_skip_reason"]
            .as_str()
            .unwrap_or_default()
            .contains("new_price 必须是 JSON string")));
}

#[test]
fn runtime_action_rejects_non_string_order_semantic_text_fields() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let numeric_order_type = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": 123,
        "price": 100.0,
        "position_size": 0.1,
        "reason": "numeric_order_type_must_not_default_to_market",
        "timestamp": 1_700_000_000_000_i64
    }));
    let boolean_side = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": true,
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.1,
        "reason": "boolean_side_must_not_be_coerced",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[numeric_order_type, boolean_side], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 2);
    assert!(skipped_actions
        .iter()
        .any(|action| action["_execution_skip_reason"]
            .as_str()
            .unwrap_or_default()
            .contains("order_type 必须是 JSON string")));
    assert!(skipped_actions
        .iter()
        .any(|action| action["_execution_skip_reason"]
            .as_str()
            .unwrap_or_default()
            .contains("side 必须是 JSON string")));
}

#[test]
fn runtime_action_rejects_engine_controlled_exchange_fields() {
    let config = test_strategy_config();
    let latest = test_candle(100.0);
    let action = RuntimeStrategyAction::from_value(&json!({
        "action": "open_position",
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "order_type": "market",
        "price": 100.0,
        "position_size": 0.1,
        "reduceOnly": true,
        "reason": "engine_controlled_reduce_only",
        "timestamp": 1_700_000_000_000_i64
    }));

    let (intents, risk_orders, skipped_actions, _idle_actions) =
        plan_runtime_actions(&[action], &latest, &config);

    assert!(intents.is_empty());
    assert!(risk_orders.is_empty());
    assert_eq!(skipped_actions.len(), 1);
    assert!(skipped_actions[0]["_execution_skip_reason"]
        .as_str()
        .unwrap_or_default()
        .contains("交易引擎控制字段 reduceOnly"));
}

fn test_strategy_config() -> StrategyConfig {
    StrategyConfig {
        strategy_id: "runtime_action_test".to_string(),
        strategy_name: "Runtime Action Test".to_string(),
        symbol: "BTC-USDT-SWAP".to_string(),
        inst_type: "SWAP".to_string(),
        timeframe: "15m".to_string(),
        initial_capital: 1_000.0,
        position_size: 0.2,
        stop_loss: 0.0,
        take_profit: 0.0,
        params: json!({}),
    }
}

fn test_candle(close: f64) -> OkxCandle {
    OkxCandle {
        timestamp: 1_700_000_000_000,
        open: close,
        high: close,
        low: close,
        close,
        volume: 1.0,
        volume_ccy: 1.0,
        volume_quote: 1.0,
        confirm: "1".to_string(),
    }
}
