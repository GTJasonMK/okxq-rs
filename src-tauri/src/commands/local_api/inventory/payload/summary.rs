use super::*;

pub(super) fn finalize_inventory_payload(
    entries: BTreeMap<String, Value>,
    watched_symbols: &std::collections::BTreeSet<String>,
    watched_market_count: usize,
    deletion_marked_symbols: &std::collections::BTreeSet<String>,
) -> Value {
    let mut rows = Vec::new();
    let mut table_totals = BTreeMap::<String, i64>::new();
    let mut total_candles = 0;
    let mut total_timeframe_records = 0;
    let mut orphan_count = 0;
    let mut covered_watched_count = 0;
    let mut managed_symbol_count = 0;
    let mut managed_market_count = 0;

    for (symbol, mut row) in entries {
        let watched = watched_symbols.contains(&symbol);
        if let Some(obj) = row.as_object_mut() {
            let enabled_markets = managed_markets(obj);
            let row_managed_market_count = enabled_markets.len();
            let managed = row_managed_market_count > 0;
            let storage_total = add_storage_total(obj, &mut table_totals);
            let deletion_marked = deletion_marked_symbols.contains(&symbol);
            let orphan = deletion_marked && storage_total > 0;

            if watched {
                covered_watched_count += 1;
            }
            if managed {
                managed_symbol_count += 1;
                managed_market_count += row_managed_market_count;
            }
            if orphan {
                orphan_count += 1;
            }

            obj.insert("watched".to_string(), Value::Bool(watched));
            obj.insert("managed".to_string(), Value::Bool(managed));
            obj.insert("orphan".to_string(), Value::Bool(orphan));
            obj.insert("deletion_marked".to_string(), Value::Bool(deletion_marked));
            obj.insert("enabled_markets".to_string(), Value::Array(enabled_markets));

            let candle_count = obj.get("candle_count").and_then(Value::as_i64).unwrap_or(0);
            let timeframe_records = obj
                .get("timeframe_record_count")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            total_candles += candle_count;
            total_timeframe_records += timeframe_records;
        }
        rows.push(row);
    }

    rows.sort_by(inventory_row_order);

    json!({
        "summary": {
            "symbol_count": rows.len(),
            "managed_symbol_count": managed_symbol_count,
            "managed_market_count": managed_market_count,
            "watched_symbol_count": covered_watched_count,
            "watched_list_count": watched_symbols.len(),
            "watched_market_count": watched_market_count,
            "orphan_symbol_count": orphan_count,
            "total_candles": total_candles,
            "total_timeframe_records": total_timeframe_records,
            "table_totals": table_totals
        },
        "rows": rows
    })
}

fn managed_markets(obj: &Map<String, Value>) -> Vec<Value> {
    obj.get("markets")
        .and_then(Value::as_object)
        .map(|markets| {
            markets
                .iter()
                .filter_map(|(inst_type, market)| {
                    if market
                        .get("managed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        Some(Value::String(inst_type.clone()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn add_storage_total(
    obj: &mut Map<String, Value>,
    table_totals: &mut BTreeMap<String, i64>,
) -> i64 {
    if let Some(counts) = obj.get_mut("storage_counts").and_then(Value::as_object_mut) {
        let total = counts
            .iter()
            .filter(|(key, _)| key.as_str() != "total")
            .map(|(_, value)| value.as_i64().unwrap_or(0))
            .sum::<i64>();
        counts.insert("total".to_string(), Value::Number(total.into()));
        for (key, value) in counts.iter() {
            *table_totals.entry(key.clone()).or_insert(0) += value.as_i64().unwrap_or(0);
        }
        return total;
    }
    0
}

fn inventory_row_order(left: &Value, right: &Value) -> std::cmp::Ordering {
    let left_total = left
        .get("storage_counts")
        .and_then(|value| value.get("total"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let right_total = right
        .get("storage_counts")
        .and_then(|value| value.get("total"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    right_total.cmp(&left_total).then_with(|| {
        left.get("symbol")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .cmp(
                right
                    .get("symbol")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use serde_json::json;

    use super::*;

    #[test]
    fn unmanaged_unwatched_inventory_is_not_orphan_without_deletion_mark() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "ETH-USDT".to_string(),
            inventory_entry("ETH-USDT", false, false, 42),
        );

        let payload = finalize_inventory_payload(entries, &BTreeSet::new(), 0, &BTreeSet::new());
        let summary = payload.get("summary").expect("summary");
        let row = payload
            .get("rows")
            .and_then(Value::as_array)
            .and_then(|rows| rows.first())
            .expect("inventory row");

        assert_eq!(
            summary.get("orphan_symbol_count").and_then(Value::as_i64),
            Some(0)
        );
        assert_eq!(row.get("orphan").and_then(Value::as_bool), Some(false));
        assert_eq!(
            row.get("deletion_marked").and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn deletion_marked_inventory_residual_is_orphan() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "ETH-USDT".to_string(),
            inventory_entry("ETH-USDT", false, false, 42),
        );
        let deletion_marks = BTreeSet::from(["ETH-USDT".to_string()]);

        let payload = finalize_inventory_payload(entries, &BTreeSet::new(), 0, &deletion_marks);
        let summary = payload.get("summary").expect("summary");
        let row = payload
            .get("rows")
            .and_then(Value::as_array)
            .and_then(|rows| rows.first())
            .expect("inventory row");

        assert_eq!(
            summary.get("orphan_symbol_count").and_then(Value::as_i64),
            Some(1)
        );
        assert_eq!(row.get("orphan").and_then(Value::as_bool), Some(true));
        assert_eq!(
            row.get("deletion_marked").and_then(Value::as_bool),
            Some(true)
        );
    }

    fn inventory_entry(symbol: &str, managed: bool, watched: bool, candle_count: i64) -> Value {
        json!({
            "symbol": symbol,
            "base_ccy": symbol.split('-').next().unwrap_or(symbol),
            "spot_inst_id": symbol,
            "swap_inst_id": format!("{symbol}-SWAP"),
            "timeframe_record_count": 0,
            "candle_count": 0,
            "markets": {
                "SWAP": {
                    "inst_id": format!("{symbol}-SWAP"),
                    "inst_type": "SWAP",
                    "managed": managed,
                    "watched": watched,
                    "timeframe_count": 0,
                    "candle_count": candle_count,
                    "gap_count": 0,
                    "history_complete_count": 0,
                    "timeframes": []
                }
            },
            "storage_counts": {
                "candles": candle_count,
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
    }
}
