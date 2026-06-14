use super::*;

pub(super) fn ensure_inventory_entry<'a>(
    entries: &'a mut BTreeMap<String, Value>,
    symbol: &str,
) -> &'a mut Value {
    entries.entry(symbol.to_string()).or_insert_with(|| {
        let base = symbol.split('-').next().unwrap_or(symbol).to_string();
        json!({
            "symbol": symbol,
            "base_ccy": base,
            "spot_inst_id": symbol,
            "swap_inst_id": format!("{symbol}-SWAP"),
            "timeframe_record_count": 0,
            "candle_count": 0,
            "markets": {},
            "storage_counts": {
                "candles": 0,
                "feature_bars_1s": 0,
                "sync_records": 0,
                "market_ticker_snapshots": 0,
                "market_recent_trades": 0,
                "local_fills": 0,
                "live_order_records": 0,
                "backtest_results": 0,
                "cost_basis": 0,
                "total": 0
            }
        })
    })
}

pub(super) fn add_i64_field(obj: &mut Map<String, Value>, key: &str, delta: i64) {
    let next = obj.get(key).and_then(Value::as_i64).unwrap_or(0) + delta;
    obj.insert(key.to_string(), Value::Number(next.into()));
}

pub(super) fn ensure_inventory_market<'a>(
    entry: &'a mut Value,
    inst_type: &str,
    inst_id: &str,
    managed: bool,
    watched: bool,
) -> &'a mut Map<String, Value> {
    let obj = entry.as_object_mut().expect("inventory entry object");
    let markets = obj
        .entry("markets".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .expect("markets object");
    let market = markets.entry(inst_type.to_string()).or_insert_with(|| {
        json!({
            "inst_id": inst_id,
            "inst_type": inst_type,
            "managed": managed,
            "watched": watched,
            "timeframe_count": 0,
            "candle_count": 0,
            "gap_count": 0,
            "history_complete_count": 0,
            "last_sync_time": null,
            "timeframes": []
        })
    });
    let market_obj = market.as_object_mut().expect("market object");
    let next_managed = market_obj
        .get("managed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || managed;
    let next_watched = market_obj
        .get("watched")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || watched;
    market_obj.insert("inst_id".to_string(), Value::String(inst_id.to_string()));
    market_obj.insert(
        "inst_type".to_string(),
        Value::String(inst_type.to_string()),
    );
    market_obj.insert("managed".to_string(), Value::Bool(next_managed));
    market_obj.insert("watched".to_string(), Value::Bool(next_watched));
    market_obj
}
