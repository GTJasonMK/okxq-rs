use serde_json::{json, Map, Value};

use crate::{commands::local_api::LocalApiRequest, okx::OkxCandle};

use super::{candle_to_json, merge_latest_diagnostic_candle};

#[test]
fn realtime_diagnostic_candle_replaces_current_bar_for_diagnostics() {
    let req = request_with_latest_candle(json!({
        "inst_id": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "15m",
        "timestamp": 1_780_000_900_000i64,
        "open": 100.0,
        "high": 103.0,
        "low": 99.0,
        "close": 102.0,
        "volume": 12.0,
        "volume_ccy": 1200.0,
        "volume_quote": 1200.0,
        "confirm": "0"
    }));
    let mut candles = vec![
        candle(1_780_000_000_000, 100.0),
        candle(1_780_000_900_000, 101.0),
    ];

    let applied =
        merge_latest_diagnostic_candle(&req, "BTC-USDT-SWAP", "SWAP", "15m", 10, &mut candles)
            .expect("merge realtime candle");

    assert!(applied);
    assert_eq!(candles.len(), 2);
    assert_eq!(candles[1].close, 102.0);
    assert_eq!(candles[1].confirm, "0");
}

#[test]
fn realtime_diagnostic_candle_appends_next_unclosed_bar_for_diagnostics() {
    let req = request_with_latest_candle(json!({
        "inst_id": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "15m",
        "timestamp": 1_780_001_800_000i64,
        "open": 102.0,
        "high": 104.0,
        "low": 101.0,
        "close": 103.0,
        "volume": 8.0,
        "volume_ccy": 824.0,
        "volume_quote": 824.0,
        "confirm": "0"
    }));
    let mut candles = vec![
        candle(1_780_000_000_000, 100.0),
        candle(1_780_000_900_000, 102.0),
    ];

    let applied =
        merge_latest_diagnostic_candle(&req, "BTC-USDT-SWAP", "SWAP", "15m", 2, &mut candles)
            .expect("merge realtime candle");

    assert!(applied);
    assert_eq!(candles.len(), 2);
    assert_eq!(candles[0].timestamp, 1_780_000_900_000);
    assert_eq!(candles[1].timestamp, 1_780_001_800_000);
    assert_eq!(candles[1].close, 103.0);
}

#[test]
fn realtime_diagnostic_candle_rejects_invalid_ohlc_for_diagnostics() {
    let req = request_with_latest_candle(json!({
        "inst_id": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "15m",
        "timestamp": 1_780_000_900_000i64,
        "open": 0.0,
        "high": 103.0,
        "low": 99.0,
        "close": 102.0,
        "volume": 12.0,
        "confirm": "0"
    }));
    let mut candles = vec![candle(1_780_000_900_000, 101.0)];

    let result =
        merge_latest_diagnostic_candle(&req, "BTC-USDT-SWAP", "SWAP", "15m", 10, &mut candles);

    assert!(result.is_err());
    assert_eq!(candles.len(), 1);
    assert_eq!(candles[0].close, 101.0);
}

#[test]
fn realtime_diagnostic_candle_rejects_missing_volume_for_diagnostics() {
    let req = request_with_latest_candle(json!({
        "inst_id": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "15m",
        "timestamp": 1_780_000_900_000i64,
        "open": 100.0,
        "high": 103.0,
        "low": 99.0,
        "close": 102.0,
        "confirm": "0"
    }));
    let mut candles = vec![candle(1_780_000_900_000, 101.0)];

    let result =
        merge_latest_diagnostic_candle(&req, "BTC-USDT-SWAP", "SWAP", "15m", 10, &mut candles);

    assert!(result.is_err());
    assert_eq!(candles.len(), 1);
    assert_eq!(candles[0].close, 101.0);
}

#[test]
fn realtime_diagnostic_candle_rejects_missing_quote_volumes_and_confirm_for_diagnostics() {
    let missing_quote_volumes = request_with_latest_candle(json!({
        "inst_id": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "15m",
        "timestamp": 1_780_000_900_000i64,
        "open": 100.0,
        "high": 103.0,
        "low": 99.0,
        "close": 102.0,
        "volume": 12.0,
        "confirm": "0"
    }));
    let missing_confirm = request_with_latest_candle(json!({
        "inst_id": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "15m",
        "timestamp": 1_780_000_900_000i64,
        "open": 100.0,
        "high": 103.0,
        "low": 99.0,
        "close": 102.0,
        "volume": 12.0,
        "volume_ccy": 1200.0,
        "volume_quote": 1200.0
    }));
    let mut candles = vec![candle(1_780_000_900_000, 101.0)];

    assert!(merge_latest_diagnostic_candle(
        &missing_quote_volumes,
        "BTC-USDT-SWAP",
        "SWAP",
        "15m",
        10,
        &mut candles,
    )
    .is_err());
    assert!(merge_latest_diagnostic_candle(
        &missing_confirm,
        "BTC-USDT-SWAP",
        "SWAP",
        "15m",
        10,
        &mut candles,
    )
    .is_err());
    assert_eq!(candles.len(), 1);
    assert_eq!(candles[0].close, 101.0);
}

#[test]
fn realtime_diagnostic_candle_rejects_string_numbers_and_fractional_timestamp_for_diagnostics() {
    let string_numbers = request_with_latest_candle(json!({
        "inst_id": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "15m",
        "timestamp": "1780000900000",
        "open": "100",
        "high": "103",
        "low": "99",
        "close": "102",
        "volume": "12",
        "volume_ccy": "1200",
        "volume_quote": "1200",
        "confirm": "0"
    }));
    let fractional_timestamp = request_with_latest_candle(json!({
        "inst_id": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "15m",
        "timestamp": 1_780_000_900_000.4,
        "open": 100.0,
        "high": 103.0,
        "low": 99.0,
        "close": 102.0,
        "volume": 12.0,
        "volume_ccy": 1200.0,
        "volume_quote": 1200.0,
        "confirm": "0"
    }));
    let mut candles = vec![candle(1_780_000_900_000, 101.0)];

    assert!(merge_latest_diagnostic_candle(
        &string_numbers,
        "BTC-USDT-SWAP",
        "SWAP",
        "15m",
        10,
        &mut candles,
    )
    .is_err());
    assert!(merge_latest_diagnostic_candle(
        &fractional_timestamp,
        "BTC-USDT-SWAP",
        "SWAP",
        "15m",
        10,
        &mut candles,
    )
    .is_err());
    assert_eq!(candles.len(), 1);
    assert_eq!(candles[0].close, 101.0);
}

#[test]
fn realtime_diagnostic_candle_rejects_missing_scope_instead_of_binding_to_request_target() {
    let missing_scope = request_with_latest_candle(json!({
        "timestamp": 1_780_000_900_000i64,
        "open": 100.0,
        "high": 103.0,
        "low": 99.0,
        "close": 102.0,
        "volume": 12.0,
        "volume_ccy": 1200.0,
        "volume_quote": 1200.0,
        "confirm": "0"
    }));
    let mut candles = vec![candle(1_780_000_900_000, 101.0)];

    assert!(merge_latest_diagnostic_candle(
        &missing_scope,
        "BTC-USDT-SWAP",
        "SWAP",
        "15m",
        10,
        &mut candles,
    )
    .is_err());
    assert_eq!(candles.len(), 1);
    assert_eq!(candles[0].close, 101.0);
}

#[test]
fn realtime_diagnostic_candle_matches_canonical_okx_timeframe() {
    let req = request_with_latest_candle(json!({
        "inst_id": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "1H",
        "timestamp": 1_780_003_600_000i64,
        "open": 100.0,
        "high": 104.0,
        "low": 99.0,
        "close": 103.0,
        "volume": 12.0,
        "volume_ccy": 1236.0,
        "volume_quote": 1236.0,
        "confirm": "0"
    }));
    let mut candles = vec![candle(1_780_000_000_000, 100.0)];

    let applied =
        merge_latest_diagnostic_candle(&req, "BTC-USDT-SWAP", "SWAP", "1h", 10, &mut candles)
            .expect("merge canonical timeframe candle");

    assert!(applied);
    assert_eq!(candles.len(), 2);
    assert_eq!(candles[1].timestamp, 1_780_003_600_000);
    assert_eq!(candles[1].close, 103.0);
}

#[test]
fn candle_json_keeps_okx_quote_volume_fields_for_strategy_context() {
    let value = candle_to_json(&OkxCandle {
        timestamp: 1_780_000_900_000,
        open: 100.0,
        high: 103.0,
        low: 99.0,
        close: 102.0,
        volume: 12.0,
        volume_ccy: 1_224.0,
        volume_quote: 1_225.0,
        confirm: "1".to_string(),
    });

    assert_eq!(value["volume"].as_f64(), Some(12.0));
    assert_eq!(value["volume_ccy"].as_f64(), Some(1_224.0));
    assert_eq!(value["volume_quote"].as_f64(), Some(1_225.0));
    assert_eq!(value["confirm"].as_str(), Some("1"));
}

fn request_with_latest_candle(latest_candle: Value) -> LocalApiRequest {
    LocalApiRequest {
        method: "POST".to_string(),
        path: "/api/live/decision-diagnostics".to_string(),
        params: Map::new(),
        body: json!({ "latest_candle": latest_candle }),
    }
}

fn candle(timestamp: i64, close: f64) -> OkxCandle {
    OkxCandle {
        timestamp,
        open: close,
        high: close,
        low: close,
        close,
        volume: 1.0,
        volume_ccy: 0.0,
        volume_quote: 0.0,
        confirm: "1".to_string(),
    }
}
