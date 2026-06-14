use super::*;
use serde_json::json;

#[test]
fn okx_bar_keeps_hour_and_day_units_uppercase() {
    assert_eq!(okx_bar("1h"), "1H");
    assert_eq!(okx_bar("4h"), "4H");
    assert_eq!(okx_bar("1d"), "1D");
    assert_eq!(okx_bar("1m"), "1m");
}

#[test]
fn parse_okx_candle_rejects_invalid_market_values() {
    assert!(parse_okx_candle(json!([
        "1700000000000",
        "100",
        "101",
        "99",
        "100",
        "0",
        "0",
        "0",
        "1"
    ]))
    .is_some());
    assert!(parse_okx_candle(json!([
        "1700000000000",
        "NaN",
        "101",
        "99",
        "100",
        "0",
        "0",
        "0",
        "1"
    ]))
    .is_none());
    assert!(
        parse_okx_candle(json!(["0", "100", "101", "99", "100", "0", "0", "0", "1"])).is_none()
    );
}

#[test]
fn parse_okx_candle_rejects_missing_volume_or_confirm_fields() {
    assert!(parse_okx_candle(json!(["1700000000000", "100", "101", "99", "100"])).is_none());
    assert!(parse_okx_candle(json!([
        "1700000000000",
        "100",
        "101",
        "99",
        "100",
        "0",
        "0",
        "0"
    ]))
    .is_none());
    assert!(parse_okx_candle(json!([
        "1700000000000",
        "100",
        "101",
        "99",
        "100",
        "0",
        "0",
        "0",
        "done"
    ]))
    .is_none());
}

#[test]
fn okx_candle_json_contract_keeps_quote_volume_and_confirm_variant() {
    let candle = OkxCandle {
        timestamp: 1700000000000,
        open: 100.0,
        high: 101.0,
        low: 99.0,
        close: 100.5,
        volume: 12.0,
        volume_ccy: 1206.0,
        volume_quote: 1206.5,
        confirm: "1".to_string(),
    };

    let value = candle.to_json();
    assert_eq!(value["volume_quote"], 1206.5);
    assert!(value.get("confirm").is_none());

    let value = candle.to_json_with_confirm();
    assert_eq!(value["volume_quote"], 1206.5);
    assert_eq!(value["confirm"], "1");
}

#[test]
fn normalize_trade_rejects_invalid_price_or_size() {
    assert!(normalize_trade(
        json!({
            "instId": "BTC-USDT-SWAP",
            "tradeId": "bad-price",
            "px": "bad",
            "sz": "1",
            "side": "buy",
            "ts": "1700000000000"
        }),
        "BTC-USDT-SWAP",
    )
    .is_none());

    assert!(normalize_trade(
        json!({
            "instId": "BTC-USDT-SWAP",
            "tradeId": "zero-size",
            "px": "70000",
            "sz": "0",
            "side": "sell",
            "ts": "1700000000000"
        }),
        "BTC-USDT-SWAP",
    )
    .is_none());
}

#[test]
fn normalize_trade_rejects_missing_trade_id_or_invalid_side() {
    assert!(normalize_trade(
        json!({
            "instId": "BTC-USDT-SWAP",
            "px": "70000",
            "sz": "1",
            "side": "buy",
            "ts": "1700000000000"
        }),
        "BTC-USDT-SWAP",
    )
    .is_none());

    assert!(normalize_trade(
        json!({
            "instId": "BTC-USDT-SWAP",
            "tradeId": "bad-side",
            "px": "70000",
            "sz": "1",
            "side": "hold",
            "ts": "1700000000000"
        }),
        "BTC-USDT-SWAP",
    )
    .is_none());
}

#[test]
fn normalize_trade_rejects_invalid_timestamp_instead_of_fabricating_epoch() {
    assert!(normalize_trade(
        json!({
            "instId": "BTC-USDT-SWAP",
            "tradeId": "bad-ts",
            "px": "70000",
            "sz": "1",
            "side": "buy",
            "ts": "bad-ts"
        }),
        "BTC-USDT-SWAP",
    )
    .is_none());
}

#[test]
fn normalize_orderbook_rejects_zero_size_levels() {
    let book = normalize_orderbook(
        json!({
            "instId": "BTC-USDT-SWAP",
            "bids": [["70000", "0", "0", "1"], ["69999", "2", "0", "1"]],
            "asks": [["70001", "1", "0", "1"]],
            "ts": "1700000000000"
        }),
        "BTC-USDT-SWAP",
    )
    .expect("valid orderbook snapshot");

    assert_eq!(book["bids"].as_array().unwrap().len(), 1);
    assert_eq!(book["bids"][0]["price"], 69999.0);
    assert_eq!(book["bids"][0]["size"], 2.0);
}

#[test]
fn normalize_orderbook_rejects_invalid_timestamp_instead_of_fabricating_epoch() {
    assert!(normalize_orderbook(
        json!({
            "instId": "BTC-USDT-SWAP",
            "bids": [["70000", "1", "0", "1"]],
            "asks": [["70001", "1", "0", "1"]],
            "ts": "bad-ts"
        }),
        "BTC-USDT-SWAP",
    )
    .is_none());
}

#[test]
fn normalize_orderbook_rejects_snapshot_without_valid_bid_or_ask() {
    assert!(normalize_orderbook(
        json!({
            "instId": "BTC-USDT-SWAP",
            "bids": [["70000", "0", "0", "1"]],
            "asks": [["70001", "1", "0", "1"]],
            "ts": "1700000000000"
        }),
        "BTC-USDT-SWAP",
    )
    .is_none());

    assert!(normalize_orderbook(
        json!({
            "instId": "BTC-USDT-SWAP",
            "bids": [["70000", "1", "0", "1"]],
            "asks": [["70001", "0", "0", "1"]],
            "ts": "1700000000000"
        }),
        "BTC-USDT-SWAP",
    )
    .is_none());
}
