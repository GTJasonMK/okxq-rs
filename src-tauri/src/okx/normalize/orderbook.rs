use serde_json::{json, Value};

use crate::orderbook_snapshot::normalize_orderbook_snapshot;

use super::values::value_string;

pub fn normalize_orderbook(book: Value, request_inst_id: &str) -> Option<Value> {
    let inst_id = value_string(&book, "instId")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| request_inst_id.to_string());
    if inst_id.is_empty() {
        return None;
    }
    let snapshot = normalize_orderbook_snapshot(&book)?;

    Some(json!({
        "inst_id": inst_id,
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
    }))
}
