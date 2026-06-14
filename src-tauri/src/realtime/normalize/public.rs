use serde_json::{json, Value};

use crate::{
    instrument::infer_spot_swap_inst_type, orderbook_snapshot::normalize_orderbook_snapshot,
};

use super::values::*;

pub(in crate::realtime) fn normalize_ticker(ticker: Value, arg_inst_id: &str) -> Option<Value> {
    let inst_id = value_string(&ticker, "instId")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| arg_inst_id.to_string())
        .trim()
        .to_string();
    if inst_id.is_empty() {
        return None;
    }
    let inst_type = value_string(&ticker, "instType")
        .unwrap_or_else(|| infer_spot_swap_inst_type(&inst_id).to_string());
    let last_price = positive_f64(ticker.get("last"))?;
    let ask = positive_f64(ticker.get("askPx"))?;
    let bid = positive_f64(ticker.get("bidPx"))?;
    let open24h = positive_f64(ticker.get("open24h"))?;
    let high24h = positive_f64(ticker.get("high24h"))?;
    let low24h = positive_f64(ticker.get("low24h"))?;
    let vol24h = non_negative_f64(ticker.get("vol24h"))?;
    let change24h = calculate_change_24h(last_price, open24h);

    Some(json!({
        "inst_id": inst_id,
        "inst_type": inst_type,
        "last": last_price,
        "ask": ask,
        "bid": bid,
        "open24h": open24h,
        "high24h": high24h,
        "low24h": low24h,
        "vol24h": vol24h,
        "change24h": change24h,
        "ts": positive_timestamp(ticker.get("ts"))?,
    }))
}

pub(in crate::realtime) fn normalize_trade(trade: Value, arg_inst_id: &str) -> Option<Value> {
    crate::okx::normalize_trade(trade, arg_inst_id)
}

pub(in crate::realtime) fn normalize_orderbook(
    book: Value,
    arg_inst_id: &str,
    channel: &str,
) -> Option<Value> {
    let inst_id = value_string(&book, "instId")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| arg_inst_id.to_string())
        .trim()
        .to_uppercase();
    if inst_id.is_empty() {
        return None;
    }

    let snapshot = normalize_orderbook_snapshot(&book)?;

    Some(json!({
        "inst_id": inst_id,
        "channel": channel,
        "asks": snapshot.asks,
        "bids": snapshot.bids,
        "best_ask": snapshot.best_ask,
        "best_bid": snapshot.best_bid,
        "mid_price": snapshot.mid_price,
        "spread": snapshot.spread,
        "spread_rate": snapshot.spread_rate,
        "ask_depth_total": snapshot.ask_depth_total,
        "bid_depth_total": snapshot.bid_depth_total,
        "ts": snapshot.timestamp,
        "checksum": value_i64(book.get("checksum")).unwrap_or(0),
    }))
}

fn positive_f64(value: Option<&Value>) -> Option<f64> {
    let parsed = parse_f64(value)?;
    (parsed.is_finite() && parsed > 0.0).then_some(parsed)
}

fn non_negative_f64(value: Option<&Value>) -> Option<f64> {
    let parsed = parse_f64(value)?;
    (parsed.is_finite() && parsed >= 0.0).then_some(parsed)
}

fn positive_timestamp(value: Option<&Value>) -> Option<i64> {
    value_i64(value).filter(|timestamp| *timestamp > 0)
}

fn calculate_change_24h(last: f64, open: f64) -> f64 {
    ((last - open) / open) * 100.0
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{normalize_orderbook, normalize_ticker, normalize_trade};

    #[test]
    fn normalize_ticker_does_not_fabricate_zero_from_invalid_last() {
        let ticker = normalize_ticker(
            json!({
                "instId": "BTC-USDT-SWAP",
                "last": "bad-last",
                "askPx": "101",
                "bidPx": "99",
                "open24h": "100",
                "ts": "1780000000000",
            }),
            "",
        );

        assert!(ticker.is_none());
    }

    #[test]
    fn normalize_orderbook_rejects_invalid_or_zero_size_levels() {
        let orderbook = normalize_orderbook(
            json!({
                "instId": "BTC-USDT-SWAP",
                "bids": [
                    ["100", "bad-size", "0", "4"],
                    ["99", "0", "0", "1"],
                    ["98", "2", "0", "2"]
                ],
                "asks": [["101", "3", "0", "5"]],
                "ts": "1780000000000",
            }),
            "",
            "books5",
        )
        .expect("valid side should keep orderbook");

        assert_eq!(
            orderbook
                .get("bids")
                .and_then(|value| value.as_array())
                .cloned(),
            Some(vec![
                json!({"price": 98.0, "size": 2.0, "total": 2.0, "count": 2})
            ])
        );
        assert_eq!(
            orderbook.get("best_bid").and_then(|value| value.as_f64()),
            Some(98.0)
        );
    }

    #[test]
    fn normalize_trade_rejects_invalid_price_or_size() {
        assert!(normalize_trade(
            json!({
                "instId": "BTC-USDT-SWAP",
                "tradeId": "trade-bad-price",
                "px": "bad-price",
                "sz": "1",
                "side": "buy",
                "ts": "1780000000000",
            }),
            "",
        )
        .is_none());

        assert!(normalize_trade(
            json!({
                "instId": "BTC-USDT-SWAP",
                "tradeId": "trade-bad-size",
                "px": "100",
                "sz": "bad-size",
                "side": "sell",
                "ts": "1780000000000",
            }),
            "",
        )
        .is_none());
    }

    #[test]
    fn normalize_trade_rejects_missing_trade_id_or_invalid_side() {
        assert!(normalize_trade(
            json!({
                "instId": "BTC-USDT-SWAP",
                "px": "100",
                "sz": "1",
                "side": "buy",
                "ts": "1780000000000",
            }),
            "",
        )
        .is_none());

        assert!(normalize_trade(
            json!({
                "instId": "BTC-USDT-SWAP",
                "tradeId": "trade-bad-side",
                "px": "100",
                "sz": "1",
                "side": "hold",
                "ts": "1780000000000",
            }),
            "",
        )
        .is_none());
    }

    #[test]
    fn normalize_ticker_rejects_invalid_timestamp_instead_of_using_receive_time() {
        assert!(normalize_ticker(
            json!({
                "instId": "BTC-USDT-SWAP",
                "last": "100",
                "askPx": "101",
                "bidPx": "99",
                "open24h": "100",
                "ts": "bad-ts",
            }),
            "",
        )
        .is_none());
    }

    #[test]
    fn normalize_ticker_rejects_invalid_ask_instead_of_copying_last() {
        assert!(normalize_ticker(
            json!({
                "instId": "BTC-USDT-SWAP",
                "last": "100",
                "askPx": "bad-ask",
                "bidPx": "99",
                "open24h": "98",
                "high24h": "102",
                "low24h": "97",
                "vol24h": "1000",
                "ts": "1780000000000",
            }),
            "",
        )
        .is_none());
    }

    #[test]
    fn normalize_ticker_rejects_invalid_volume_instead_of_fabricating_zero() {
        assert!(normalize_ticker(
            json!({
                "instId": "BTC-USDT-SWAP",
                "last": "100",
                "askPx": "101",
                "bidPx": "99",
                "open24h": "98",
                "high24h": "102",
                "low24h": "97",
                "vol24h": "bad-volume",
                "ts": "1780000000000",
            }),
            "",
        )
        .is_none());
    }

    #[test]
    fn normalize_trade_rejects_invalid_timestamp_instead_of_using_receive_time() {
        assert!(normalize_trade(
            json!({
                "instId": "BTC-USDT-SWAP",
                "tradeId": "trade-bad-ts",
                "px": "100",
                "sz": "1",
                "side": "buy",
                "ts": "bad-ts",
            }),
            "",
        )
        .is_none());
    }

    #[test]
    fn normalize_orderbook_rejects_invalid_timestamp_instead_of_using_receive_time() {
        assert!(normalize_orderbook(
            json!({
                "instId": "BTC-USDT-SWAP",
                "bids": [["100", "1", "0", "1"]],
                "asks": [["101", "1", "0", "1"]],
                "ts": "bad-ts",
            }),
            "",
            "books5",
        )
        .is_none());
    }

    #[test]
    fn normalize_orderbook_rejects_snapshot_without_valid_bid_or_ask() {
        assert!(normalize_orderbook(
            json!({
                "instId": "BTC-USDT-SWAP",
                "bids": [["100", "0", "0", "1"]],
                "asks": [["101", "1", "0", "1"]],
                "ts": "1780000000000",
            }),
            "",
            "books5",
        )
        .is_none());

        assert!(normalize_orderbook(
            json!({
                "instId": "BTC-USDT-SWAP",
                "bids": [["100", "1", "0", "1"]],
                "asks": [["101", "0", "0", "1"]],
                "ts": "1780000000000",
            }),
            "",
            "books5",
        )
        .is_none());
    }
}
