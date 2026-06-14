use serde_json::json;

use crate::strategy_executor::{
    candle_requirements, context_cache_stamp, funding_requirements, orderbook_requirements,
    state_context_requirements, strategy_context, StrategyContextInput,
};

#[test]
fn candle_requirements_supports_symbols_timeframes_cross_product() {
    let requirements = candle_requirements(
        &json!({
            "symbols": ["BTC-USDT-SWAP", "ETH-USDT-SWAP"],
            "timeframes": ["1m", "15m"],
        }),
        "BTC-USDT-SWAP",
        "SWAP",
        "15m",
        120,
    );

    assert!(requirements
        .iter()
        .any(|item| item.symbol == "ETH-USDT-SWAP" && item.timeframe == "1m"));
    assert!(requirements
        .iter()
        .any(|item| item.symbol == "BTC-USDT-SWAP" && item.timeframe == "15m"));
    assert_eq!(requirements.len(), 4);
}

#[test]
fn candle_requirements_merges_explicit_candle_specs_and_primary() {
    let requirements = candle_requirements(
        &json!({
            "candles": [
                {"symbol": "ETH-USDT-SWAP", "inst_type": "SWAP", "timeframe": "5m", "min_bars": 200},
                {"symbol": "ETH-USDT-SWAP", "inst_type": "SWAP", "timeframe": "5m", "min_bars": 500}
            ],
        }),
        "BTC-USDT-SWAP",
        "SWAP",
        "15m",
        120,
    );

    let eth = requirements
        .iter()
        .find(|item| item.symbol == "ETH-USDT-SWAP" && item.timeframe == "5m")
        .unwrap();
    assert_eq!(eth.min_bars, 500);
    assert!(requirements
        .iter()
        .any(|item| item.symbol == "BTC-USDT-SWAP" && item.timeframe == "15m"));
}

#[test]
fn orderbook_requirements_follow_symbols_when_enabled() {
    let requirements = orderbook_requirements(
        &json!({
            "symbols": ["BTC-USDT-SWAP", "ETH-USDT-SWAP"],
            "timeframes": ["1m", "15m"],
            "orderbook": true,
        }),
        "BTC-USDT-SWAP",
        "SWAP",
    );

    assert_eq!(requirements.len(), 2);
    assert!(requirements
        .iter()
        .any(|item| item.symbol == "BTC-USDT-SWAP" && item.depth == 1 && item.required));
    assert!(requirements
        .iter()
        .any(|item| item.symbol == "ETH-USDT-SWAP" && item.depth == 1 && item.required));
}

#[test]
fn orderbook_requirements_parse_explicit_specs_and_merge_depth() {
    let requirements = orderbook_requirements(
        &json!({
            "orderbook": [
                {"symbol": "eth-usdt-swap", "inst_type": "SWAP", "depth": 5, "required": false},
                {"symbol": "ETH-USDT-SWAP", "inst_type": "SWAP", "size": 20, "required": true},
                "SOL-USDT-SWAP"
            ],
        }),
        "BTC-USDT-SWAP",
        "SWAP",
    );

    let eth = requirements
        .iter()
        .find(|item| item.symbol == "ETH-USDT-SWAP")
        .unwrap();
    assert_eq!(eth.depth, 20);
    assert!(eth.required);
    assert!(requirements
        .iter()
        .any(|item| item.symbol == "SOL-USDT-SWAP" && item.depth == 1));
}

#[test]
fn feed_requirements_parse_object_symbol_lists() {
    let orderbook = orderbook_requirements(
        &json!({
            "orderbook": {
                "symbols": ["eth-usdt-swap", "sol-usdt-swap"],
                "inst_type": "SWAP",
                "sz": 25,
                "required": false
            },
        }),
        "BTC-USDT-SWAP",
        "SPOT",
    );
    assert_eq!(orderbook.len(), 2);
    assert!(orderbook
        .iter()
        .all(|item| { item.inst_type == "SWAP" && item.depth == 25 && !item.required }));
    assert!(orderbook.iter().any(|item| item.symbol == "ETH-USDT-SWAP"));

    let funding = funding_requirements(
        &json!({
            "funding": {
                "symbols": ["eth-usdt-swap", "sol-usdt-swap"],
                "inst_type": "SWAP",
                "lookback_rows": 36,
                "required": false
            },
        }),
        "BTC-USDT-SWAP",
        "SPOT",
    );
    assert_eq!(funding.len(), 2);
    assert!(funding
        .iter()
        .all(|item| { item.inst_type == "SWAP" && item.history_limit == 36 && !item.required }));
    assert!(funding.iter().any(|item| item.symbol == "SOL-USDT-SWAP"));
}

#[test]
fn funding_requirements_follow_symbols_when_enabled() {
    let requirements = funding_requirements(
        &json!({
            "symbols": ["BTC-USDT-SWAP", "ETH-USDT-SWAP"],
            "funding": true,
        }),
        "BTC-USDT-SWAP",
        "SWAP",
    );

    assert_eq!(requirements.len(), 2);
    assert!(requirements.iter().any(|item| {
        item.symbol == "BTC-USDT-SWAP" && item.history_limit == 12 && item.required
    }));
    assert!(requirements
        .iter()
        .any(|item| item.symbol == "ETH-USDT-SWAP" && item.required));
}

#[test]
fn funding_requirements_parse_explicit_specs_and_merge_history_limit() {
    let requirements = funding_requirements(
        &json!({
            "funding": [
                {"symbol": "eth-usdt-swap", "inst_type": "SWAP", "limit": 8, "required": false},
                {"symbol": "ETH-USDT-SWAP", "inst_type": "SWAP", "history_limit": 48, "required": true},
                "SOL-USDT-SWAP"
            ],
        }),
        "BTC-USDT-SWAP",
        "SWAP",
    );

    let eth = requirements
        .iter()
        .find(|item| item.symbol == "ETH-USDT-SWAP")
        .unwrap();
    assert_eq!(eth.history_limit, 48);
    assert!(eth.required);
    assert!(requirements
        .iter()
        .any(|item| item.symbol == "SOL-USDT-SWAP" && item.history_limit == 12));
}

#[test]
fn state_context_requirements_reports_enabled_state_sections() {
    let requirements = state_context_requirements(&json!({
        "positions": true,
        "account": {"required": true},
        "orders": {"recent_fills": true}
    }));
    assert!(requirements.account);
    assert!(requirements.positions);
    assert!(requirements.orders);

    let disabled = state_context_requirements(&json!({
        "positions": {"required": false},
        "account": {"required": false},
        "orders": {"open": false, "recent_fills": false, "recent_rejections": false}
    }));
    assert!(!disabled.account);
    assert!(!disabled.positions);
    assert!(!disabled.orders);
}

#[test]
fn strategy_context_normalizes_order_state_sections() {
    let config = json!({
        "strategy_id": "fixture",
        "strategy_name": "Fixture",
        "symbol": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "15m",
    });
    let context = strategy_context(StrategyContextInput {
        config: &config,
        candles: json!({}),
        timestamp: 2,
        account: json!({}),
        positions: json!({}),
        orders: json!({
            "open": [{"id": "risk-1"}],
            "recent_fills": "bad-shape",
            "total_orders": 3
        }),
        funding: json!({}),
        orderbook: json!({}),
    });

    assert_eq!(context["orders"]["open"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        context["orders"]["recent_fills"].as_array().map(Vec::len),
        Some(0)
    );
    assert_eq!(
        context["orders"]["recent_rejections"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(context["orders"]["total_orders"].as_i64(), Some(3));

    let stamp = context_cache_stamp(&context);
    assert_eq!(stamp["orders"]["open"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        stamp["orders"]["recent_fills"].as_array().map(Vec::len),
        Some(0)
    );
    assert_eq!(stamp["orders"]["total_orders"].as_i64(), Some(3));
}

#[test]
fn context_cache_stamp_summarizes_candles_and_state() {
    let config = json!({
        "strategy_id": "fixture",
        "strategy_name": "Fixture",
        "symbol": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "15m",
    });
    let context = strategy_context(StrategyContextInput {
        config: &config,
        candles: json!({
            "BTC-USDT-SWAP": {
                "15m": [
                    {"timestamp": 1, "close": 100.0},
                    {"timestamp": 2, "close": 101.5}
                ]
            }
        }),
        timestamp: 2,
        account: json!({"equity": 1000.0}),
        positions: json!({"BTC-USDT-SWAP": {"side": "long"}}),
        orders: json!({"open": [], "recent_fills": [], "recent_rejections": []}),
        funding: json!({"BTC-USDT-SWAP": {
            "source": "okx_funding_rates",
            "latest": {"funding_time": 2, "funding_rate": 0.0002},
            "history": [
                {"funding_time": 1, "funding_rate": 0.0001},
                {"funding_time": 2, "funding_rate": 0.0002}
            ]
        }}),
        orderbook: json!({"BTC-USDT-SWAP": {
            "best_bid": 99.5,
            "best_ask": 100.5,
            "mid_price": 100.0,
            "spread": 1.0,
            "asks": [{"price": 100.5, "size": 1.0}],
            "bids": [{"price": 99.5, "size": 1.0}],
            "ts": 3
        }}),
    });

    let stamp = context_cache_stamp(&context);

    assert_eq!(stamp["candles"]["BTC-USDT-SWAP"]["15m"]["len"], json!(2));
    assert_eq!(
        stamp["candles"]["BTC-USDT-SWAP"]["15m"]["last_ts"],
        json!(2)
    );
    assert_eq!(
        stamp["positions"]["BTC-USDT-SWAP"]["side"].as_str(),
        Some("long")
    );
    assert_eq!(
        stamp["orderbook"]["BTC-USDT-SWAP"]["best_ask"],
        json!(100.5)
    );
    assert_eq!(
        stamp["funding"]["BTC-USDT-SWAP"]["latest_rate"],
        json!(0.0002)
    );
    assert_eq!(stamp["funding"]["BTC-USDT-SWAP"]["history_len"], json!(2));
    assert!(stamp["orderbook"]["BTC-USDT-SWAP"].get("asks").is_none());
}
