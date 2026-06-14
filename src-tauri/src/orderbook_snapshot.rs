use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NormalizedOrderbookSnapshot {
    pub(crate) asks: Vec<Value>,
    pub(crate) bids: Vec<Value>,
    pub(crate) best_ask: f64,
    pub(crate) best_bid: f64,
    pub(crate) mid_price: f64,
    pub(crate) spread: f64,
    pub(crate) spread_rate: f64,
    pub(crate) ask_depth_total: f64,
    pub(crate) bid_depth_total: f64,
    pub(crate) timestamp: i64,
}

pub(crate) fn normalize_orderbook_snapshot(book: &Value) -> Option<NormalizedOrderbookSnapshot> {
    let asks = normalize_orderbook_levels(book.get("asks"));
    let bids = normalize_orderbook_levels(book.get("bids"));
    let best_ask = asks
        .first()
        .and_then(|item| item.get("price"))
        .and_then(Value::as_f64)?;
    let best_bid = bids
        .first()
        .and_then(|item| item.get("price"))
        .and_then(Value::as_f64)?;
    if !best_ask.is_finite() || best_ask <= 0.0 || !best_bid.is_finite() || best_bid <= 0.0 {
        return None;
    }

    let spread = best_ask - best_bid;
    let mid_price = (best_ask + best_bid) / 2.0;
    let timestamp =
        parse_i64(book.get("ts").or_else(|| book.get("timestamp"))).filter(|value| *value > 0)?;

    Some(NormalizedOrderbookSnapshot {
        ask_depth_total: depth_total(&asks),
        bid_depth_total: depth_total(&bids),
        asks,
        bids,
        best_ask,
        best_bid,
        mid_price,
        spread,
        spread_rate: spread / mid_price * 100.0,
        timestamp,
    })
}

fn normalize_orderbook_levels(levels: Option<&Value>) -> Vec<Value> {
    let Some(items) = levels.and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut total = 0.0;
    items
        .iter()
        .filter_map(|item| normalize_orderbook_level(item, &mut total))
        .collect()
}

fn normalize_orderbook_level(level: &Value, total: &mut f64) -> Option<Value> {
    let items = level.as_array()?;
    let price = parse_f64(items.first())?;
    let size = parse_f64(items.get(1))?;
    if !price.is_finite() || price <= 0.0 || !size.is_finite() || size <= 0.0 {
        return None;
    }
    let count = parse_i64(items.get(3)).filter(|value| *value >= 0)?;

    *total += size;
    Some(json!({
        "price": price,
        "size": size,
        "total": *total,
        "count": count,
    }))
}

fn depth_total(levels: &[Value]) -> f64 {
    levels
        .last()
        .and_then(|item| item.get("total"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

fn parse_f64(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(item) => item.as_f64(),
        Value::String(item) => item.parse::<f64>().ok(),
        _ => None,
    }
}

fn parse_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(item) => item.as_i64(),
        Value::String(item) => item.parse::<i64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::normalize_orderbook_snapshot;

    #[test]
    fn orderbook_snapshot_rejects_missing_valid_side() {
        assert!(normalize_orderbook_snapshot(&json!({
            "bids": [["100", "0", "0", "1"]],
            "asks": [["101", "1", "0", "1"]],
            "ts": "1700000000000",
        }))
        .is_none());
    }

    #[test]
    fn orderbook_snapshot_rejects_missing_order_count() {
        assert!(normalize_orderbook_snapshot(&json!({
            "bids": [["100", "1", "7"]],
            "asks": [["101", "2", "8"]],
            "ts": "1700000000000",
        }))
        .is_none());
    }

    #[test]
    fn orderbook_snapshot_preserves_four_column_order_count() {
        let snapshot = normalize_orderbook_snapshot(&json!({
            "bids": [["100", "1", "0", "7"]],
            "asks": [["101", "2", "0", "8"]],
            "ts": "1700000000000",
        }))
        .expect("four-column snapshot should be valid");

        assert_eq!(snapshot.bids[0]["count"].as_i64(), Some(7));
        assert_eq!(snapshot.asks[0]["count"].as_i64(), Some(8));
    }
}
