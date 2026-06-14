use serde_json::{json, Value};

use super::values::{parse_f64, parse_i64, value_string};

pub fn normalize_trade(trade: Value, request_inst_id: &str) -> Option<Value> {
    let inst_id = value_string(&trade, "instId")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| request_inst_id.to_string())
        .trim()
        .to_uppercase();
    if inst_id.is_empty() {
        return None;
    }
    let price = parse_positive_f64(trade.get("px").unwrap_or(&Value::Null))?;
    let size = parse_positive_f64(trade.get("sz").unwrap_or(&Value::Null))?;
    let trade_id = value_string(&trade, "tradeId")?.trim().to_string();
    if trade_id.is_empty() {
        return None;
    }
    let side = normalize_trade_side(&trade)?;
    let timestamp = parse_i64(trade.get("ts").unwrap_or(&Value::Null))?;
    if timestamp <= 0 {
        return None;
    }

    Some(json!({
        "inst_id": inst_id,
        "trade_id": trade_id,
        "price": price,
        "size": size,
        "side": side,
        "ts": timestamp,
    }))
}

fn normalize_trade_side(trade: &Value) -> Option<&'static str> {
    match value_string(trade, "side")?
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "buy" => Some("buy"),
        "sell" => Some("sell"),
        _ => None,
    }
}

fn parse_positive_f64(value: &Value) -> Option<f64> {
    let parsed = parse_f64(value)?;
    (parsed.is_finite() && parsed > 0.0).then_some(parsed)
}
