use std::time::Duration;

use serde_json::json;
use tokio::sync::broadcast;

use super::{
    normalize_live_config_scope, planned_exit_worker_wait_duration, subscriptions::*, trigger::*,
    LiveStrategyConfig, PLANNED_EXIT_IDLE_POLL,
};
use crate::{realtime::RealtimeCandleEvent, strategy_executor};

#[test]
fn live_private_subscriptions_cover_order_fill_position_and_account_state() {
    let channels = LIVE_PRIVATE_SUBSCRIPTIONS
        .iter()
        .map(|item| item.channel_key())
        .collect::<Vec<_>>();

    assert_eq!(
        channels,
        vec!["account", "orders", "orders-algo", "fills", "positions"]
    );
}

#[test]
fn live_candle_event_matching_uses_symbol_timeframe_and_positive_timestamp() {
    let subscriptions = vec![("BTC-USDT-SWAP".to_string(), "15m".to_string())];

    assert!(event_matches_subscriptions(
        &RealtimeCandleEvent {
            inst_id: "btc-usdt-swap".to_string(),
            timeframe: "15M".to_string(),
            timestamp: 1_780_000_000_000,
        },
        &subscriptions,
    ));
    assert!(!event_matches_subscriptions(
        &RealtimeCandleEvent {
            inst_id: "ETH-USDT-SWAP".to_string(),
            timeframe: "15m".to_string(),
            timestamp: 1_780_000_000_000,
        },
        &subscriptions,
    ));
    assert!(!event_matches_subscriptions(
        &RealtimeCandleEvent {
            inst_id: "BTC-USDT-SWAP".to_string(),
            timeframe: "15m".to_string(),
            timestamp: 0,
        },
        &subscriptions,
    ));
}

#[tokio::test]
async fn wait_for_strategy_candle_returns_matching_ws_event() {
    let (tx, mut rx) = broadcast::channel(8);
    let subscriptions = vec![("BTC-USDT-SWAP".to_string(), "15m".to_string())];
    tx.send(RealtimeCandleEvent {
        inst_id: "BTC-USDT-SWAP".to_string(),
        timeframe: "15m".to_string(),
        timestamp: 1_780_000_000_000,
    })
    .expect("receiver should be active");

    let trigger =
        wait_for_strategy_candle_or_watchdog(&mut rx, &subscriptions, Duration::from_secs(1)).await;

    assert!(matches!(trigger, LiveLoopTrigger::ConfirmedCandle(_)));
}

#[tokio::test]
async fn wait_for_strategy_candle_ignores_unrelated_events_until_watchdog() {
    let (tx, mut rx) = broadcast::channel(8);
    let subscriptions = vec![("BTC-USDT-SWAP".to_string(), "15m".to_string())];
    tx.send(RealtimeCandleEvent {
        inst_id: "ETH-USDT-SWAP".to_string(),
        timeframe: "15m".to_string(),
        timestamp: 1_780_000_000_000,
    })
    .expect("receiver should be active");

    let trigger =
        wait_for_strategy_candle_or_watchdog(&mut rx, &subscriptions, Duration::from_millis(10))
            .await;

    assert_eq!(trigger, LiveLoopTrigger::RestWatchdog);
}

#[tokio::test]
async fn wait_for_strategy_candle_ignores_auxiliary_timeframe_until_watchdog() {
    let (tx, mut rx) = broadcast::channel(8);
    let trigger_subscriptions = vec![("BTC-USDT-SWAP".to_string(), "15m".to_string())];
    tx.send(RealtimeCandleEvent {
        inst_id: "BTC-USDT-SWAP".to_string(),
        timeframe: "5m".to_string(),
        timestamp: 1_780_000_000_000,
    })
    .expect("receiver should be active");

    let trigger = wait_for_strategy_candle_or_watchdog(
        &mut rx,
        &trigger_subscriptions,
        Duration::from_millis(10),
    )
    .await;

    assert_eq!(trigger, LiveLoopTrigger::RestWatchdog);
}

#[test]
fn planned_exit_worker_waits_until_due_time_or_idle_poll() {
    assert_eq!(
        planned_exit_worker_wait_duration(1_700_000_000_000, Some(1_700_000_000_000)),
        Duration::ZERO
    );
    assert_eq!(
        planned_exit_worker_wait_duration(1_700_000_000_000, Some(1_700_000_015_000)),
        Duration::from_millis(15_000)
    );
    assert_eq!(
        planned_exit_worker_wait_duration(1_700_000_000_000, None),
        PLANNED_EXIT_IDLE_POLL
    );
}

#[test]
fn live_trigger_subscriptions_prefer_config_symbol_and_timeframe() {
    let mut config = config_for_subscription_tests();
    config.symbol = "BTC-USDT-SWAP".to_string();
    config.timeframe = "15m".to_string();
    let subscriptions = vec![
        ("BTC-USDT-SWAP".to_string(), "5m".to_string()),
        ("ETH-USDT-SWAP".to_string(), "15m".to_string()),
        ("BTC-USDT-SWAP".to_string(), "15m".to_string()),
    ];

    let triggers =
        live_trigger_subscriptions(&config, &subscriptions).expect("trigger subscriptions");

    assert_eq!(
        triggers,
        vec![("BTC-USDT-SWAP".to_string(), "15m".to_string())]
    );
}

#[test]
fn live_trigger_subscriptions_fall_back_to_decision_timeframe() {
    let mut config = config_for_subscription_tests();
    config.symbol = "BTC-USDT-SWAP".to_string();
    config.timeframe = "15m".to_string();
    let subscriptions = vec![
        ("BTC-USDT-SWAP".to_string(), "5m".to_string()),
        ("ETH-USDT-SWAP".to_string(), "15m".to_string()),
        ("SOL-USDT-SWAP".to_string(), "15m".to_string()),
    ];

    let triggers =
        live_trigger_subscriptions(&config, &subscriptions).expect("trigger subscriptions");

    assert_eq!(
        triggers,
        vec![
            ("ETH-USDT-SWAP".to_string(), "15m".to_string()),
            ("SOL-USDT-SWAP".to_string(), "15m".to_string()),
        ]
    );
}

#[test]
fn subscription_symbol_normalization_respects_instrument_type() {
    assert_eq!(
        strategy_executor::normalize_runtime_inst_id("btc-usdt", "SWAP").expect("swap symbol"),
        "BTC-USDT-SWAP"
    );
    assert_eq!(
        strategy_executor::normalize_runtime_inst_id("btc-usdt-swap", "SPOT").expect("spot symbol"),
        "BTC-USDT"
    );
}

#[test]
fn live_start_config_scope_is_normalized_once() {
    let mut config = config_for_subscription_tests();
    config.symbol = "btc-usdt".to_string();
    config.inst_type = "swap".to_string();

    normalize_live_config_scope(&mut config).expect("live config scope should normalize");

    assert_eq!(config.symbol, "BTC-USDT-SWAP");
    assert_eq!(config.inst_type, "SWAP");
}

#[test]
fn live_start_config_scope_infers_swap_from_symbol_suffix() {
    let mut config = config_for_subscription_tests();
    config.symbol = "eth-usdt-swap".to_string();
    config.inst_type = String::new();

    normalize_live_config_scope(&mut config).expect("live config scope should infer swap");

    assert_eq!(config.symbol, "ETH-USDT-SWAP");
    assert_eq!(config.inst_type, "SWAP");
}

#[test]
fn portfolio_layers_detection_blocks_any_config_key() {
    assert!(has_portfolio_layers(&json!({"portfolio_layers": [{}]})));
    assert!(has_portfolio_layers(&json!({"portfolio_layers": []})));
    assert!(has_portfolio_layers(&json!({"portfolio_layers": "legacy"})));
    assert!(!has_portfolio_layers(&json!({})));
}

fn config_for_subscription_tests() -> LiveStrategyConfig {
    LiveStrategyConfig {
        strategy_id: "subscription_test".to_string(),
        strategy_name: "Subscription Test".to_string(),
        symbol: "BTC-USDT-SWAP".to_string(),
        timeframe: "15m".to_string(),
        inst_type: "SWAP".to_string(),
        mode: "simulated".to_string(),
        initial_capital: 1_000.0,
        position_size: 0.1,
        stop_loss: 0.02,
        take_profit: 0.0,
        risk_timeframe: "1m".to_string(),
        check_interval: 60,
        params: json!({}),
        project_root: std::path::PathBuf::new(),
        risk_control_enabled: true,
        max_single_loss_ratio: 0.05,
        max_position_pct: 1.0,
        max_order_value: 10_000.0,
    }
}
