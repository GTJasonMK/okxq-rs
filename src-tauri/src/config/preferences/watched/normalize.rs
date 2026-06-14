use serde_json::Value;

use super::{
    plans::{
        apply_sync_days_to_plans, infer_sync_days_from_plans, normalize_sync_days,
        normalize_sync_plans_from_value,
    },
    types::WatchedSymbolRecord,
};

pub(in crate::config::preferences) fn normalize_watched_symbols(
    value: Option<&Value>,
) -> Vec<WatchedSymbolRecord> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut records = Vec::new();
    for item in items {
        let symbol_value = item
            .get("symbol")
            .and_then(Value::as_str)
            .or_else(|| item.as_str());
        let Some(symbol) = symbol_value.and_then(normalize_watched_symbol) else {
            continue;
        };
        if !seen.insert(symbol.clone()) {
            continue;
        }
        let sync_plans = normalize_sync_plans_from_value(item.get("sync_plans"));
        let sync_days = value_i64(item, "sync_days")
            .map(normalize_sync_days)
            .unwrap_or_else(|| infer_sync_days_from_plans(&sync_plans));
        let sync_plans = apply_sync_days_to_plans(sync_plans, sync_days);
        let mut record = WatchedSymbolRecord::new(
            &symbol,
            item.get("sync_spot")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            item.get("sync_swap")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            item.get("archive_all_history")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            sync_days,
            sync_plans,
            None,
        );
        if !record.sync_spot && !record.sync_swap {
            record.sync_spot = true;
        }
        if let Some(created_at) = item.get("created_at").and_then(Value::as_str) {
            record.created_at = created_at.to_string();
        }
        if let Some(updated_at) = item.get("updated_at").and_then(Value::as_str) {
            record.updated_at = updated_at.to_string();
        }
        records.push(record);
    }
    records
}

pub(in crate::config::preferences) fn normalize_watched_symbol(value: &str) -> Option<String> {
    let mut normalized = value.trim().to_uppercase();
    if normalized.is_empty() {
        return None;
    }
    if normalized.ends_with("-SWAP") {
        normalized.truncate(normalized.len() - 5);
    }
    if !normalized.contains('-') {
        normalized = format!("{normalized}-USDT");
    }
    Some(normalized)
}

fn value_i64(item: &Value, key: &str) -> Option<i64> {
    item.get(key).and_then(Value::as_i64)
}
