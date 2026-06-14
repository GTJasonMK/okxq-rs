use super::*;

use crate::{
    strategy_engine::{
        backtest::historical_live::state::{OrderAction, PlannedExit, SimOrder, SimPosition},
        StrategyActionRecord,
    },
    strategy_execution_contract::{
        StrategyCancelOrderIntent, StrategyExecutionIntent, StrategyIntentAction,
        StrategyModifyOrderIntent, StrategyOrderTargetKind, StrategyPlannedExitIntent,
        StrategyRiskOrderIntent,
    },
};

const SYMBOL: &str = "BTC-USDT-SWAP";
const INST_TYPE: &str = "SWAP";
const TIMEFRAME: &str = "1m";

#[test]
fn limit_orders_advance_after_unfilled_candle() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 96.0, 100.0, 10.0),
        candle(2, 100.0, 101.0, 94.0, 95.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "limit", 95.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    assert!(backtest.fills.is_empty());

    backtest.process_market_until(2, &market);

    assert_eq!(backtest.fills.len(), 1);
    assert_eq!(backtest.fills[0]["timestamp"].as_i64(), Some(2));
    assert_eq!(backtest.orders[0].filled, 1.0);
}

#[test]
fn historical_market_data_merges_duplicate_series_instead_of_overwriting() {
    let market = HistoricalMarketData::new(vec![
        HistoricalCandleSeries {
            symbol: SYMBOL.to_string(),
            inst_type: INST_TYPE.to_string(),
            timeframe: TIMEFRAME.to_string(),
            candles: vec![
                candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
                candle(2, 120.0, 121.0, 119.0, 120.0, 10.0),
            ],
        },
        HistoricalCandleSeries {
            symbol: SYMBOL.to_string(),
            inst_type: INST_TYPE.to_string(),
            timeframe: TIMEFRAME.to_string(),
            candles: vec![
                candle(2, 125.0, 126.0, 124.0, 125.0, 10.0),
                candle(3, 130.0, 131.0, 129.0, 130.0, 10.0),
            ],
        },
    ]);

    assert_eq!(market.last_close(SYMBOL, TIMEFRAME, 1), Some(100.0));
    assert_eq!(market.last_close(SYMBOL, TIMEFRAME, 2), Some(125.0));
    assert_eq!(market.last_close(SYMBOL, TIMEFRAME, 3), Some(130.0));
}

#[test]
fn order_with_unloaded_timeframe_is_rejected_instead_of_waiting_forever() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let mut intent = open_intent("1", "market", 100.0, Vec::new(), None);
    intent.timeframe = "5m".to_string();

    backtest.submit_intents(&[intent], 0, &market);
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.orders.len(), 1);
    assert_eq!(backtest.orders[0].status, "rejected");
    assert!(backtest.orders[0]
        .error_message
        .contains("回测缺少订单撮合K线序列 BTC-USDT-SWAP 5m"));
    assert_eq!(backtest.rejected_orders.len(), 1);
    assert!(backtest.fills.is_empty());
}

#[test]
fn historical_market_data_merges_duplicate_funding_series_instead_of_overwriting() {
    let market = market(&[candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)]).with_funding(vec![
        HistoricalFundingSeries {
            symbol: SYMBOL.to_string(),
            inst_type: INST_TYPE.to_string(),
            rates: vec![
                HistoricalFundingPoint {
                    symbol: SYMBOL.to_string(),
                    inst_type: INST_TYPE.to_string(),
                    funding_time: 1,
                    funding_rate: 0.001,
                },
                HistoricalFundingPoint {
                    symbol: SYMBOL.to_string(),
                    inst_type: INST_TYPE.to_string(),
                    funding_time: 2,
                    funding_rate: 0.002,
                },
            ],
        },
        HistoricalFundingSeries {
            symbol: SYMBOL.to_string(),
            inst_type: INST_TYPE.to_string(),
            rates: vec![
                HistoricalFundingPoint {
                    symbol: SYMBOL.to_string(),
                    inst_type: INST_TYPE.to_string(),
                    funding_time: 2,
                    funding_rate: 0.003,
                },
                HistoricalFundingPoint {
                    symbol: SYMBOL.to_string(),
                    inst_type: INST_TYPE.to_string(),
                    funding_time: 3,
                    funding_rate: 0.004,
                },
            ],
        },
    ]);

    let rates = market.funding_between(SYMBOL, INST_TYPE, 0, 3);
    let observed = rates
        .iter()
        .map(|point| (point.funding_time, point.funding_rate))
        .collect::<Vec<_>>();
    assert_eq!(observed, vec![(1, 0.001), (2, 0.003), (3, 0.004)]);
}

#[test]
fn backtest_rejects_invalid_configured_leverage_instead_of_clamping() {
    for params in [
        json!({"leverage": 200}),
        json!({"leverage": 0}),
        json!({"leverage": "bad"}),
        json!({"contract_mode": false, "leverage": 3}),
    ] {
        let config = test_config(params);
        let error = match HistoricalLiveBacktest::try_new(&config, &[], 1) {
            Ok(_) => panic!("invalid leverage config must be rejected before simulation starts"),
            Err(error) => error.to_string(),
        };

        assert!(
            error.contains("回测")
                && (error.contains("leverage") || error.contains("contract_mode=false")),
            "unexpected validation error: {error}"
        );
    }
}

#[test]
fn backtest_rejects_invalid_cost_model_params_instead_of_clamping() {
    for (params, expected_key) in [
        (
            json!({"historical_fee_rate": -0.001}),
            "historical_fee_rate",
        ),
        (
            json!({"historical_slippage_bps": 600}),
            "historical_slippage_bps",
        ),
        (
            json!({"historical_spread_rate": 0.2}),
            "historical_spread_rate",
        ),
        (
            json!({"historical_participation_rate": 0}),
            "historical_participation_rate",
        ),
        (json!({"historical_fee_rate": "bad"}), "historical_fee_rate"),
    ] {
        let config = test_config(params);
        let error = match HistoricalLiveBacktest::try_new(&config, &[], 1) {
            Ok(_) => panic!("invalid cost model config must be rejected before simulation starts"),
            Err(error) => error.to_string(),
        };

        assert!(
            error.contains(expected_key),
            "unexpected validation error: {error}"
        );
        assert!(
            error.contains("静默"),
            "cost model validation should explain hidden adjustment risk: {error}"
        );
    }
}

#[test]
fn backtest_rejects_action_scoped_invalid_contract_leverage_before_order() {
    let mut config = test_config(json!({
        "leverage": 200,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    config.inst_type = "SPOT".to_string();
    let candles = vec![swap_candle(1, 100.0, 101.0, 99.0, 100.0, 10.0, 0.1)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let open = intent(
        StrategyIntentAction::OpenPosition,
        "long",
        "market",
        100.0,
        None,
        Some("1".to_string()),
        Vec::new(),
        None,
    );

    assert_eq!(open.inst_type, "SWAP");
    assert!(backtest
        .configured_leverage_for_inst_type(&open.inst_type)
        .is_err());

    backtest.submit_intents(&[open], 0, &market);

    assert_eq!(backtest.orders.len(), 1);
    assert_eq!(backtest.orders[0].status, "rejected");
    assert_eq!(backtest.orders[0].exchange_filled, 0.0);
    let rejection = backtest
        .rejected_orders
        .last()
        .expect("invalid leverage should reject the action before order creation");
    assert_eq!(rejection["action"].as_str(), Some("open_position"));
    assert!(rejection["error_message"]
        .as_str()
        .unwrap_or_default()
        .contains("125x"));
}

#[test]
fn action_max_slippage_blocks_backtest_open_like_live() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.001,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let mut open = open_intent("1", "market", 100.0, Vec::new(), None);
    open.max_slippage = Some(0.0005);

    let execution_config = backtest.execution_config_for_intent(&open);
    assert_eq!(
        execution_config.params["_runtime_max_slippage"].as_f64(),
        Some(0.0005)
    );

    backtest.submit_intents(&[open], 1, &market);

    assert_eq!(backtest.orders[0].status, "rejected");
    assert!(backtest.rejected_orders[0]["error_message"]
        .as_str()
        .unwrap_or("")
        .contains("预估滑点"));
}

#[test]
fn invalid_open_side_is_rejected_like_live_instead_of_defaulting_long() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let mut open = open_intent("1", "market", 100.0, Vec::new(), None);
    open.action_record.side = "invalid-side".to_string();

    backtest.submit_intents(&[open], 1, &market);

    assert_eq!(backtest.orders.len(), 1);
    assert_eq!(backtest.orders[0].status, "rejected");
    assert_eq!(backtest.orders[0].side, "unknown");
    assert_eq!(backtest.orders[0].pos_side, "unknown");
    assert!(backtest.positions.is_empty());
    assert!(backtest.orders[0]
        .error_message
        .contains("无法从策略 side=invalid-side 推导开仓方向"));
}

#[test]
fn malformed_queued_order_with_invalid_side_is_rejected_instead_of_default_fill() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    backtest.orders.push(SimOrder {
        client_order_id: "bt-cl-malformed-side".to_string(),
        order_id: "malformed-side".to_string(),
        symbol: SYMBOL.to_string(),
        inst_type: INST_TYPE.to_string(),
        timeframe: TIMEFRAME.to_string(),
        action: OrderAction::Open,
        order_type: "market".to_string(),
        side: "sideways".to_string(),
        pos_side: "long".to_string(),
        leverage: 1.0,
        exchange_quantity: 1.0,
        exchange_filled: 0.0,
        quantity: 1.0,
        filled: 0.0,
        fill_summary: Default::default(),
        price: None,
        reference_price: 100.0,
        reference_price_source: "test_fixture".to_string(),
        reference_price_missing: false,
        trigger_price: None,
        risk_kind: None,
        status: "open".to_string(),
        reason: "malformed_side".to_string(),
        submitted_ts: 0,
        last_processed_ts: 0,
        action_ts: 0,
        reduce_only: false,
        entry_order_id: None,
        attached_risk_identity: None,
        attached_risk_orders: Vec::new(),
        planned_exit: None,
        error_message: String::new(),
    });

    backtest.process_market_until(1, &market);

    assert!(backtest.fills.is_empty());
    assert!(backtest.positions.is_empty());
    assert_eq!(backtest.orders[0].status, "rejected");
    assert!(backtest.orders[0]
        .error_message
        .contains("订单方向无效: sideways"));
    assert_eq!(backtest.rejected_orders.len(), 1);
}

#[test]
fn malformed_queued_order_with_invalid_pos_side_is_rejected_instead_of_defaulting_long() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    backtest.orders.push(SimOrder {
        client_order_id: "bt-cl-malformed-pos-side".to_string(),
        order_id: "malformed-pos-side".to_string(),
        symbol: SYMBOL.to_string(),
        inst_type: INST_TYPE.to_string(),
        timeframe: TIMEFRAME.to_string(),
        action: OrderAction::Open,
        order_type: "market".to_string(),
        side: "buy".to_string(),
        pos_side: "sideways".to_string(),
        leverage: 1.0,
        exchange_quantity: 1.0,
        exchange_filled: 0.0,
        quantity: 1.0,
        filled: 0.0,
        fill_summary: Default::default(),
        price: None,
        reference_price: 100.0,
        reference_price_source: "test_fixture".to_string(),
        reference_price_missing: false,
        trigger_price: None,
        risk_kind: None,
        status: "open".to_string(),
        reason: "malformed_pos_side".to_string(),
        submitted_ts: 0,
        last_processed_ts: 0,
        action_ts: 0,
        reduce_only: false,
        entry_order_id: None,
        attached_risk_identity: None,
        attached_risk_orders: Vec::new(),
        planned_exit: None,
        error_message: String::new(),
    });

    backtest.process_market_until(1, &market);

    assert!(backtest.fills.is_empty());
    assert!(backtest.positions.is_empty());
    assert_eq!(backtest.orders[0].status, "rejected");
    assert!(backtest.orders[0]
        .error_message
        .contains("持仓方向无效: sideways"));
    assert_eq!(backtest.rejected_orders.len(), 1);
}

#[test]
fn invalid_planned_exit_side_is_rejected_instead_of_defaulting_sell() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.planned_exits.push(PlannedExit {
        symbol: SYMBOL.to_string(),
        inst_type: INST_TYPE.to_string(),
        timeframe: TIMEFRAME.to_string(),
        side: "sideways".to_string(),
        exchange_quantity: 1.0,
        quantity: 1.0,
        due_ts: 1,
        entry_order_id: None,
        reason: "invalid_side".to_string(),
        contract: "planned_exit_time_v1".to_string(),
        submitted: false,
    });

    let submitted = backtest.submit_due_planned_exits(1, &market);

    assert_eq!(submitted, 1);
    assert!(backtest.orders.is_empty());
    assert_eq!(backtest.rejected_orders.len(), 1);
    assert_eq!(
        backtest.rejected_orders[0]["action"].as_str(),
        Some("close_position")
    );
    assert_eq!(
        backtest.rejected_orders[0]["side"].as_str(),
        Some("unknown")
    );
    assert_eq!(
        backtest.rejected_orders[0]["pos_side"].as_str(),
        Some("sideways")
    );
    assert!(backtest.rejected_orders[0]["error_message"]
        .as_str()
        .unwrap_or("")
        .contains("计划退出持仓方向无效"));
}

#[test]
fn planned_exit_without_position_is_rejected_instead_of_creating_zero_price_close() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.planned_exits.push(PlannedExit {
        symbol: SYMBOL.to_string(),
        inst_type: INST_TYPE.to_string(),
        timeframe: TIMEFRAME.to_string(),
        side: "long".to_string(),
        exchange_quantity: 1.0,
        quantity: 1.0,
        due_ts: 1,
        entry_order_id: Some("missing-entry".to_string()),
        reason: "stale_plan".to_string(),
        contract: "planned_exit_time_v1".to_string(),
        submitted: false,
    });

    let submitted = backtest.submit_due_planned_exits(1, &market);

    assert_eq!(submitted, 1);
    assert!(backtest.orders.is_empty());
    assert_eq!(backtest.rejected_orders.len(), 1);
    assert_eq!(
        backtest.rejected_orders[0]["action"].as_str(),
        Some("close_position")
    );
    assert!(backtest.rejected_orders[0]["error_message"]
        .as_str()
        .unwrap_or("")
        .contains("计划退出没有可平持仓"));
}

#[test]
fn planned_exit_submits_current_available_position_quantity() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    backtest.positions.insert(
        (SYMBOL.to_string(), "long".to_string()),
        SimPosition {
            symbol: SYMBOL.to_string(),
            inst_type: INST_TYPE.to_string(),
            timeframe: TIMEFRAME.to_string(),
            side: "long".to_string(),
            exchange_quantity: 1.0,
            quantity: 1.0,
            entry_price: 100.0,
            leverage: 1.0,
            realized_pnl: 0.0,
            opened_ts: 1,
            last_funding_ts: 1,
            accumulated_funding: 0.0,
            entry_order_id: "entry-1".to_string(),
        },
    );
    backtest.planned_exits.push(PlannedExit {
        symbol: SYMBOL.to_string(),
        inst_type: INST_TYPE.to_string(),
        timeframe: TIMEFRAME.to_string(),
        side: "long".to_string(),
        exchange_quantity: 2.0,
        quantity: 2.0,
        due_ts: 1,
        entry_order_id: Some("entry-1".to_string()),
        reason: "stale_size".to_string(),
        contract: "planned_exit_time_v1".to_string(),
        submitted: false,
    });

    let submitted = backtest.submit_due_planned_exits(1, &market);

    assert_eq!(submitted, 1);
    assert_eq!(backtest.orders.len(), 1);
    assert!(backtest.rejected_orders.is_empty());
    assert_eq!(backtest.orders[0].exchange_quantity, 1.0);
    assert_eq!(backtest.orders[0].quantity, 1.0);
    assert_eq!(backtest.orders[0].reference_price, 100.0);
}

#[test]
fn short_open_respects_allow_short_gate_like_live() {
    let config = test_config(json!({
        "allow_short": "false",
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let mut open = open_intent("1", "market", 100.0, Vec::new(), None);
    open.action_record.side = "short".to_string();

    backtest.submit_intents(&[open], 1, &market);

    assert_eq!(backtest.orders.len(), 1);
    assert_eq!(backtest.orders[0].status, "rejected");
    assert_eq!(backtest.orders[0].side, "sell");
    assert_eq!(backtest.orders[0].pos_side, "short");
    assert!(backtest.positions.is_empty());
    assert!(backtest.orders[0]
        .error_message
        .contains("策略配置不允许做空"));
}

#[test]
fn market_processing_rejection_uses_candle_event_timestamp() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 0.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.orders[0].status, "rejected");
    assert_eq!(backtest.orders[0].last_processed_ts, 1);
    assert_eq!(backtest.rejected_orders.len(), 1);
    assert_eq!(backtest.rejected_orders[0]["timestamp"].as_i64(), Some(1));
    assert_eq!(
        backtest.rejected_orders[0]["submitted_ts"].as_i64(),
        Some(0)
    );
    assert_eq!(
        backtest.rejected_orders[0]["action_timestamp"].as_i64(),
        Some(0)
    );
    assert_eq!(
        backtest.rejected_orders[0]["action"].as_str(),
        Some("open_position")
    );
    assert_eq!(
        backtest.rejected_orders[0]["pos_side"].as_str(),
        Some("long")
    );
    assert_eq!(
        backtest.rejected_orders[0]["order_type"].as_str(),
        Some("market")
    );
    assert!(backtest.rejected_orders[0]["error_message"]
        .as_str()
        .unwrap_or("")
        .contains("K线成交量为 0"));
}

#[test]
fn optimal_limit_ioc_cancels_unfilled_remainder_after_first_market_attempt() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 0.5,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 2.0),
        candle(2, 101.0, 102.0, 100.0, 101.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent(
            "3",
            "optimal_limit_ioc",
            100.0,
            Vec::new(),
            None,
        )],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.orders[0].exchange_filled, 1.0);
    assert_eq!(backtest.orders[0].status, "cancelled");
    assert!(backtest.orders[0].error_message.contains("立即成交"));
    assert_eq!(backtest.fills.len(), 1);

    let order_context = backtest.orders_context();
    assert_eq!(
        order_context["recent_fills"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        order_context["recent_rejections"].as_array().map(Vec::len),
        Some(0)
    );

    backtest.process_market_until(2, &market);

    assert_eq!(backtest.fills.len(), 1);
    assert_eq!(backtest.orders[0].exchange_filled, 1.0);
    assert_eq!(backtest.positions.len(), 1);
    assert_eq!(
        backtest
            .positions
            .values()
            .next()
            .map(|position| position.exchange_quantity),
        Some(1.0)
    );
}

#[test]
fn market_order_cancels_unfilled_remainder_after_first_market_attempt() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 0.5,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 2.0),
        candle(2, 101.0, 102.0, 100.0, 101.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("3", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.orders[0].exchange_filled, 1.0);
    assert_eq!(backtest.orders[0].status, "cancelled");
    assert!(backtest.orders[0].error_message.contains("立即成交"));
    assert_eq!(backtest.fills.len(), 1);

    backtest.process_market_until(2, &market);

    assert_eq!(backtest.fills.len(), 1);
    assert_eq!(backtest.orders[0].exchange_filled, 1.0);

    let report = backtest.finish(&market, json!({}));
    let order = &report.detail["orders"][0];
    let fill = &report.detail["fills"][0];
    assert_eq!(order["status"].as_str(), Some("cancelled"));
    assert_eq!(order["filled_size"].as_f64(), Some(1.0));
    assert_eq!(order["fill_count"].as_u64(), Some(1));
    assert_eq!(order["avg_fill_price"].as_f64(), Some(100.0));
    assert_eq!(order["fill_notional"].as_f64(), fill["value"].as_f64());
    assert_eq!(order["total_fee"].as_f64(), fill["commission"].as_f64());
    assert_eq!(order["first_fill_ts"].as_i64(), fill["timestamp"].as_i64());
    assert_eq!(order["last_fill_ts"].as_i64(), fill["timestamp"].as_i64());
    assert_eq!(order["success"].as_bool(), Some(true));
}

#[test]
fn ioc_order_uses_limit_price_and_cancels_when_price_not_touched() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 96.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "ioc", 95.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert!(backtest.fills.is_empty());
    assert!(backtest.positions.is_empty());
    assert_eq!(backtest.orders[0].status, "cancelled");
    assert_eq!(backtest.orders[0].exchange_filled, 0.0);
    assert!(backtest.orders[0].error_message.contains("立即成交"));

    let order_context = backtest.orders_context();
    assert_eq!(order_context["open"].as_array().map(Vec::len), Some(0));
    assert_eq!(
        order_context["recent_fills"].as_array().map(Vec::len),
        Some(0)
    );
    assert_eq!(
        order_context["recent_rejections"].as_array().map(Vec::len),
        Some(1)
    );
    let rejection = &order_context["recent_rejections"][0];
    assert_eq!(rejection["status"].as_str(), Some("canceled"));
    assert_eq!(rejection["success"].as_bool(), Some(false));
    assert_eq!(rejection["filled_size"].as_f64(), Some(0.0));
    assert_eq!(rejection["size"].as_f64(), Some(1.0));
    assert!(rejection["error_message"]
        .as_str()
        .unwrap_or("")
        .contains("立即成交"));

    let report = backtest.finish(&market, json!({}));
    let order = &report.detail["orders"][0];
    assert_eq!(order["status"].as_str(), Some("cancelled"));
    assert_eq!(order["filled_size"].as_f64(), Some(0.0));
    assert_eq!(order["success"].as_bool(), Some(false));
}

#[test]
fn ioc_order_does_not_wait_for_later_intrabar_limit_touch() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 100.0, 95.0, 96.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "ioc", 95.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert!(backtest.fills.is_empty());
    assert!(backtest.positions.is_empty());
    assert_eq!(backtest.orders[0].status, "cancelled");
    assert_eq!(backtest.orders[0].exchange_filled, 0.0);
    assert!(backtest.orders[0].error_message.contains("立即成交"));
}

#[test]
fn marketable_limit_order_uses_open_like_price_not_worse_limit_price() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "limit", 110.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.fills.len(), 1);
    assert_eq!(backtest.fills[0]["price"].as_f64(), Some(100.0));
    assert_eq!(backtest.orders[0].status, "filled");
}

#[test]
fn fok_order_does_not_partially_fill_when_capacity_is_insufficient() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 0.5,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 2.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("3", "fok", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert!(backtest.fills.is_empty());
    assert!(backtest.positions.is_empty());
    assert_eq!(backtest.orders[0].status, "cancelled");
    assert_eq!(backtest.orders[0].exchange_filled, 0.0);
    assert!(backtest.orders[0].error_message.contains("FOK"));
}

#[test]
fn post_only_order_uses_limit_price_not_market_open_price() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 95.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "post_only", 95.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.fills.len(), 1);
    assert_eq!(backtest.fills[0]["price"].as_f64(), Some(95.0));
    assert_eq!(backtest.orders[0].status, "filled");
}

#[test]
fn marketable_post_only_order_cancels_instead_of_taking_liquidity() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "post_only", 101.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert!(backtest.fills.is_empty());
    assert!(backtest.positions.is_empty());
    assert_eq!(backtest.orders[0].status, "cancelled");
    assert!(backtest.orders[0].error_message.contains("post-only"));
}

#[test]
fn non_marketable_post_only_order_can_fill_after_resting() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 96.0, 100.0, 10.0),
        candle(2, 100.0, 101.0, 95.0, 100.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "post_only", 95.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    assert!(backtest.fills.is_empty());
    assert_eq!(backtest.orders[0].status, "open");

    backtest.process_market_until(2, &market);

    assert_eq!(backtest.fills.len(), 1);
    assert_eq!(backtest.fills[0]["timestamp"].as_i64(), Some(2));
    assert_eq!(backtest.fills[0]["price"].as_f64(), Some(95.0));
    assert_eq!(backtest.orders[0].status, "filled");
}

#[test]
fn partial_entry_fills_advance_candles_and_resize_attached_exits() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 0.5,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 2.0),
        candle(2, 101.0, 102.0, 100.0, 101.0, 2.0),
        candle(3, 102.0, 103.0, 101.0, 102.0, 2.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let risk = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: None,
        stop_loss: Some(0.10),
        take_profit: None,
        reason: "protect_partial_entry".to_string(),
    };
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 10,
        reason: "hold_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };

    backtest.submit_intents(
        &[open_intent(
            "3",
            "limit",
            100.0,
            vec![risk],
            Some(planned_exit),
        )],
        0,
        &market,
    );

    backtest.process_market_until(1, &market);
    assert_eq!(backtest.orders[0].filled, 1.0);
    assert_eq!(attached_risk_quantity(&backtest), Some(1.0));
    assert_eq!(attached_risk_trigger(&backtest), Some(90.0));
    assert_eq!(
        attached_risk_order_type(&backtest).as_deref(),
        Some("stop_loss")
    );
    assert_eq!(backtest.planned_exits[0].quantity, 1.0);

    backtest.process_market_until(2, &market);

    let open_fill_timestamps = backtest
        .fills
        .iter()
        .filter(|fill| fill["action"].as_str() == Some("open_position"))
        .filter_map(|fill| fill["timestamp"].as_i64())
        .collect::<Vec<_>>();
    assert_eq!(open_fill_timestamps, vec![1, 2]);
    assert_eq!(backtest.orders[0].filled, 2.0);
    assert_eq!(attached_risk_quantity(&backtest), Some(2.0));
    assert_eq!(attached_risk_trigger(&backtest), Some(90.0));
    assert_eq!(backtest.planned_exits[0].quantity, 2.0);
}

#[test]
fn resized_attached_risk_order_still_scans_current_stop_candle() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 0.5,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 2.0),
        candle(2, 101.0, 102.0, 89.0, 101.0, 6.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let risk = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: None,
        stop_loss: Some(0.10),
        take_profit: None,
        reason: "partial_entry_stop_spike".to_string(),
    };

    backtest.submit_intents(
        &[open_intent("3", "limit", 100.0, vec![risk], None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    assert_eq!(backtest.orders[0].filled, 1.0);
    assert_eq!(attached_risk_trigger(&backtest), Some(90.0));

    backtest.process_market_until(2, &market);

    let fills = backtest
        .fills
        .iter()
        .map(|fill| {
            (
                fill["action"].as_str().unwrap_or_default().to_string(),
                fill["timestamp"].as_i64().unwrap_or_default(),
                fill["price"].as_f64().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        fills,
        vec![
            ("open_position".to_string(), 1, 100.0),
            ("open_position".to_string(), 2, 100.0),
            ("place_risk_order".to_string(), 2, 90.0),
        ]
    );
    assert!(backtest.positions.is_empty());
}

#[test]
fn newly_attached_stop_loss_scans_entry_fill_candle_spike() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 89.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let risk = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: None,
        stop_loss: Some(0.10),
        take_profit: None,
        reason: "entry_candle_stop_spike".to_string(),
    };

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, vec![risk], None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    let fills = backtest
        .fills
        .iter()
        .map(|fill| {
            (
                fill["action"].as_str().unwrap_or_default().to_string(),
                fill["timestamp"].as_i64().unwrap_or_default(),
                fill["price"].as_f64().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        fills,
        vec![
            ("open_position".to_string(), 1, 100.0),
            ("place_risk_order".to_string(), 1, 90.0),
        ]
    );
    assert!(backtest.positions.is_empty());
}

#[test]
fn planned_exit_created_by_fill_is_submitted_inside_current_market_window() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(2, 105.0, 106.0, 104.0, 105.0, 10.0),
        candle(3, 110.0, 111.0, 109.0, 110.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 2,
        reason: "hold_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };

    backtest.submit_intents(
        &[open_intent(
            "1",
            "market",
            100.0,
            Vec::new(),
            Some(planned_exit),
        )],
        0,
        &market,
    );
    backtest.process_market_until(3, &market);

    let fill_actions = backtest
        .fills
        .iter()
        .map(|fill| {
            (
                fill["action"].as_str().unwrap_or_default().to_string(),
                fill["timestamp"].as_i64().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        fill_actions,
        vec![
            ("open_position".to_string(), 1),
            ("close_position".to_string(), 3),
        ]
    );
    let planned_close = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Close))
        .expect("planned exit close should be submitted inside the same window");
    assert_eq!(planned_close.submitted_ts, 2);
    assert_eq!(planned_close.action_ts, 2);
    assert_eq!(planned_close.reference_price, 105.0);
    assert!(backtest.positions.is_empty());
}

#[test]
fn planned_exit_reference_price_fallback_is_exposed_in_runtime_summary() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let eth_market = market_for_symbol("ETH-USDT-SWAP", &candles);
    let btc_only_market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 2,
        reason: "hold_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };
    let open_eth = intent_for_symbol(
        "ETH-USDT-SWAP",
        StrategyIntentAction::OpenPosition,
        "long",
        "market",
        100.0,
        None,
        Some("1".to_string()),
        Vec::new(),
        Some(planned_exit),
    );

    backtest.submit_intents(&[open_eth], 0, &eth_market);
    backtest.process_market_until(1, &eth_market);
    backtest.process_market_until(2, &btc_only_market);

    let planned_close = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Close))
        .expect("planned exit close should be submitted");
    assert_eq!(planned_close.reference_price, 100.0);
    let planned_close_json = super::values::order_json(planned_close);
    assert_eq!(planned_close_json["reference_price"].as_f64(), Some(100.0));
    assert_eq!(
        planned_close_json["reference_price_source"].as_str(),
        Some("entry_price_fallback")
    );
    assert_eq!(
        planned_close_json["reference_price_missing"].as_bool(),
        Some(true)
    );
    assert_eq!(backtest.planned_exit_reference_price_fallbacks, 1);

    let report = backtest.finish(&btc_only_market, json!({}));
    let summary = &report.detail["runtime_action_summary"];
    assert_eq!(
        summary["planned_exit_reference_price_fallback_count"].as_u64(),
        Some(1)
    );
    assert!(summary["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str() == Some("planned_exit_reference_price_fallback")));
    assert_eq!(
        report.detail["simulation_assumptions"]["planned_exit_reference_price_fallback_count"]
            .as_u64(),
        Some(1)
    );
}

#[test]
fn planned_exit_close_preserves_entry_intent_inst_type() {
    let mut config = test_config(json!({
        "contract_mode": true,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    config.inst_type = "SPOT".to_string();
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(2, 105.0, 106.0, 104.0, 105.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1,
        reason: "hold_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };

    backtest.submit_intents(
        &[open_intent(
            "1",
            "market",
            100.0,
            Vec::new(),
            Some(planned_exit),
        )],
        0,
        &market,
    );
    backtest.process_market_until(2, &market);

    let planned_close = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Close))
        .expect("planned exit close should be submitted");
    assert_eq!(planned_close.symbol, SYMBOL);
    assert_eq!(planned_close.inst_type, INST_TYPE);
    let close_fill = backtest
        .fills
        .iter()
        .find(|fill| fill["action"].as_str() == Some("close_position"))
        .expect("planned exit close should fill");
    assert_eq!(close_fill["inst_type"].as_str(), Some(INST_TYPE));
}

#[test]
fn planned_exit_retries_residual_after_partial_market_close_cancel() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 0.5,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 8.0),
        candle(2, 105.0, 106.0, 104.0, 105.0, 4.0),
        candle(3, 110.0, 111.0, 109.0, 110.0, 4.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1,
        reason: "hold_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };

    backtest.submit_intents(
        &[open_intent(
            "4",
            "market",
            100.0,
            Vec::new(),
            Some(planned_exit),
        )],
        0,
        &market,
    );
    backtest.process_market_until(3, &market);

    let close_fills = backtest
        .fills
        .iter()
        .filter(|fill| fill["action"].as_str() == Some("close_position"))
        .map(|fill| {
            (
                fill["timestamp"].as_i64().unwrap_or_default(),
                fill["size"].as_f64().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(close_fills, vec![(2, 2.0), (3, 2.0)]);
    assert!(backtest.positions.is_empty());
    assert_eq!(
        backtest
            .orders
            .iter()
            .filter(|order| matches!(order.action, OrderAction::Close))
            .count(),
        2
    );
}

#[test]
fn planned_exit_requeue_rejects_inconsistent_position_quantity_instead_of_silent_fallback() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let mut backtest = HistoricalLiveBacktest::new(&config, &[], 1);
    backtest.positions.insert(
        (SYMBOL.to_string(), "long".to_string()),
        SimPosition {
            symbol: SYMBOL.to_string(),
            inst_type: INST_TYPE.to_string(),
            timeframe: TIMEFRAME.to_string(),
            side: "long".to_string(),
            exchange_quantity: 1.0,
            quantity: 0.0,
            entry_price: 100.0,
            leverage: 1.0,
            realized_pnl: 0.0,
            opened_ts: 1,
            last_funding_ts: 1,
            accumulated_funding: 0.0,
            entry_order_id: "entry-1".to_string(),
        },
    );
    backtest.planned_exits.push(PlannedExit {
        symbol: SYMBOL.to_string(),
        inst_type: INST_TYPE.to_string(),
        timeframe: TIMEFRAME.to_string(),
        side: "long".to_string(),
        exchange_quantity: 0.5,
        quantity: 0.5,
        due_ts: 2,
        entry_order_id: Some("entry-1".to_string()),
        reason: "hold_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
        submitted: true,
    });
    backtest.orders.push(SimOrder {
        client_order_id: "bt-cl-close-1".to_string(),
        order_id: "close-1".to_string(),
        symbol: SYMBOL.to_string(),
        inst_type: INST_TYPE.to_string(),
        timeframe: TIMEFRAME.to_string(),
        action: OrderAction::Close,
        order_type: "market".to_string(),
        side: "sell".to_string(),
        pos_side: "long".to_string(),
        leverage: 1.0,
        exchange_quantity: 2.0,
        exchange_filled: 1.0,
        quantity: 2.0,
        filled: 1.0,
        fill_summary: Default::default(),
        price: None,
        reference_price: 100.0,
        reference_price_source: "test_fixture".to_string(),
        reference_price_missing: false,
        trigger_price: None,
        risk_kind: None,
        status: "partially_filled".to_string(),
        reason: "planned_exit:planned_exit_time_v1:hold_elapsed".to_string(),
        submitted_ts: 2,
        last_processed_ts: 2,
        action_ts: 2,
        reduce_only: true,
        entry_order_id: Some("entry-1".to_string()),
        attached_risk_identity: None,
        attached_risk_orders: Vec::new(),
        planned_exit: None,
        error_message: String::new(),
    });

    backtest.cancel_order_index(0, 3, "测试取消计划退出剩余数量");

    assert_eq!(backtest.orders[0].status, "cancelled");
    assert_eq!(backtest.rejected_orders.len(), 1);
    assert!(backtest.rejected_orders[0]["error_message"]
        .as_str()
        .unwrap_or("")
        .contains("计划退出残量重排失败"));
    assert_eq!(backtest.planned_exits[0].exchange_quantity, 0.5);
    assert_eq!(backtest.planned_exits[0].quantity, 0.5);
    assert!(backtest.planned_exits[0].submitted);
}

#[test]
fn planned_exit_cancels_late_limit_entry_remainder_like_live() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 2.0),
        candle(2, 100.0, 101.0, 99.0, 100.0, 2.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1,
        reason: "hold_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };

    backtest.submit_intents(
        &[open_intent(
            "4",
            "limit",
            100.0,
            Vec::new(),
            Some(planned_exit),
        )],
        0,
        &market,
    );
    backtest.process_market_until(2, &market);

    assert_eq!(
        backtest.planned_exits.len(),
        1,
        "a live planned exit is keyed by the entry order identity, not by every later partial fill"
    );
    let entry_order = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Open))
        .expect("entry order should be retained for diagnostics");
    assert_eq!(entry_order.exchange_filled, 2.0);
    assert_eq!(entry_order.status, "cancelled");
    assert!(entry_order.error_message.contains("计划退出到期"));
    let close_orders = backtest
        .orders
        .iter()
        .filter(|order| matches!(order.action, OrderAction::Close))
        .collect::<Vec<_>>();
    assert_eq!(close_orders.len(), 1);
    assert_eq!(close_orders[0].exchange_quantity, 2.0);
    let close_fills = backtest
        .fills
        .iter()
        .filter(|fill| fill["action"].as_str() == Some("close_position"))
        .collect::<Vec<_>>();
    assert_eq!(close_fills.len(), 1);
    assert_eq!(close_fills[0]["size"].as_f64(), Some(2.0));
    assert!(backtest.positions.is_empty());
}

#[test]
fn planned_exit_does_not_cancel_non_resting_entry_remainder_like_live() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 2.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 2,
        reason: "hold_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };

    backtest.submit_intents(
        &[open_intent(
            "4",
            "limit",
            100.0,
            Vec::new(),
            Some(planned_exit),
        )],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    backtest.orders[0].order_type = "market".to_string();

    backtest.process_market_until(2, &market);

    let entry_order = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Open))
        .expect("entry order should be retained for diagnostics");
    assert_eq!(entry_order.exchange_filled, 2.0);
    assert_eq!(entry_order.exchange_remaining(), 2.0);
    assert_eq!(entry_order.status, "partially_filled");
    let close_orders = backtest
        .orders
        .iter()
        .filter(|order| matches!(order.action, OrderAction::Close))
        .collect::<Vec<_>>();
    assert_eq!(close_orders.len(), 1);
    assert_eq!(close_orders[0].exchange_quantity, 2.0);
}

#[test]
fn finish_exposes_runtime_action_summary_for_diagnostics() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(2, 105.0, 106.0, 104.0, 105.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let planned_exit = StrategyPlannedExitIntent {
        timestamp: 1,
        reason: "hold_elapsed".to_string(),
        contract: "planned_exit_time_v1".to_string(),
    };

    backtest.record_strategy_step(
        &[json!({
            "action": "open_position",
            "symbol": SYMBOL,
            "planned_exit_time": 1,
        })],
        &[],
        &[],
        json!({}),
        json!({}),
    );
    backtest.submit_intents(
        &[open_intent(
            "1",
            "market",
            100.0,
            Vec::new(),
            Some(planned_exit),
        )],
        0,
        &market,
    );
    backtest.process_market_until(2, &market);

    let report = backtest.finish(&market, json!({}));
    let summary = &report.detail["runtime_action_summary"];
    assert_eq!(summary["open_position"].as_u64(), Some(1));
    assert_eq!(summary["total"].as_u64(), Some(1));
    assert_eq!(summary["open_action_count"].as_u64(), Some(1));
    assert_eq!(summary["open_actions_with_planned_exit"].as_u64(), Some(1));
    assert_eq!(
        summary["planned_exit_contract"].as_str(),
        Some("planned_exit_complete")
    );
    assert_eq!(summary["planned_exit_coverage_pct"].as_f64(), Some(100.0));
    assert_eq!(summary["planned_close_count"].as_u64(), Some(1));
    assert_eq!(summary["risk_close_count"].as_u64(), Some(0));
    assert_eq!(summary["warnings"].as_array().map(Vec::len), Some(0));
    let assumptions = &report.detail["simulation_assumptions"];
    assert_eq!(
        assumptions["limit_order_fill"].as_str(),
        Some("marketable_limits_fill_at_open_like_price_capped_by_limit_else_resting_touch")
    );
    assert_eq!(
        assumptions["ioc_fok_limit_fill"].as_str(),
        Some("submission_candle_open_or_cancel")
    );
}

#[test]
fn attached_risk_trigger_must_match_tick_size_before_open() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
        "tickSz": 0.1,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let risk = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: Some(94.005),
        stop_loss: None,
        take_profit: None,
        reason: "off_tick_attached_stop".to_string(),
    };

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, vec![risk], None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.orders.len(), 1);
    assert_eq!(backtest.orders[0].status, "rejected");
    assert!(backtest.orders[0].error_message.contains("保护单触发价"));
    assert!(backtest.orders[0].error_message.contains("tickSz"));
    assert!(backtest.fills.is_empty());
    assert!(backtest.positions.is_empty());
}

#[test]
fn attached_ratio_risk_trigger_must_match_tick_size_like_live() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
        "tickSz": 0.0001,
    }));
    let candles = vec![candle(1, 0.8854, 0.8860, 0.8850, 0.8854, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let risk = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: None,
        stop_loss: Some(0.045),
        take_profit: None,
        reason: "ratio_stop_loss".to_string(),
    };

    backtest.submit_intents(
        &[open_intent("1", "market", 0.8854, vec![risk], None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.orders.len(), 1);
    assert_eq!(backtest.orders[0].status, "rejected");
    assert!(backtest.orders[0].error_message.contains("保护单触发价"));
    assert!(backtest.orders[0].error_message.contains("tickSz"));
    assert!(backtest.fills.is_empty());
    assert!(backtest.positions.is_empty());
}

#[test]
fn attached_same_kind_risk_orders_are_preserved_like_live() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
        "tickSz": 0.1,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let first_take_profit = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "take_profit".to_string(),
        trigger_price: Some(110.0),
        stop_loss: None,
        take_profit: None,
        reason: "first_take_profit".to_string(),
    };
    let second_take_profit = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "take_profit".to_string(),
        trigger_price: Some(120.0),
        stop_loss: None,
        take_profit: None,
        reason: "second_take_profit".to_string(),
    };

    backtest.submit_intents(
        &[open_intent(
            "1",
            "market",
            100.0,
            vec![first_take_profit, second_take_profit],
            None,
        )],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    let risk_orders = backtest
        .orders
        .iter()
        .filter(|order| matches!(order.action, OrderAction::Risk))
        .collect::<Vec<_>>();
    assert_eq!(risk_orders.len(), 2);
    assert_eq!(risk_orders[0].trigger_price, Some(110.0));
    assert_eq!(risk_orders[0].reason, "first_take_profit");
    assert_eq!(risk_orders[1].trigger_price, Some(120.0));
    assert_eq!(risk_orders[1].reason, "second_take_profit");
}

#[test]
fn attached_risk_with_ambiguous_ratio_kind_is_rejected_like_live() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
        "tickSz": 0.1,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let risk = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "".to_string(),
        trigger_price: None,
        stop_loss: Some(0.05),
        take_profit: Some(0.10),
        reason: "ambiguous_ratio_risk".to_string(),
    };

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, vec![risk], None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.orders.len(), 1);
    assert_eq!(backtest.orders[0].status, "rejected");
    assert!(backtest.orders[0].error_message.contains("暂不支持"));
    assert!(backtest.fills.is_empty());
    assert!(backtest.positions.is_empty());
}

#[test]
fn process_market_until_scans_all_intrabar_candles_for_attached_risk_orders() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(2, 100.0, 101.0, 95.0, 100.0, 10.0),
        candle(3, 100.0, 101.0, 89.0, 100.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let risk = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: None,
        stop_loss: Some(0.10),
        take_profit: None,
        reason: "intrabar_stop_loss".to_string(),
    };

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, vec![risk], None)],
        0,
        &market,
    );
    backtest.process_market_until(3, &market);

    let risk_fill = backtest
        .fills
        .iter()
        .find(|fill| fill["action"].as_str() == Some("place_risk_order"))
        .expect("attached stop should fill on the later intrabar low");
    assert_eq!(risk_fill["timestamp"].as_i64(), Some(3));
    assert_eq!(risk_fill["price"].as_f64(), Some(90.0));
    assert!(backtest.positions.is_empty());
}

#[test]
fn simultaneous_take_profit_and_stop_loss_uses_conservative_stop_loss() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(2, 100.0, 112.0, 88.0, 100.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let take_profit = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "take_profit".to_string(),
        trigger_price: None,
        stop_loss: None,
        take_profit: Some(0.10),
        reason: "take_profit_first".to_string(),
    };
    let stop_loss = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: None,
        stop_loss: Some(0.10),
        take_profit: None,
        reason: "stop_loss_second".to_string(),
    };

    backtest.submit_intents(
        &[open_intent(
            "1",
            "market",
            100.0,
            vec![take_profit, stop_loss],
            None,
        )],
        0,
        &market,
    );
    backtest.process_market_until(2, &market);

    let risk_fills = backtest
        .fills
        .iter()
        .filter(|fill| fill["action"].as_str() == Some("place_risk_order"))
        .collect::<Vec<_>>();
    assert_eq!(risk_fills.len(), 1);
    assert_eq!(risk_fills[0]["timestamp"].as_i64(), Some(2));
    assert_eq!(risk_fills[0]["price"].as_f64(), Some(90.0));
    assert_eq!(risk_fills[0]["pnl"].as_f64(), Some(-10.0));
    assert!(backtest.positions.is_empty());
}

#[test]
fn duplicate_reduce_only_close_orders_do_not_overfill_flat_position() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(2, 105.0, 106.0, 104.0, 105.0, 10.0),
        candle(3, 110.0, 111.0, 109.0, 110.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    let close = close_intent("sell", 105.0);
    backtest.submit_intents(&[close.clone(), close], 2, &market);
    backtest.process_market_until(3, &market);

    let close_fills = backtest
        .fills
        .iter()
        .filter(|fill| fill["action"].as_str() == Some("close_position"))
        .count();
    let cancelled_closes = backtest
        .orders
        .iter()
        .filter(|order| matches!(order.action, OrderAction::Close) && order.status == "cancelled")
        .count();
    assert_eq!(close_fills, 1);
    assert_eq!(cancelled_closes, 1);
    assert!(backtest.positions.is_empty());
}

#[test]
fn rejected_close_order_keeps_close_action_identity() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(&[close_intent("sell", 100.0)], 0, &market);

    let order = backtest
        .orders
        .last()
        .expect("rejected close should be recorded");
    assert!(matches!(order.action, OrderAction::Close));
    assert_eq!(order.side, "sell");
    assert_eq!(order.pos_side, "long");
    assert_eq!(order.status, "rejected");
    assert_eq!(order.price, None);
    assert!(super::values::order_json(order)["price"].is_null());
    let rejected = backtest
        .rejected_orders
        .last()
        .expect("rejected close context should be recorded");
    assert_eq!(rejected["action"].as_str(), Some("close_position"));
    assert!(rejected["price"].is_null());
    assert_eq!(rejected["submitted_ts"].as_i64(), Some(0));
    assert_eq!(rejected["updated_ts"].as_i64(), Some(0));
}

#[test]
fn rejected_limit_order_context_preserves_normalized_price_fields() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let mut open = open_intent("1", "post-only", 100.0, Vec::new(), None);
    open.action_record.side = "invalid-side".to_string();

    backtest.submit_intents(&[open], 0, &market);

    let order = backtest
        .orders
        .last()
        .expect("rejected limit order should be recorded");
    assert_eq!(order.status, "rejected");
    assert_eq!(order.order_type, "post_only");
    assert_eq!(order.price, Some(100.0));

    let rejected = backtest
        .rejected_orders
        .last()
        .expect("rejected limit context should be recorded");
    assert_eq!(rejected["order_type"].as_str(), Some("post_only"));
    assert_eq!(rejected["price"].as_f64(), Some(100.0));
    assert_eq!(rejected["submitted_ts"].as_i64(), Some(0));
    assert_eq!(rejected["updated_ts"].as_i64(), Some(0));
}

#[test]
fn processing_rejection_reuses_order_context_fields() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 0.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "limit", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    let order = backtest
        .orders
        .last()
        .expect("processing-rejected order should be retained");
    assert_eq!(order.status, "rejected");
    assert_eq!(order.price, Some(100.0));
    assert_eq!(order.submitted_ts, 0);
    assert_eq!(order.last_processed_ts, 1);

    let rejected = backtest
        .rejected_orders
        .last()
        .expect("processing rejection context should be recorded");
    assert_eq!(rejected["action"].as_str(), Some("open_position"));
    assert_eq!(rejected["order_type"].as_str(), Some("limit"));
    assert_eq!(rejected["price"].as_f64(), Some(100.0));
    assert_eq!(rejected["submitted_ts"].as_i64(), Some(0));
    assert_eq!(rejected["timestamp"].as_i64(), Some(1));
    assert_eq!(rejected["updated_ts"].as_i64(), Some(1));
    assert_eq!(rejected["created_ts"].as_i64(), Some(0));
}

#[test]
fn market_close_without_reference_price_uses_position_quantity_like_live() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(2, 105.0, 106.0, 104.0, 105.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    backtest.submit_intents(&[close_intent("sell", 0.0)], 1, &market);
    backtest.process_market_until(2, &market);

    let close_order = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Close))
        .expect("market close should be submitted even without reference price");
    assert_eq!(close_order.status, "filled");
    assert_eq!(close_order.price, None);
    assert_eq!(close_order.reference_price, 0.0);
    let close_fill = backtest
        .fills
        .iter()
        .find(|fill| fill["action"].as_str() == Some("close_position"))
        .expect("market close should fill from historical candle");
    assert_eq!(close_fill["price"].as_f64(), Some(105.0));
    assert_eq!(close_fill["size"].as_f64(), Some(1.0));
    assert!(backtest.positions.is_empty());
    assert!(backtest.rejected_orders.is_empty());
}

#[test]
fn explicit_market_close_size_without_reference_price_is_accepted_like_live() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(2, 105.0, 106.0, 104.0, 105.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("2", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    let mut close = close_intent("sell", 0.0);
    close.exchange_size = Some("1".to_string());
    backtest.submit_intents(&[close], 1, &market);
    backtest.process_market_until(2, &market);

    let close_fill = backtest
        .fills
        .iter()
        .find(|fill| fill["action"].as_str() == Some("close_position"))
        .expect("explicit market close should fill from historical candle");
    assert_eq!(close_fill["size"].as_f64(), Some(1.0));
    let position = backtest
        .positions
        .values()
        .next()
        .expect("partial close should keep residual position");
    assert_eq!(position.exchange_quantity, 1.0);
    assert_eq!(position.quantity, 1.0);
    assert!(backtest.rejected_orders.is_empty());
}

#[test]
fn modify_reduce_only_quote_ctval_new_size_without_reference_price_uses_position_like_live() {
    let config = test_config(json!({
        "ctVal": 10.0,
        "ctValCcy": "USDT",
        "lotSz": 1.0,
        "minSz": 1.0,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        swap_candle(1, 100.0, 101.0, 99.0, 100.0, 100.0, 100.0),
        swap_candle(2, 200.0, 201.0, 199.0, 200.0, 100.0, 100.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("10", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    backtest.submit_intents(&[close_intent("sell", 0.0)], 1, &market);

    let close_order = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Close))
        .expect("market close order should exist")
        .clone();
    assert_eq!(close_order.price, None);
    assert_eq!(close_order.reference_price, 0.0);

    backtest.submit_intents(
        &[modify_intent(&close_order, Some("5"), None, false)],
        1,
        &market,
    );

    let close_order = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Close))
        .expect("market close order should still exist");
    assert_eq!(close_order.status, "open");
    assert_eq!(close_order.exchange_quantity, 5.0);
    assert_eq!(close_order.quantity, 0.5);
    assert!(backtest.rejected_orders.is_empty());

    backtest.process_market_until(2, &market);

    let close_fill = backtest
        .fills
        .iter()
        .find(|fill| fill["action"].as_str() == Some("close_position"))
        .expect("modified market close should fill from historical candle");
    assert_eq!(close_fill["price"].as_f64(), Some(200.0));
    assert_eq!(close_fill["size"].as_f64(), Some(5.0));
    assert_eq!(close_fill["base_size"].as_f64(), Some(0.5));
    let position = backtest
        .positions
        .values()
        .next()
        .expect("partial close should keep residual quote-ctVal position");
    assert_eq!(position.exchange_quantity, 5.0);
    assert_eq!(position.quantity, 0.5);
    assert!(backtest.rejected_orders.is_empty());
}

#[test]
fn contract_close_fee_is_charged_once() {
    let config = test_config(json!({
        "historical_fee_rate": 0.01,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(2, 110.0, 111.0, 109.0, 110.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    backtest.submit_intents(&[close_intent("sell", 110.0)], 1, &market);
    backtest.process_market_until(2, &market);

    let expected_cash = 10_000.0 - 1.0 + 10.0 - 1.1;
    assert!((backtest.cash - expected_cash).abs() < 1e-9);
}

#[test]
fn quote_ctval_swap_close_pnl_uses_open_position_exposure() {
    let config = test_config(json!({
        "ctVal": 10.0,
        "ctValCcy": "USDT",
        "lotSz": 1.0,
        "minSz": 1.0,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        swap_candle(1, 100.0, 101.0, 99.0, 100.0, 100.0, 100.0),
        swap_candle(2, 200.0, 201.0, 199.0, 200.0, 100.0, 100.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("10", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    let position = backtest
        .positions
        .values()
        .next()
        .expect("position should open");
    assert_eq!(position.exchange_quantity, 10.0);
    assert_eq!(position.quantity, 1.0);
    assert_eq!(position.entry_price, 100.0);

    backtest.submit_intents(&[close_intent("sell", 200.0)], 1, &market);
    backtest.process_market_until(2, &market);

    let close_fill = backtest
        .fills
        .iter()
        .find(|fill| fill["action"].as_str() == Some("close_position"))
        .expect("close fill should be recorded");
    assert_eq!(close_fill["size"].as_f64(), Some(10.0));
    assert_eq!(close_fill["base_size"].as_f64(), Some(1.0));
    assert_eq!(close_fill["value"].as_f64(), Some(100.0));
    assert_eq!(close_fill["pnl"].as_f64(), Some(100.0));
    assert_eq!(
        backtest.trade_records.last().map(|trade| trade.quantity),
        Some(1.0)
    );
    assert_eq!(
        backtest
            .trade_records
            .last()
            .map(|trade| trade.exchange_quantity),
        Some(10.0)
    );
    assert_eq!(
        backtest.trade_records.last().and_then(|trade| trade.pnl),
        Some(100.0)
    );
    assert!((backtest.cash - 10_100.0).abs() < 1e-9);
    assert!(backtest.positions.is_empty());
}

#[test]
fn orders_context_rows_expose_live_like_quantity_fields() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );

    let open_context = backtest.orders_context();
    assert_eq!(open_context["source"], "historical_live_backtest");
    assert_eq!(open_context["mode"], "historical_sim");
    let open = &open_context["open"][0];
    assert_eq!(open["source"], "historical_live_backtest");
    assert_eq!(open["mode"], "historical_sim");
    assert_eq!(open["quantity"].as_f64(), Some(1.0));
    assert_eq!(open["size"].as_f64(), Some(1.0));
    assert_eq!(open["base_quantity"].as_f64(), Some(1.0));

    backtest.process_market_until(1, &market);

    let fill_context = backtest.orders_context();
    let fill = &fill_context["recent_fills"][0];
    assert_eq!(fill["source"], "historical_live_backtest");
    assert_eq!(fill["mode"], "historical_sim");
    assert_eq!(fill["inst_id"].as_str(), Some(SYMBOL));
    assert_eq!(fill["status"], "filled");
    assert_eq!(fill["success"], true);
    assert_eq!(fill["quantity"].as_f64(), Some(1.0));
    assert_eq!(fill["filled_quantity"].as_f64(), Some(1.0));
    assert_eq!(fill["base_quantity"].as_f64(), Some(1.0));
    assert_eq!(fill["fee"].as_f64(), Some(0.0));
}

#[test]
fn report_equity_curve_preserves_position_snapshots_after_fill() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    backtest.record_equity(&candles[0], &market);

    let report = backtest.finish(&market, json!({}));
    let snapshot = &report.detail["equity_curve"][0];
    let position = &snapshot["positions"][0];

    assert_eq!(snapshot["position_count"].as_u64(), Some(1));
    assert_eq!(snapshot["position_side"].as_str(), Some("long"));
    assert_eq!(position["symbol"].as_str(), Some(SYMBOL));
    assert_eq!(position["side"].as_str(), Some("long"));
    assert_eq!(position["entry_price"].as_f64(), Some(100.0));
    assert_eq!(position["quantity"].as_f64(), Some(1.0));
    assert_eq!(position["exchange_quantity"].as_f64(), Some(1.0));
    assert_eq!(
        position["mark_price_source"].as_str(),
        Some("historical_last_close")
    );
}

#[test]
fn account_context_exposes_live_like_equity_aliases() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    let account = backtest.account_context(1, &market);

    assert_eq!(account["source"], "historical_live_backtest");
    assert_eq!(account["mode"], "historical_sim");
    assert_eq!(account["equity"].as_f64(), Some(10_000.0));
    assert_eq!(account["total_equity"].as_f64(), Some(10_000.0));
    assert_eq!(account["total_eq"].as_f64(), Some(10_000.0));
    assert_eq!(account["usdt_balance"].as_f64(), Some(10_000.0));
    assert_eq!(account["usdt_available"].as_f64(), Some(10_000.0));
    assert_eq!(account["usdt_equity_usd"].as_f64(), Some(10_000.0));
    assert_eq!(account["details"][0]["ccy"].as_str(), Some("USDT"));
}

#[test]
fn quote_ctval_swap_context_notional_uses_contract_value_not_mark_base() {
    let config = test_config(json!({
        "ctVal": 10.0,
        "ctValCcy": "USDT",
        "lotSz": 1.0,
        "minSz": 1.0,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        swap_candle(1, 100.0, 101.0, 99.0, 100.0, 100.0, 100.0),
        swap_candle(2, 200.0, 201.0, 199.0, 200.0, 100.0, 100.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("10", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    let positions = backtest.positions_context(2, &market);
    let row = &positions["open"][0];
    assert_eq!(row["pos"].as_f64(), Some(10.0));
    assert_eq!(row["basePos"].as_f64(), Some(1.0));
    assert_eq!(row["markPx"].as_f64(), Some(200.0));
    assert_eq!(row["notionalUsd"].as_f64(), Some(100.0));
    assert_eq!(row["upl"].as_f64(), Some(100.0));

    let account = backtest.account_context(2, &market);
    assert_eq!(account["equity"].as_f64(), Some(10_100.0));
    assert_eq!(account["margin_used"].as_f64(), Some(100.0));
    assert_eq!(account["available_equity"].as_f64(), Some(10_000.0));
}

#[test]
fn missing_mark_price_is_exposed_in_positions_and_runtime_summary() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(2, 105.0, 106.0, 104.0, 105.0, 10.0),
    ];
    let eth_market = market_for_symbol("ETH-USDT-SWAP", &candles);
    let btc_only_market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let open_eth = intent_for_symbol(
        "ETH-USDT-SWAP",
        StrategyIntentAction::OpenPosition,
        "long",
        "market",
        100.0,
        None,
        Some("1".to_string()),
        Vec::new(),
        None,
    );

    backtest.submit_intents(&[open_eth], 0, &eth_market);
    backtest.process_market_until(1, &eth_market);

    let positions = backtest.positions_context(2, &btc_only_market);
    let row = &positions["open"][0];
    assert_eq!(row["symbol"].as_str(), Some("ETH-USDT-SWAP"));
    assert_eq!(row["mark_price"].as_f64(), Some(100.0));
    assert_eq!(row["mark_price_missing"].as_bool(), Some(true));
    assert_eq!(
        row["mark_price_source"].as_str(),
        Some("entry_price_fallback")
    );

    let report = backtest.finish(&btc_only_market, json!({}));
    let summary = &report.detail["runtime_action_summary"];
    assert_eq!(
        summary["open_positions_missing_mark_count"].as_u64(),
        Some(1)
    );
    assert!(summary["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str() == Some("open_positions_missing_mark_price")));
}

#[test]
fn swap_explicit_exchange_size_converts_contracts_to_base_quantity() {
    let config = test_config(json!({
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 0.01,
        "minSz": 0.01,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![swap_candle(1, 100.0, 101.0, 99.0, 100.0, 100.0, 1.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    let position = backtest
        .positions
        .values()
        .next()
        .expect("position should open");
    assert!((position.exchange_quantity - 1.0).abs() < 1e-12);
    assert!((position.quantity - 0.01).abs() < 1e-12);

    let context = backtest.positions_context(1, &market);
    let row = &context["open"][0];
    assert_eq!(row["pos"].as_f64(), Some(1.0));
    assert_eq!(row["basePos"].as_f64(), Some(0.01));
    assert_eq!(row["notionalUsd"].as_f64(), Some(1.0));
    let keyed = &context["BTC-USDT-SWAP"];
    assert_eq!(keyed["source"].as_str(), Some("historical_live_backtest"));
    assert_eq!(keyed["inst_id"].as_str(), Some("BTC-USDT-SWAP"));
    assert_eq!(keyed["inst_type"].as_str(), Some("SWAP"));
    assert_eq!(keyed["side"].as_str(), Some("long"));
    assert_eq!(keyed["pos_side"].as_str(), Some("long"));
    assert_eq!(keyed["quantity"].as_f64(), Some(1.0));
    assert_eq!(keyed["entry_price"].as_f64(), Some(100.0));
    assert_eq!(keyed["mark_price"].as_f64(), Some(100.0));
    assert_eq!(keyed["unrealized_pnl"].as_f64(), Some(0.0));
    let side_keyed = &context["BTC-USDT-SWAP:long"];
    assert_eq!(
        side_keyed["source"].as_str(),
        Some("historical_live_backtest")
    );
    assert_eq!(side_keyed["pos_side"].as_str(), Some("long"));
    assert_eq!(side_keyed["quantity"].as_f64(), Some(1.0));
}

#[test]
fn swap_intent_uses_contract_units_when_strategy_default_inst_type_is_spot() {
    let mut config = test_config(json!({
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 1.0,
        "minSz": 1.0,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    config.inst_type = "SPOT".to_string();
    let candles = vec![swap_candle(1, 100.0, 101.0, 99.0, 100.0, 1.0, 0.01)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.orders[0].inst_type, "SWAP");
    assert_eq!(backtest.orders[0].exchange_quantity, 1.0);
    assert!((backtest.orders[0].quantity - 0.01).abs() < 1e-12);
    assert_eq!(backtest.orders[0].exchange_filled, 1.0);
    assert!((backtest.orders[0].filled - 0.01).abs() < 1e-12);
    assert_eq!(backtest.fills[0]["value"].as_f64(), Some(1.0));
    let position = backtest
        .positions
        .values()
        .next()
        .expect("swap action should open a contract position");
    assert_eq!(position.inst_type, "SWAP");
    assert_eq!(position.exchange_quantity, 1.0);
    assert!((position.quantity - 0.01).abs() < 1e-12);

    let report = backtest.finish(&market, json!({}));
    let assumptions = &report.detail["simulation_assumptions"];
    assert_eq!(assumptions["contract_mode"].as_bool(), Some(true));
    assert_eq!(
        assumptions["configured_contract_mode"].as_bool(),
        Some(false)
    );
    assert_eq!(
        assumptions["order_size_unit"].as_str(),
        Some("okx_exchange_size_by_inst_type")
    );
    let cost_model = &report.detail["cost_model"];
    assert_eq!(cost_model["contract_mode"].as_bool(), Some(true));
    assert_eq!(
        cost_model["order_size_unit"].as_str(),
        Some("okx_exchange_size_by_inst_type")
    );
}

#[test]
fn multi_symbol_orders_use_symbol_specific_instrument_rules() {
    let config = test_config(json!({
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 0.01,
        "minSz": 0.01,
        "tickSz": 0.1,
        "_backtest_instrument_rules_by_symbol": {
            "BTC-USDT-SWAP": {
                "instId": "BTC-USDT-SWAP",
                "ctVal": 0.01,
                "ctValCcy": "BTC",
                "lotSz": 0.01,
                "minSz": 0.01,
                "tickSz": 0.1
            },
            "ETH-USDT-SWAP": {
                "instId": "ETH-USDT-SWAP",
                "ctVal": 0.1,
                "ctValCcy": "ETH",
                "lotSz": 0.1,
                "minSz": 0.1,
                "tickSz": 0.01
            }
        },
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let symbol = "ETH-USDT-SWAP";
    let candles = vec![swap_candle(
        1, 2000.0, 2001.0, 1999.0, 2000.0, 1000.0, 100.0,
    )];
    let market = market_for_symbol(symbol, &candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[intent_for_symbol(
            symbol,
            StrategyIntentAction::OpenPosition,
            "long",
            "market",
            2000.0,
            None,
            None,
            Vec::new(),
            None,
        )],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.orders[0].symbol, symbol);
    assert!((backtest.orders[0].exchange_quantity - 5.0).abs() < 1e-12);
    assert!((backtest.orders[0].quantity - 0.5).abs() < 1e-12);
    assert_eq!(backtest.orders[0].status, "filled");
}

#[test]
fn params_rules_reject_cross_symbol_orders_without_symbol_rules() {
    let config = test_config(json!({
        "backtest_instrument_rules_source": "params",
        "_backtest_instrument_rules_source_resolved": "params",
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 0.01,
        "minSz": 0.01,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let symbol = "ETH-USDT-SWAP";
    let candles = vec![swap_candle(
        1, 2000.0, 2001.0, 1999.0, 2000.0, 1000.0, 100.0,
    )];
    let market = market_for_symbol(symbol, &candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[intent_for_symbol(
            symbol,
            StrategyIntentAction::OpenPosition,
            "long",
            "market",
            2000.0,
            None,
            None,
            Vec::new(),
            None,
        )],
        0,
        &market,
    );

    assert_eq!(backtest.orders[0].status, "rejected");
    assert!(backtest.orders[0]
        .error_message
        .contains("缺少回测交易规格"));
    assert!(backtest.positions.is_empty());
}

#[test]
fn swap_fill_capacity_uses_volume_ccy_not_contract_volume() {
    let config = test_config(json!({
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 0.01,
        "minSz": 0.01,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 0.5,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![swap_candle(1, 100.0, 101.0, 99.0, 100.0, 100.0, 1.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("100", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.orders[0].exchange_filled, 50.0);
    assert_eq!(backtest.orders[0].filled, 0.5);
    assert_eq!(backtest.orders[0].status, "cancelled");
    assert!(backtest.orders[0].error_message.contains("立即成交"));
}

#[test]
fn swap_long_position_pays_positive_funding_once() {
    let config = test_config(json!({
        "ctVal": 1.0,
        "ctValCcy": "BTC",
        "lotSz": 1.0,
        "minSz": 1.0,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(2, 100.0, 101.0, 99.0, 100.0, 10.0),
    ];
    let market = market_with_funding(&candles, &[(1, 0.10), (2, 0.001)]);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    backtest.process_market_until(2, &market);
    backtest.process_market_until(2, &market);

    assert_eq!(backtest.funding_events.len(), 1);
    assert!((backtest.total_funding + 0.1).abs() < 1e-12);
    assert!((backtest.cash - 9_999.9).abs() < 1e-12);
    let funding_trade = backtest
        .trade_records
        .iter()
        .find(|trade| trade.action.as_deref() == Some("funding"))
        .expect("funding should be recorded as a trade event");
    assert_eq!(funding_trade.timestamp, 2);
    assert!((funding_trade.funding + 0.1).abs() < 1e-12);
}

#[test]
fn funding_mark_price_fallback_is_exposed_in_event_and_runtime_summary() {
    let config = test_config(json!({
        "ctVal": 1.0,
        "ctValCcy": "BTC",
        "lotSz": 1.0,
        "minSz": 1.0,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let eth_market = market_for_symbol("ETH-USDT-SWAP", &candles);
    let funding_market = market(&candles).with_funding(vec![HistoricalFundingSeries {
        symbol: "ETH-USDT-SWAP".to_string(),
        inst_type: INST_TYPE.to_string(),
        rates: vec![HistoricalFundingPoint {
            symbol: "ETH-USDT-SWAP".to_string(),
            inst_type: INST_TYPE.to_string(),
            funding_time: 2,
            funding_rate: 0.001,
        }],
    }]);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let open_eth = intent_for_symbol(
        "ETH-USDT-SWAP",
        StrategyIntentAction::OpenPosition,
        "long",
        "market",
        100.0,
        None,
        Some("1".to_string()),
        Vec::new(),
        None,
    );

    backtest.submit_intents(&[open_eth], 0, &eth_market);
    backtest.process_market_until(1, &eth_market);
    backtest.process_market_until(2, &funding_market);

    assert_eq!(backtest.funding_events.len(), 1);
    let event = &backtest.funding_events[0];
    assert_eq!(event["mark_price"].as_f64(), Some(100.0));
    assert_eq!(event["mark_price_missing"].as_bool(), Some(true));
    assert_eq!(
        event["mark_price_source"].as_str(),
        Some("entry_price_fallback")
    );

    let report = backtest.finish(&funding_market, json!({}));
    let summary = &report.detail["runtime_action_summary"];
    assert_eq!(
        summary["funding_mark_price_fallback_count"].as_u64(),
        Some(1)
    );
    assert!(summary["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str() == Some("funding_mark_price_fallback")));
    assert_eq!(
        report.detail["simulation_assumptions"]["funding_mark_price_fallback_count"].as_u64(),
        Some(1)
    );
}

#[test]
fn swap_short_position_receives_positive_funding() {
    let config = test_config(json!({
        "ctVal": 1.0,
        "ctValCcy": "BTC",
        "lotSz": 1.0,
        "minSz": 1.0,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(2, 100.0, 101.0, 99.0, 100.0, 10.0),
    ];
    let market = market_with_funding(&candles, &[(2, 0.001)]);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[intent(
            StrategyIntentAction::OpenPosition,
            "short",
            "market",
            100.0,
            None,
            Some("1".to_string()),
            Vec::new(),
            None,
        )],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    backtest.process_market_until(2, &market);

    assert_eq!(backtest.funding_events.len(), 1);
    assert!((backtest.total_funding - 0.1).abs() < 1e-12);
    assert!((backtest.cash - 10_000.1).abs() < 1e-12);
}

#[test]
fn explicit_swap_exchange_size_must_match_lot_size() {
    let config = test_config(json!({
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 0.01,
        "minSz": 0.01,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![swap_candle(1, 100.0, 101.0, 99.0, 100.0, 100.0, 1.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("0.005", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );

    assert_eq!(backtest.rejected_orders.len(), 1);
    assert!(backtest.rejected_orders[0]["error_message"]
        .as_str()
        .unwrap_or("")
        .contains("lotSz"));
    assert!(backtest.positions.is_empty());
}

#[test]
fn standalone_risk_order_uses_explicit_exchange_size() {
    let config = test_config(json!({
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 0.01,
        "minSz": 0.01,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        swap_candle(1, 100.0, 101.0, 99.0, 100.0, 100.0, 1.0),
        swap_candle(2, 100.0, 101.0, 99.0, 100.0, 100.0, 1.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("100", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    backtest.submit_intents(&[risk_intent("25", 95.0)], 1, &market);

    let risk_order = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Risk))
        .expect("risk order should be submitted");
    assert_eq!(risk_order.exchange_quantity, 25.0);
    assert_eq!(risk_order.quantity, 0.25);
    assert_eq!(risk_order.order_type, "stop_loss");
    assert_eq!(risk_order.status, "open");
}

#[test]
fn malformed_risk_order_without_kind_is_rejected_instead_of_defaulting_stop_loss() {
    let config = test_config(json!({
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 0.01,
        "minSz": 0.01,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        swap_candle(1, 100.0, 101.0, 99.0, 100.0, 100.0, 1.0),
        swap_candle(2, 100.0, 101.0, 94.0, 100.0, 100.0, 1.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("100", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    backtest.submit_intents(&[risk_intent("25", 95.0)], 1, &market);

    let risk_order = backtest
        .orders
        .iter_mut()
        .find(|order| matches!(order.action, OrderAction::Risk))
        .expect("risk order should be submitted");
    risk_order.risk_kind = None;

    backtest.process_market_until(2, &market);

    let risk_order = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Risk))
        .expect("risk order should still be visible after rejection");
    assert_eq!(risk_order.status, "rejected");
    assert!(risk_order.error_message.contains("保护单缺少风险类型"));
    assert!(backtest
        .fills
        .iter()
        .all(|fill| fill["action"].as_str() != Some("place_risk_order")));
    assert_eq!(backtest.rejected_orders.len(), 1);
    assert!(backtest.rejected_orders[0]["error_message"]
        .as_str()
        .unwrap_or("")
        .contains("保护单缺少风险类型"));
    assert!(!backtest.positions.is_empty());
}

#[test]
fn standalone_risk_without_reference_price_uses_position_quantity_like_live() {
    let config = test_config(json!({
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 0.01,
        "minSz": 0.01,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        swap_candle(1, 100.0, 101.0, 99.0, 100.0, 100.0, 1.0),
        swap_candle(2, 100.0, 101.0, 99.0, 100.0, 100.0, 1.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("100", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    let mut risk = risk_intent("25", 95.0);
    risk.action_record.price = 0.0;
    risk.exchange_size = None;
    backtest.submit_intents(&[risk], 1, &market);

    let risk_order = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Risk))
        .expect("risk order should be submitted even without reference price");
    assert_eq!(risk_order.exchange_quantity, 100.0);
    assert_eq!(risk_order.quantity, 1.0);
    assert_eq!(risk_order.reference_price, 100.0);
    let risk_order_json = super::values::order_json(risk_order);
    assert_eq!(risk_order_json["reference_price"].as_f64(), Some(100.0));
    assert_eq!(
        risk_order_json["reference_price_source"].as_str(),
        Some("position_entry_price")
    );
    assert_eq!(
        risk_order_json["reference_price_missing"].as_bool(),
        Some(false)
    );
    assert_eq!(risk_order.status, "open");
    assert!(backtest.rejected_orders.is_empty());
}

#[test]
fn standalone_risk_trigger_direction_must_protect_position() {
    let config = test_config(json!({
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 0.01,
        "minSz": 0.01,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        swap_candle(1, 100.0, 101.0, 99.0, 100.0, 100.0, 1.0),
        swap_candle(2, 100.0, 101.0, 99.0, 100.0, 100.0, 1.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("100", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    backtest.submit_intents(&[risk_intent("25", 105.0)], 1, &market);

    assert_eq!(
        backtest.orders.last().map(|order| order.status.as_str()),
        Some("rejected")
    );
    assert!(matches!(
        backtest.orders.last().map(|order| order.action),
        Some(OrderAction::Risk)
    ));
    assert_eq!(
        backtest.orders.last().map(|order| order.pos_side.as_str()),
        Some("long")
    );
    assert_eq!(
        backtest
            .rejected_orders
            .last()
            .and_then(|item| item["action"].as_str()),
        Some("place_risk_order")
    );
    assert!(backtest
        .orders
        .last()
        .map(|order| order.error_message.contains("触发价方向无效"))
        .unwrap_or(false));
    assert!(backtest
        .orders
        .iter()
        .filter(|order| matches!(order.action, OrderAction::Risk))
        .all(|order| order.status != "open"));
}

#[test]
fn modify_risk_order_new_price_updates_trigger_price() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        candle(1, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(2, 100.0, 101.0, 94.0, 100.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let risk = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: Some(90.0),
        stop_loss: None,
        take_profit: None,
        reason: "modifiable_stop".to_string(),
    };

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, vec![risk], None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    let risk_order = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Risk))
        .expect("attached risk order should exist")
        .clone();

    backtest.submit_intents(
        &[modify_intent(&risk_order, None, Some("95.0"), false)],
        1,
        &market,
    );
    assert_eq!(attached_risk_trigger(&backtest), Some(95.0));

    backtest.process_market_until(2, &market);

    let risk_fill = backtest
        .fills
        .iter()
        .find(|fill| fill["action"].as_str() == Some("place_risk_order"))
        .expect("modified stop should trigger at the new price");
    assert_eq!(risk_fill["timestamp"].as_i64(), Some(2));
    assert_eq!(risk_fill["price"].as_f64(), Some(95.0));
    assert!(backtest.positions.is_empty());
}

#[test]
fn modify_risk_order_new_price_must_match_tick_size_like_live() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
        "tickSz": 0.1,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let risk = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: Some(90.0),
        stop_loss: None,
        take_profit: None,
        reason: "modifiable_stop".to_string(),
    };

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, vec![risk], None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    let risk_order = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Risk))
        .expect("attached risk order should exist")
        .clone();

    backtest.submit_intents(
        &[modify_intent(&risk_order, None, Some("95.05"), false)],
        1,
        &market,
    );

    let risk_order = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Risk))
        .expect("risk order should remain");
    assert_eq!(risk_order.status, "open");
    assert_eq!(risk_order.trigger_price, Some(90.0));
    assert!(risk_order.error_message.contains("tickSz"));
    let rejection = backtest
        .rejected_orders
        .last()
        .expect("invalid risk amend should be rejected");
    assert_eq!(rejection["action"].as_str(), Some("modify_order"));
    assert!(rejection["error_message"]
        .as_str()
        .unwrap_or_default()
        .contains("tickSz"));
}

#[test]
fn spot_standalone_ratio_risk_without_trigger_is_rejected_like_live() {
    let symbol = "BTC-USDT";
    let mut config = test_config(json!({
        "contract_mode": false,
        "lotSz": 0.01,
        "minSz": 0.01,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    config.symbol = symbol.to_string();
    config.inst_type = "SPOT".to_string();
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market_for_symbol(symbol, &candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let mut open = intent_for_symbol(
        symbol,
        StrategyIntentAction::OpenPosition,
        "long",
        "market",
        100.0,
        None,
        Some("1".to_string()),
        Vec::new(),
        None,
    );
    open.inst_type = "SPOT".to_string();

    backtest.submit_intents(&[open], 0, &market);
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.orders[0].status, "filled");
    let risk = StrategyRiskOrderIntent {
        symbol: symbol.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: None,
        stop_loss: Some(0.05),
        take_profit: None,
        reason: "spot_ratio_stop_without_average".to_string(),
    };
    let mut risk_intent = intent_for_symbol(
        symbol,
        StrategyIntentAction::PlaceRiskOrder,
        "long",
        "market",
        100.0,
        Some("sell".to_string()),
        Some("1".to_string()),
        vec![risk],
        None,
    );
    risk_intent.inst_type = "SPOT".to_string();

    backtest.submit_intents(&[risk_intent], 1, &market);

    let rejected = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Risk))
        .expect("risk intent should be recorded as rejected");
    assert_eq!(rejected.status, "rejected");
    assert!(rejected.error_message.contains("独立保护单缺少有效触发价"));
    assert_eq!(
        backtest
            .orders
            .iter()
            .filter(|order| matches!(order.action, OrderAction::Risk) && order.is_open())
            .count(),
        0
    );
}

#[test]
fn cancel_order_target_kind_exchange_does_not_cancel_risk_order() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let risk = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: Some(90.0),
        stop_loss: None,
        take_profit: None,
        reason: "target_kind_stop".to_string(),
    };

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, vec![risk], None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    let risk_order = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Risk))
        .expect("attached risk order should exist")
        .clone();
    let mut exchange_cancel =
        cancel_missing_intent(&risk_order.order_id, &risk_order.client_order_id);
    exchange_cancel.cancel_order.as_mut().unwrap().target_kind = StrategyOrderTargetKind::Exchange;

    backtest.submit_intents(&[exchange_cancel], 1, &market);

    let current_risk = backtest
        .orders
        .iter()
        .find(|order| order.order_id == risk_order.order_id)
        .expect("risk order should remain");
    assert_eq!(current_risk.status, "open");
    assert_eq!(
        backtest
            .rejected_orders
            .last()
            .and_then(|row| row["target_order_kind"].as_str()),
        Some("exchange")
    );

    let mut algo_cancel = cancel_missing_intent(&risk_order.order_id, &risk_order.client_order_id);
    algo_cancel.cancel_order.as_mut().unwrap().target_kind = StrategyOrderTargetKind::Algo;
    backtest.submit_intents(&[algo_cancel], 1, &market);

    let current_risk = backtest
        .orders
        .iter()
        .find(|order| order.order_id == risk_order.order_id)
        .expect("risk order should still exist");
    assert_eq!(current_risk.status, "cancelled");
}

#[test]
fn modify_order_target_kind_exchange_does_not_modify_risk_order() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let risk = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: Some(90.0),
        stop_loss: None,
        take_profit: None,
        reason: "target_kind_stop".to_string(),
    };

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, vec![risk], None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    let risk_order = backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Risk))
        .expect("attached risk order should exist")
        .clone();
    let mut exchange_modify = modify_intent(&risk_order, None, Some("95.0"), false);
    exchange_modify.modify_order.as_mut().unwrap().target_kind = StrategyOrderTargetKind::Exchange;

    backtest.submit_intents(&[exchange_modify], 1, &market);

    assert_eq!(attached_risk_trigger(&backtest), Some(90.0));
    assert_eq!(
        backtest
            .rejected_orders
            .last()
            .and_then(|row| row["target_order_kind"].as_str()),
        Some("exchange")
    );

    let mut algo_modify = modify_intent(&risk_order, None, Some("95.0"), false);
    algo_modify.modify_order.as_mut().unwrap().target_kind = StrategyOrderTargetKind::Algo;
    backtest.submit_intents(&[algo_modify], 1, &market);

    assert_eq!(attached_risk_trigger(&backtest), Some(95.0));
}

#[test]
fn order_management_any_target_kind_rejects_exchange_algo_collision_like_live() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
        "tickSz": 0.1,
        "lotSz": 1.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let risk = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: Some(90.0),
        stop_loss: None,
        take_profit: None,
        reason: "collision_stop".to_string(),
    };

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, vec![risk], None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    backtest.submit_intents(
        &[open_intent("1", "limit", 95.0, Vec::new(), None)],
        1,
        &market,
    );

    let risk_index = backtest
        .orders
        .iter()
        .position(|order| matches!(order.action, OrderAction::Risk) && order.is_open())
        .expect("active risk order should exist");
    let exchange_index = backtest
        .orders
        .iter()
        .position(|order| matches!(order.action, OrderAction::Open) && order.is_open())
        .expect("active exchange order should exist");
    let shared_client_order_id = "bt-collision-client";
    backtest.orders[risk_index].client_order_id = shared_client_order_id.to_string();
    backtest.orders[exchange_index].client_order_id = shared_client_order_id.to_string();

    backtest.submit_intents(
        &[cancel_missing_intent("", shared_client_order_id)],
        2,
        &market,
    );

    assert_eq!(backtest.orders[risk_index].status, "open");
    assert_eq!(backtest.orders[exchange_index].status, "open");
    let rejection = backtest
        .rejected_orders
        .last()
        .expect("cancel collision should be rejected");
    assert_eq!(rejection["action"].as_str(), Some("cancel_order"));
    assert_eq!(rejection["target_order_kind"].as_str(), Some("any"));
    assert!(rejection["error_message"]
        .as_str()
        .unwrap_or_default()
        .contains("同时命中普通订单和保护单"));

    backtest.submit_intents(
        &[modify_missing_intent("", shared_client_order_id)],
        3,
        &market,
    );

    assert_eq!(backtest.orders[risk_index].trigger_price, Some(90.0));
    assert_eq!(backtest.orders[exchange_index].price, Some(95.0));
    let rejection = backtest
        .rejected_orders
        .last()
        .expect("modify collision should be rejected");
    assert_eq!(rejection["action"].as_str(), Some("modify_order"));
    assert_eq!(rejection["target_order_kind"].as_str(), Some("any"));
    assert!(rejection["error_message"]
        .as_str()
        .unwrap_or_default()
        .contains("同时命中普通订单和保护单"));
}

#[test]
fn order_management_scope_explicit_limits_target_symbol_like_live() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = HistoricalMarketData::new(vec![
        HistoricalCandleSeries {
            symbol: SYMBOL.to_string(),
            inst_type: INST_TYPE.to_string(),
            timeframe: TIMEFRAME.to_string(),
            candles: candles.clone(),
        },
        HistoricalCandleSeries {
            symbol: "ETH-USDT-SWAP".to_string(),
            inst_type: INST_TYPE.to_string(),
            timeframe: TIMEFRAME.to_string(),
            candles: candles.clone(),
        },
    ]);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[
            open_intent("1", "limit", 95.0, Vec::new(), None),
            intent_for_symbol(
                "ETH-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "long",
                "limit",
                95.0,
                None,
                Some("1".to_string()),
                Vec::new(),
                None,
            ),
        ],
        0,
        &market,
    );

    let shared_client_order_id = "bt-scoped-client";
    for order in &mut backtest.orders {
        order.client_order_id = shared_client_order_id.to_string();
    }
    let btc_index = backtest
        .orders
        .iter()
        .position(|order| order.symbol == SYMBOL)
        .expect("BTC order should exist");
    let eth_index = backtest
        .orders
        .iter()
        .position(|order| order.symbol == "ETH-USDT-SWAP")
        .expect("ETH order should exist");
    let mut eth_cancel = cancel_missing_intent("", shared_client_order_id);
    eth_cancel.symbol = "ETH-USDT-SWAP".to_string();
    eth_cancel.cancel_order.as_mut().unwrap().scope_explicit = true;

    backtest.submit_intents(&[eth_cancel], 1, &market);

    assert_eq!(backtest.orders[btc_index].status, "open");
    assert_eq!(backtest.orders[eth_index].status, "cancelled");
    assert!(backtest.rejected_orders.is_empty());
}

#[test]
fn order_management_global_identity_rejects_cross_symbol_ambiguity_like_live() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = HistoricalMarketData::new(vec![
        HistoricalCandleSeries {
            symbol: SYMBOL.to_string(),
            inst_type: INST_TYPE.to_string(),
            timeframe: TIMEFRAME.to_string(),
            candles: candles.clone(),
        },
        HistoricalCandleSeries {
            symbol: "ETH-USDT-SWAP".to_string(),
            inst_type: INST_TYPE.to_string(),
            timeframe: TIMEFRAME.to_string(),
            candles: candles.clone(),
        },
    ]);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[
            open_intent("1", "limit", 95.0, Vec::new(), None),
            intent_for_symbol(
                "ETH-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "long",
                "limit",
                95.0,
                None,
                Some("1".to_string()),
                Vec::new(),
                None,
            ),
        ],
        0,
        &market,
    );

    let shared_client_order_id = "bt-ambiguous-client";
    for order in &mut backtest.orders {
        order.client_order_id = shared_client_order_id.to_string();
    }

    backtest.submit_intents(
        &[cancel_missing_intent("", shared_client_order_id)],
        1,
        &market,
    );
    assert!(backtest.orders.iter().all(|order| order.status == "open"));
    let rejection = backtest
        .rejected_orders
        .last()
        .expect("global cancel ambiguity should be rejected");
    assert_eq!(rejection["action"].as_str(), Some("cancel_order"));
    assert!(rejection["error_message"]
        .as_str()
        .unwrap_or_default()
        .contains("同时命中多个交易对"));

    backtest.submit_intents(
        &[modify_missing_intent("", shared_client_order_id)],
        2,
        &market,
    );
    assert!(backtest
        .orders
        .iter()
        .all(|order| order.price == Some(95.0)));
    let rejection = backtest
        .rejected_orders
        .last()
        .expect("global modify ambiguity should be rejected");
    assert_eq!(rejection["action"].as_str(), Some("modify_order"));
    assert!(rejection["error_message"]
        .as_str()
        .unwrap_or_default()
        .contains("同时命中多个交易对"));
}

#[test]
fn modify_order_new_price_must_match_tick_size() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
        "tickSz": 0.1,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "limit", 95.0, Vec::new(), None)],
        0,
        &market,
    );
    let order = backtest.orders[0].clone();
    backtest.submit_intents(
        &[modify_intent(&order, None, Some("95.05"), true)],
        0,
        &market,
    );

    assert_eq!(backtest.orders[0].status, "open");
    assert!(backtest.orders[0].error_message.contains("tickSz"));
    assert_eq!(backtest.orders[0].price, Some(95.0));
    assert_eq!(
        backtest.rejected_orders[0]["action"].as_str(),
        Some("modify_order")
    );
    assert_eq!(backtest.rejected_orders[0]["price"].as_f64(), Some(95.0));
    assert_eq!(
        backtest.rejected_orders[0]["submitted_ts"].as_i64(),
        Some(0)
    );
    assert_eq!(backtest.rejected_orders[0]["updated_ts"].as_i64(), Some(0));
    assert!(backtest.rejected_orders[0]["error_message"]
        .as_str()
        .unwrap_or_default()
        .contains("tickSz"));
}

#[test]
fn modify_order_validates_all_fields_before_mutating_order() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
        "tickSz": 0.1,
        "lotSz": 1.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "limit", 95.0, Vec::new(), None)],
        0,
        &market,
    );
    let order = backtest.orders[0].clone();
    backtest.submit_intents(
        &[modify_intent(&order, Some("2"), Some("95.05"), false)],
        0,
        &market,
    );

    assert_eq!(backtest.orders[0].status, "open");
    assert_eq!(backtest.orders[0].exchange_quantity, 1.0);
    assert_eq!(backtest.orders[0].quantity, 1.0);
    assert_eq!(backtest.orders[0].price, Some(95.0));
    assert_eq!(
        backtest.rejected_orders[0]["action"].as_str(),
        Some("modify_order")
    );
}

#[test]
fn modify_order_size_uses_new_price_for_quote_ctval_contracts() {
    let config = test_config(json!({
        "ctVal": 10.0,
        "ctValCcy": "USDT",
        "lotSz": 1.0,
        "minSz": 1.0,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![
        swap_candle(1, 150.0, 151.0, 120.0, 150.0, 100.0, 100.0),
        swap_candle(2, 200.0, 201.0, 199.0, 200.0, 100.0, 100.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("10", "limit", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);
    assert!(backtest.fills.is_empty());

    let order = backtest.orders[0].clone();
    backtest.submit_intents(
        &[modify_intent(&order, Some("20"), Some("200.0"), false)],
        1,
        &market,
    );

    assert_eq!(backtest.orders[0].exchange_quantity, 20.0);
    assert_eq!(backtest.orders[0].price, Some(200.0));
    assert_eq!(backtest.orders[0].quantity, 1.0);

    backtest.process_market_until(2, &market);

    let fill = backtest
        .fills
        .iter()
        .find(|fill| fill["action"].as_str() == Some("open_position"))
        .expect("modified quote-ctVal order should fill");
    assert_eq!(fill["size"].as_f64(), Some(20.0));
    assert_eq!(fill["base_size"].as_f64(), Some(1.0));
    let position = backtest
        .positions
        .values()
        .next()
        .expect("modified order fill should create a position");
    assert_eq!(position.exchange_quantity, 20.0);
    assert_eq!(position.quantity, 1.0);
    assert_eq!(position.entry_price, 200.0);
}

#[test]
fn order_management_preserves_original_submission_timestamp() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
        "tickSz": 0.1,
        "lotSz": 1.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "limit", 95.0, Vec::new(), None)],
        0,
        &market,
    );
    let order = backtest.orders[0].clone();
    backtest.submit_intents(
        &[modify_intent(&order, Some("2"), Some("96.0"), false)],
        2,
        &market,
    );

    assert_eq!(backtest.orders[0].submitted_ts, 0);
    assert_eq!(backtest.orders[0].last_processed_ts, 2);
    assert_eq!(backtest.orders[0].price, Some(96.0));
    assert_eq!(backtest.orders[0].exchange_quantity, 2.0);
    let modified_order_json = super::values::order_json(&backtest.orders[0]);
    assert_eq!(modified_order_json["submitted_ts"].as_i64(), Some(0));
    assert_eq!(modified_order_json["timestamp"].as_i64(), Some(2));
    assert_eq!(modified_order_json["updated_ts"].as_i64(), Some(2));

    let order = backtest.orders[0].clone();
    backtest.submit_intents(
        &[cancel_missing_intent(
            &order.order_id,
            &order.client_order_id,
        )],
        3,
        &market,
    );

    assert_eq!(backtest.orders[0].status, "cancelled");
    assert_eq!(backtest.orders[0].submitted_ts, 0);
    assert_eq!(backtest.orders[0].last_processed_ts, 3);
    let cancelled_order_json = super::values::order_json(&backtest.orders[0]);
    assert_eq!(cancelled_order_json["submitted_ts"].as_i64(), Some(0));
    assert_eq!(cancelled_order_json["timestamp"].as_i64(), Some(3));
    assert_eq!(cancelled_order_json["updated_ts"].as_i64(), Some(3));
}

#[test]
fn order_management_missing_target_is_rejected_instead_of_silent_noop() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[
            cancel_missing_intent("missing-order", "missing-client"),
            modify_missing_intent("missing-order", "missing-client"),
        ],
        0,
        &market,
    );

    let rejected = backtest
        .rejected_orders
        .iter()
        .map(|row| {
            (
                row["action"].as_str().unwrap_or_default().to_string(),
                row["target_order_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                row["error_message"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(rejected.len(), 2);
    assert_eq!(rejected[0].0, "cancel_order");
    assert_eq!(rejected[0].1, "missing-order");
    assert!(rejected[0].2.contains("未找到目标订单"));
    assert_eq!(rejected[1].0, "modify_order");
    assert_eq!(rejected[1].1, "missing-order");
    assert!(rejected[1].2.contains("未找到目标订单"));
}

#[test]
fn duplicate_cancel_intent_is_skipped_like_live_action_submission() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "limit", 95.0, Vec::new(), None)],
        0,
        &market,
    );
    let order = backtest.orders[0].clone();
    let cancel = cancel_missing_intent(&order.order_id, &order.client_order_id);

    backtest.submit_intents(std::slice::from_ref(&cancel), 2, &market);
    backtest.submit_intents(&[cancel], 2, &market);

    assert_eq!(backtest.orders[0].status, "cancelled");
    assert!(backtest.rejected_orders.is_empty());
    assert_eq!(backtest.skipped_actions.len(), 1);
    let skipped = &backtest.skipped_actions[0];
    assert_eq!(skipped["action"].as_str(), Some("cancel_order"));
    assert!(skipped["action_identity"]
        .as_str()
        .unwrap_or_default()
        .contains(&order.order_id));
    assert!(skipped["_execution_skip_reason"]
        .as_str()
        .unwrap_or_default()
        .contains("live action 去重"));
}

#[test]
fn duplicate_standalone_risk_intent_is_skipped_like_live_action_submission() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
        "tickSz": 0.1,
    }));
    let candles = vec![candle(1, 100.0, 101.0, 99.0, 100.0, 10.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    let risk = risk_intent("1", 95.0);
    backtest.submit_intents(std::slice::from_ref(&risk), 1, &market);
    backtest.submit_intents(&[risk], 1, &market);

    let risk_orders = backtest
        .orders
        .iter()
        .filter(|order| matches!(order.action, OrderAction::Risk))
        .collect::<Vec<_>>();
    assert_eq!(risk_orders.len(), 1);
    assert_eq!(risk_orders[0].trigger_price, Some(95.0));
    assert_eq!(backtest.skipped_actions.len(), 1);
    let skipped = &backtest.skipped_actions[0];
    assert_eq!(skipped["action"].as_str(), Some("place_risk_order"));
    assert!(skipped["action_identity"]
        .as_str()
        .unwrap_or_default()
        .contains("place_risk_order"));
}

#[test]
fn implicit_open_quantity_uses_initial_capital_like_live_execution() {
    let config = test_config(json!({
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 1.0,
        "minSz": 1.0,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![swap_candle(1, 100.0, 101.0, 99.0, 100.0, 10_000.0, 100.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    backtest.cash = 20_000.0;

    backtest.submit_intents(
        &[intent(
            StrategyIntentAction::OpenPosition,
            "long",
            "market",
            100.0,
            None,
            None,
            Vec::new(),
            None,
        )],
        0,
        &market,
    );

    assert_eq!(backtest.orders[0].exchange_quantity, 1000.0);
    assert_eq!(backtest.orders[0].quantity, 10.0);
}

#[test]
fn futures_implicit_open_quantity_uses_contract_leverage_without_contract_mode_override() {
    let mut config = test_config(json!({
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 1.0,
        "minSz": 1.0,
        "tickSz": 0.1,
        "leverage": 3,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    config.inst_type = "FUTURES".to_string();
    let candles = vec![swap_candle(1, 100.0, 101.0, 99.0, 100.0, 10_000.0, 100.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let mut intent = intent(
        StrategyIntentAction::OpenPosition,
        "long",
        "market",
        100.0,
        None,
        None,
        Vec::new(),
        None,
    );
    intent.inst_type = "FUTURES".to_string();

    backtest.submit_intents(&[intent], 0, &market);
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.orders[0].status, "filled");
    assert_eq!(backtest.orders[0].inst_type, "FUTURES");
    assert_eq!(backtest.orders[0].exchange_quantity, 3000.0);
    assert_eq!(backtest.orders[0].quantity, 30.0);
}

#[test]
fn action_position_size_is_clamped_like_live_execution() {
    let config = test_config(json!({
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 1.0,
        "minSz": 1.0,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![swap_candle(1, 100.0, 101.0, 99.0, 100.0, 10_000.0, 100.0)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
    let mut intent = intent(
        StrategyIntentAction::OpenPosition,
        "long",
        "market",
        100.0,
        None,
        None,
        Vec::new(),
        None,
    );
    intent.action_record.position_size = Some(2.0);

    backtest.submit_intents(&[intent], 0, &market);
    backtest.process_market_until(1, &market);

    assert_eq!(backtest.orders[0].status, "filled");
    assert_eq!(backtest.orders[0].exchange_quantity, 10_000.0);
    assert_eq!(backtest.orders[0].quantity, 100.0);
    assert_eq!(backtest.fills.len(), 1);
}

#[test]
fn process_market_refreshes_trading_day_before_daily_loss_risk_check() {
    let config = test_config(json!({
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
        "max_daily_loss_ratio": 0.05,
    }));
    let day_one = 1;
    let day_two = 86_400_001;
    let candles = vec![
        candle(day_one, 100.0, 101.0, 99.0, 100.0, 10.0),
        candle(day_two, 100.0, 101.0, 99.0, 100.0, 10.0),
    ];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.record_equity(&candles[0], &market);
    assert_eq!(backtest.day_start_equity, 10_000.0);

    backtest.cash = 9_000.0;
    backtest.process_market_until(day_two, &market);
    assert_eq!(backtest.trading_day, "1970-01-02");
    assert_eq!(backtest.day_start_equity, 9_000.0);

    backtest.submit_intents(
        &[open_intent("1", "limit", 95.0, Vec::new(), None)],
        day_two,
        &market,
    );

    assert_eq!(backtest.orders.len(), 1);
    assert_eq!(backtest.orders[0].status, "open");
    assert!(
        backtest.rejected_orders.is_empty(),
        "new trading day should not inherit previous day loss baseline"
    );
}

#[test]
fn fill_capacity_below_lot_size_does_not_create_off_lot_partial_fill() {
    let config = test_config(json!({
        "ctVal": 0.01,
        "ctValCcy": "BTC",
        "lotSz": 1.0,
        "minSz": 1.0,
        "tickSz": 0.1,
        "historical_fee_rate": 0.0,
        "historical_participation_rate": 1.0,
        "historical_slippage_rate": 0.0,
        "historical_spread_rate": 0.0,
    }));
    let candles = vec![swap_candle(1, 100.0, 101.0, 99.0, 100.0, 0.5, 0.005)];
    let market = market(&candles);
    let mut backtest = HistoricalLiveBacktest::new(&config, &candles, 1);

    backtest.submit_intents(
        &[open_intent("1", "market", 100.0, Vec::new(), None)],
        0,
        &market,
    );
    backtest.process_market_until(1, &market);

    assert!(backtest.fills.is_empty());
    assert_eq!(backtest.orders[0].exchange_filled, 0.0);
    assert_eq!(backtest.orders[0].status, "cancelled");
    assert!(backtest.orders[0].error_message.contains("立即成交"));
}

fn test_config(params: Value) -> StrategyConfig {
    let mut merged = json!({
        "ctVal": 1.0,
        "ctValCcy": "BTC",
        "lotSz": 1.0,
        "minSz": 1.0,
        "tickSz": 0.1,
    });
    if let (Some(target), Some(overrides)) = (merged.as_object_mut(), params.as_object()) {
        for (key, value) in overrides {
            target.insert(key.clone(), value.clone());
        }
    }
    StrategyConfig {
        strategy_id: "test".to_string(),
        strategy_name: "Test".to_string(),
        symbol: SYMBOL.to_string(),
        inst_type: INST_TYPE.to_string(),
        timeframe: TIMEFRAME.to_string(),
        initial_capital: 10_000.0,
        position_size: 0.1,
        stop_loss: 0.05,
        take_profit: 0.1,
        params: merged,
    }
}

fn candle(timestamp: i64, open: f64, high: f64, low: f64, close: f64, volume: f64) -> OkxCandle {
    OkxCandle {
        timestamp,
        open,
        high,
        low,
        close,
        volume,
        volume_ccy: volume,
        volume_quote: volume * close,
        confirm: "1".to_string(),
    }
}

fn swap_candle(
    timestamp: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    volume_ccy: f64,
) -> OkxCandle {
    OkxCandle {
        timestamp,
        open,
        high,
        low,
        close,
        volume,
        volume_ccy,
        volume_quote: volume_ccy * close,
        confirm: "1".to_string(),
    }
}

fn market(candles: &[OkxCandle]) -> HistoricalMarketData {
    market_for_symbol(SYMBOL, candles)
}

fn market_for_symbol(symbol: &str, candles: &[OkxCandle]) -> HistoricalMarketData {
    HistoricalMarketData::new(vec![HistoricalCandleSeries {
        symbol: symbol.to_string(),
        inst_type: INST_TYPE.to_string(),
        timeframe: TIMEFRAME.to_string(),
        candles: candles.to_vec(),
    }])
}

fn market_with_funding(candles: &[OkxCandle], rates: &[(i64, f64)]) -> HistoricalMarketData {
    market(candles).with_funding(vec![HistoricalFundingSeries {
        symbol: SYMBOL.to_string(),
        inst_type: INST_TYPE.to_string(),
        rates: rates
            .iter()
            .map(|(funding_time, funding_rate)| HistoricalFundingPoint {
                symbol: SYMBOL.to_string(),
                inst_type: INST_TYPE.to_string(),
                funding_time: *funding_time,
                funding_rate: *funding_rate,
            })
            .collect(),
    }])
}

fn open_intent(
    exchange_size: &str,
    order_type: &str,
    price: f64,
    attached_risk_orders: Vec<StrategyRiskOrderIntent>,
    planned_exit: Option<StrategyPlannedExitIntent>,
) -> StrategyExecutionIntent {
    intent(
        StrategyIntentAction::OpenPosition,
        "long",
        order_type,
        price,
        None,
        Some(exchange_size.to_string()),
        attached_risk_orders,
        planned_exit,
    )
}

fn close_intent(order_side: &str, price: f64) -> StrategyExecutionIntent {
    intent(
        StrategyIntentAction::ClosePosition,
        "long",
        "market",
        price,
        Some(order_side.to_string()),
        None,
        Vec::new(),
        None,
    )
}

fn risk_intent(exchange_size: &str, trigger_price: f64) -> StrategyExecutionIntent {
    let risk = StrategyRiskOrderIntent {
        symbol: SYMBOL.to_string(),
        side: "sell".to_string(),
        order_type: "stop_loss".to_string(),
        trigger_price: Some(trigger_price),
        stop_loss: None,
        take_profit: None,
        reason: "standalone_risk".to_string(),
    };
    intent(
        StrategyIntentAction::PlaceRiskOrder,
        "long",
        "market",
        100.0,
        Some("sell".to_string()),
        Some(exchange_size.to_string()),
        vec![risk],
        None,
    )
}

fn cancel_missing_intent(order_id: &str, client_order_id: &str) -> StrategyExecutionIntent {
    let mut intent = intent(
        StrategyIntentAction::CancelOrder,
        "hold",
        "market",
        0.0,
        None,
        None,
        Vec::new(),
        None,
    );
    intent.cancel_order = Some(StrategyCancelOrderIntent {
        order_id: order_id.to_string(),
        client_order_id: client_order_id.to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
    });
    intent
}

fn modify_missing_intent(order_id: &str, client_order_id: &str) -> StrategyExecutionIntent {
    let mut intent = intent(
        StrategyIntentAction::ModifyOrder,
        "hold",
        "market",
        0.0,
        None,
        None,
        Vec::new(),
        None,
    );
    intent.modify_order = Some(StrategyModifyOrderIntent {
        order_id: order_id.to_string(),
        client_order_id: client_order_id.to_string(),
        new_size: Some("1".to_string()),
        new_price: None,
        cancel_on_fail: true,
        request_id: "missing-modify-test".to_string(),
        scope_explicit: false,
        target_kind: StrategyOrderTargetKind::Any,
        target_order_type: None,
    });
    intent
}

fn modify_intent(
    order: &super::state::SimOrder,
    new_size: Option<&str>,
    new_price: Option<&str>,
    cancel_on_fail: bool,
) -> StrategyExecutionIntent {
    StrategyExecutionIntent {
        action: StrategyIntentAction::ModifyOrder,
        action_record: StrategyActionRecord {
            action: "modify_order".to_string(),
            side: "hold".to_string(),
            price: order.reference_price,
            reason: "modify_order".to_string(),
            strength: 1.0,
            timestamp: 0,
            position_size: None,
        },
        symbol: order.symbol.clone(),
        inst_type: order.inst_type.clone(),
        timeframe: order.timeframe.clone(),
        order_type: order.order_type.clone(),
        order_side: None,
        exchange_size: None,
        planned_exit: None,
        cancel_order: None,
        modify_order: Some(StrategyModifyOrderIntent {
            order_id: order.order_id.clone(),
            client_order_id: order.client_order_id.clone(),
            new_size: new_size.map(str::to_string),
            new_price: new_price.map(str::to_string),
            cancel_on_fail,
            request_id: "modify-test".to_string(),
            scope_explicit: true,
            target_kind: StrategyOrderTargetKind::Any,
            target_order_type: None,
        }),
        stop_loss: None,
        take_profit: None,
        max_slippage: None,
        attached_risk_orders: Vec::new(),
    }
}

fn intent(
    action: StrategyIntentAction,
    side: &str,
    order_type: &str,
    price: f64,
    order_side: Option<String>,
    exchange_size: Option<String>,
    attached_risk_orders: Vec<StrategyRiskOrderIntent>,
    planned_exit: Option<StrategyPlannedExitIntent>,
) -> StrategyExecutionIntent {
    intent_for_symbol(
        SYMBOL,
        action,
        side,
        order_type,
        price,
        order_side,
        exchange_size,
        attached_risk_orders,
        planned_exit,
    )
}

fn intent_for_symbol(
    symbol: &str,
    action: StrategyIntentAction,
    side: &str,
    order_type: &str,
    price: f64,
    order_side: Option<String>,
    exchange_size: Option<String>,
    attached_risk_orders: Vec<StrategyRiskOrderIntent>,
    planned_exit: Option<StrategyPlannedExitIntent>,
) -> StrategyExecutionIntent {
    StrategyExecutionIntent {
        action,
        action_record: StrategyActionRecord {
            action: action.as_str().to_string(),
            side: side.to_string(),
            price,
            reason: action.as_str().to_string(),
            strength: 1.0,
            timestamp: 0,
            position_size: None,
        },
        symbol: symbol.to_string(),
        inst_type: INST_TYPE.to_string(),
        timeframe: TIMEFRAME.to_string(),
        order_type: order_type.to_string(),
        order_side,
        exchange_size,
        planned_exit,
        cancel_order: None,
        modify_order: None,
        stop_loss: None,
        take_profit: None,
        max_slippage: None,
        attached_risk_orders,
    }
}

fn attached_risk_quantity(backtest: &HistoricalLiveBacktest) -> Option<f64> {
    backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Risk))
        .map(|order| order.quantity)
}

fn attached_risk_trigger(backtest: &HistoricalLiveBacktest) -> Option<f64> {
    backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Risk))
        .and_then(|order| order.trigger_price)
}

fn attached_risk_order_type(backtest: &HistoricalLiveBacktest) -> Option<String> {
    backtest
        .orders
        .iter()
        .find(|order| matches!(order.action, OrderAction::Risk))
        .map(|order| order.order_type.clone())
}
