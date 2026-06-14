use serde_json::{json, Map, Value};

use crate::{
    error::AppResult,
    okx::OkxPrivateClient,
    trading_semantics::{normalize_runtime_order_type_text, position_side_from_okx_position},
};

pub(crate) async fn fetch_private_account_context(
    client: &OkxPrivateClient,
    mode: &str,
    initial_capital: f64,
) -> AppResult<Value> {
    let items = client.get_account_balance().await?;
    Ok(account_context_from_private_items_with_source(
        items,
        mode,
        initial_capital,
        "okx_private_rest",
    ))
}

pub(crate) async fn fetch_private_positions_context(
    client: &OkxPrivateClient,
    mode: &str,
    inst_type: &str,
) -> AppResult<Value> {
    let normalized_inst_type = inst_type.trim().to_uppercase();
    let items = client
        .get_positions(
            (!normalized_inst_type.is_empty()).then_some(normalized_inst_type.as_str()),
            None,
        )
        .await?;
    Ok(positions_context_from_private_items_with_source(
        items,
        mode,
        "okx_private_rest",
    ))
}

pub(crate) async fn fetch_private_orders_context(
    client: &OkxPrivateClient,
    mode: &str,
    inst_type: &str,
) -> AppResult<Value> {
    let normalized_inst_type = inst_type.trim().to_uppercase();
    let inst_type = (!normalized_inst_type.is_empty()).then_some(normalized_inst_type.as_str());
    let open_orders = client.get_pending_orders(inst_type, None).await?;
    let recent_fills = client.get_fills(inst_type, None, 50).await?;
    let order_history = client.get_order_history(inst_type, None, 50).await?;
    Ok(orders_context_from_private_items_with_source(
        open_orders,
        recent_fills,
        order_history,
        mode,
        "okx_private_rest",
    ))
}

#[cfg(test)]
pub(crate) fn account_context_from_private_items(
    items: Vec<Value>,
    mode: &str,
    initial_capital: f64,
) -> Value {
    account_context_from_private_items_with_source(items, mode, initial_capital, "okx_private_rest")
}

pub(crate) fn account_context_from_private_items_with_source(
    items: Vec<Value>,
    mode: &str,
    initial_capital: f64,
    source: &str,
) -> Value {
    let Some(account) = items.first() else {
        return json!({
            "mode": mode,
            "source": source,
            "initial_capital": initial_capital,
            "cash": null,
            "equity": null,
            "total_equity": null,
            "total_eq": null,
            "iso_eq": null,
            "adj_eq": null,
            "usdt_balance": null,
            "usdt_available": null,
            "usdt_equity_usd": null,
            "details": [],
        });
    };
    let raw_details = account
        .get("details")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let detail_total_equity = sum_finite(
        raw_details
            .iter()
            .filter_map(|item| first_f64(item, &["eqUsd", "disEq", "eq"])),
    );
    let total_equity = value_finite_f64(account, "totalEq").or(detail_total_equity);
    let iso_equity = value_finite_f64(account, "isoEq");
    let adjusted_equity = value_finite_f64(account, "adjEq");
    let usdt_detail = raw_details
        .iter()
        .find(|item| value_string(item, "ccy", "").eq_ignore_ascii_case("USDT"));
    let usdt_balance =
        usdt_detail.and_then(|item| first_f64(item, &["cashBal", "eq", "availBal", "availEq"]));
    let usdt_available =
        usdt_detail.and_then(|item| first_f64(item, &["availBal", "availEq", "cashBal", "eq"]));
    let usdt_equity_usd = usdt_detail.and_then(|item| first_f64(item, &["eqUsd", "disEq"]));
    let details = raw_details
        .into_iter()
        .map(|item| {
            json!({
                "ccy": value_string(&item, "ccy", ""),
                "avail_bal": value_finite_f64(&item, "availBal"),
                "avail_eq": value_finite_f64(&item, "availEq"),
                "frozen_bal": value_finite_f64(&item, "frozenBal"),
                "ord_frozen": value_finite_f64(&item, "ordFrozen"),
                "cash_bal": value_finite_f64(&item, "cashBal"),
                "eq": value_finite_f64(&item, "eq"),
                "eq_usd": value_finite_f64(&item, "eqUsd"),
                "dis_eq": value_finite_f64(&item, "disEq"),
                "u_time": value_string(&item, "uTime", ""),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "mode": mode,
        "source": source,
        "initial_capital": initial_capital,
        "cash": usdt_available,
        "equity": total_equity,
        "total_equity": total_equity,
        "total_eq": total_equity,
        "iso_eq": iso_equity,
        "adj_eq": adjusted_equity,
        "usdt_balance": usdt_balance,
        "usdt_available": usdt_available,
        "usdt_equity_usd": usdt_equity_usd,
        "details": details,
        "raw": account,
    })
}

#[cfg(test)]
pub(crate) fn positions_context_from_private_items(items: Vec<Value>, mode: &str) -> Value {
    positions_context_from_private_items_with_source(items, mode, "okx_private_rest")
}

pub(crate) fn positions_context_from_private_items_with_source(
    items: Vec<Value>,
    mode: &str,
    source: &str,
) -> Value {
    let mut positions = Map::new();
    let mut open = Vec::new();
    for item in items {
        let inst_id = value_string(&item, "instId", "").trim().to_uppercase();
        if inst_id.is_empty() {
            continue;
        }
        let pos = value_f64(&item, "pos", 0.0);
        if !pos.is_finite() || pos.abs() <= f64::EPSILON {
            continue;
        }
        let side = normalized_position_side(&item, pos);
        let entry_price = value_positive_f64(&item, "avgPx");
        let mark_price = value_positive_f64(&item, "markPx");
        let unrealized_pnl = value_finite_f64(&item, "upl");
        let unrealized_pnl_pct = value_finite_f64(&item, "uplRatio");
        let leverage = value_positive_f64(&item, "lever");
        let liq_px = value_positive_f64(&item, "liqPx");
        let margin = first_non_negative_f64(&item, &["margin", "imr"]);
        let row = json!({
            "mode": mode,
            "source": source,
            "symbol": inst_id.clone(),
            "inst_id": inst_id.clone(),
            "inst_type": value_string(&item, "instType", ""),
            "side": side,
            "pos_side": side,
            "quantity": pos.abs(),
            "pos": pos,
            "entry_price": entry_price,
            "avg_px": entry_price,
            "mark_price": mark_price,
            "mark_px": mark_price,
            "unrealized_pnl": unrealized_pnl,
            "upl": unrealized_pnl,
            "unrealized_pnl_pct": unrealized_pnl_pct,
            "upl_ratio": unrealized_pnl_pct,
            "lever": leverage,
            "liq_px": liq_px,
            "margin": margin,
            "mgn_mode": value_string(&item, "mgnMode", ""),
            "c_time": value_i64(&item, "cTime", 0),
            "u_time": value_i64(&item, "uTime", 0),
            "raw": item,
        });
        positions.insert(inst_id.clone(), row.clone());
        positions.insert(format!("{inst_id}:{side}"), row.clone());
        open.push(row);
    }
    positions.insert("open".to_string(), Value::Array(open));
    Value::Object(positions)
}

#[cfg(test)]
pub(crate) fn orders_context_from_private_items(
    open_orders: Vec<Value>,
    recent_fills: Vec<Value>,
    order_history: Vec<Value>,
    mode: &str,
) -> Value {
    orders_context_from_private_items_with_source(
        open_orders,
        recent_fills,
        order_history,
        mode,
        "okx_private_rest",
    )
}

pub(crate) fn orders_context_from_private_items_with_source(
    open_orders: Vec<Value>,
    recent_fills: Vec<Value>,
    order_history: Vec<Value>,
    mode: &str,
    source: &str,
) -> Value {
    let open = open_orders
        .into_iter()
        .map(|item| private_order_context_row(item, mode, source))
        .collect::<Vec<_>>();
    let recent_fills = recent_fills
        .into_iter()
        .map(|item| private_fill_context_row(item, mode, source))
        .collect::<Vec<_>>();
    let recent_rejections = order_history
        .into_iter()
        .filter(|item| private_order_bucket(item).is_rejection())
        .map(|item| private_order_context_row(item, mode, source))
        .collect::<Vec<_>>();

    json!({
        "source": source,
        "mode": mode,
        "open": open,
        "recent_fills": recent_fills,
        "recent_rejections": recent_rejections,
    })
}

pub(crate) fn orders_context_from_private_stream_items(
    order_events: Vec<Value>,
    algo_order_events: Vec<Value>,
    recent_fills: Vec<Value>,
    mode: &str,
    source: &str,
) -> Value {
    let mut open = order_events
        .iter()
        .filter(|item| private_order_bucket(item).is_open())
        .cloned()
        .map(|item| private_order_context_row(item, mode, source))
        .collect::<Vec<_>>();
    open.extend(
        algo_order_events
            .iter()
            .filter(|item| private_algo_order_bucket(item).is_open())
            .cloned()
            .map(|item| private_algo_order_context_row(item, mode, source)),
    );

    let mut recent_rejections = order_events
        .into_iter()
        .filter(|item| private_order_bucket(item).is_rejection())
        .map(|item| private_order_context_row(item, mode, source))
        .collect::<Vec<_>>();
    recent_rejections.extend(
        algo_order_events
            .iter()
            .filter(|item| private_algo_order_bucket(item).is_rejection())
            .cloned()
            .map(|item| private_algo_order_context_row(item, mode, source)),
    );

    let mut recent_fills = recent_fills
        .into_iter()
        .map(|item| private_fill_context_row(item, mode, source))
        .collect::<Vec<_>>();
    recent_fills.extend(
        algo_order_events
            .into_iter()
            .filter(|item| private_algo_order_bucket(item).is_fill())
            .map(|item| private_algo_order_context_row(item, mode, source)),
    );

    json!({
        "source": source,
        "mode": mode,
        "open": open,
        "recent_fills": recent_fills,
        "recent_rejections": recent_rejections,
    })
}

pub(crate) fn private_order_stream_cache_available(
    order_events: &Option<Vec<Value>>,
    algo_events: &Option<Vec<Value>>,
    fill_events: &Option<Vec<Value>>,
) -> bool {
    [order_events, algo_events, fill_events]
        .into_iter()
        .any(|items| items.as_ref().is_some_and(|items| !items.is_empty()))
}

#[cfg(test)]
pub(crate) fn merge_order_contexts(primary: Value, secondary: Value) -> Value {
    merge_order_contexts_with_source(primary, secondary, "okx_private_rest+live_order_records")
}

pub(crate) fn merge_order_contexts_with_source(
    primary: Value,
    secondary: Value,
    merged_source: &str,
) -> Value {
    let mut object = secondary.as_object().cloned().unwrap_or_default();
    for key in ["open", "recent_fills", "recent_rejections"] {
        let rows = merge_order_context_bucket(primary.get(key), secondary.get(key), merged_source);
        object.insert(key.to_string(), Value::Array(rows));
    }
    object.insert("source".to_string(), json!(merged_source));
    if let Some(mode) = primary.get("mode").cloned() {
        object.insert("mode".to_string(), mode);
    }
    Value::Object(object)
}

pub(crate) fn merge_private_order_contexts_with_source(
    primary: Value,
    secondary: Value,
    merged_source: &str,
) -> Value {
    let mut object = secondary.as_object().cloned().unwrap_or_default();
    for key in ["open", "recent_fills", "recent_rejections"] {
        let rows =
            merge_private_order_context_bucket(primary.get(key), secondary.get(key), merged_source);
        object.insert(key.to_string(), Value::Array(rows));
    }
    object.insert("source".to_string(), json!(merged_source));
    if let Some(mode) = primary.get("mode").cloned() {
        object.insert("mode".to_string(), mode);
    }
    Value::Object(object)
}

fn merge_order_context_bucket(
    primary: Option<&Value>,
    secondary: Option<&Value>,
    merged_source: &str,
) -> Vec<Value> {
    let mut rows = primary
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let primary_len = rows.len();
    for local in secondary.and_then(Value::as_array).into_iter().flatten() {
        if let Some(index) = matching_primary_order_index(&rows[..primary_len], local) {
            rows[index] =
                merge_private_and_local_order_row(rows[index].clone(), local, merged_source);
        } else {
            rows.push(local.clone());
        }
    }
    rows
}

fn merge_private_order_context_bucket(
    primary: Option<&Value>,
    secondary: Option<&Value>,
    merged_source: &str,
) -> Vec<Value> {
    let mut rows = primary
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let primary_len = rows.len();
    for secondary_row in secondary.and_then(Value::as_array).into_iter().flatten() {
        if let Some(index) = matching_primary_order_index(&rows[..primary_len], secondary_row) {
            rows[index] =
                merge_private_order_rows(rows[index].clone(), secondary_row, merged_source);
        } else {
            rows.push(secondary_row.clone());
        }
    }
    rows
}

fn matching_primary_order_index(rows: &[Value], local: &Value) -> Option<usize> {
    let local_identities = order_identity_values(local);
    if local_identities.is_empty() {
        return None;
    }
    rows.iter().position(|row| {
        order_context_scope_matches(row, local)
            && order_identity_values(row)
                .iter()
                .any(|item| local_identities.contains(item))
    })
}

fn order_context_scope_matches(primary: &Value, local: &Value) -> bool {
    let primary_inst_id = order_context_inst_id(primary);
    let local_inst_id = order_context_inst_id(local);
    if primary_inst_id.is_empty() || local_inst_id.is_empty() || primary_inst_id != local_inst_id {
        return false;
    }

    let primary_mode = value_string(primary, "mode", "")
        .trim()
        .to_ascii_lowercase();
    let local_mode = value_string(local, "mode", "").trim().to_ascii_lowercase();
    primary_mode.is_empty() || local_mode.is_empty() || primary_mode == local_mode
}

fn order_context_inst_id(row: &Value) -> String {
    first_string(row, &["inst_id", "symbol", "instId"])
        .trim()
        .to_ascii_uppercase()
}

const ORDER_IDENTITY_FIELDS: [(&str, &str); 9] = [
    ("ord", "order_id"),
    ("ord", "actual_order_id"),
    ("ord", "algo_id"),
    ("ord", "algoId"),
    ("cl", "client_order_id"),
    ("cl", "actual_client_order_id"),
    ("cl", "algo_client_order_id"),
    ("cl", "algoClOrdId"),
    ("trade", "trade_id"),
];

fn order_identity_values(row: &Value) -> Vec<String> {
    let mut values = Vec::new();
    for (kind, key) in ORDER_IDENTITY_FIELDS {
        push_identity_value(&mut values, kind, value_string(row, key, ""));
    }
    values.sort();
    values.dedup();
    values
}

fn push_identity_value(values: &mut Vec<String>, kind: &str, value: String) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    let normalized = if kind == "ord" {
        value.to_ascii_uppercase()
    } else {
        value.to_string()
    };
    values.push(format!("{kind}:{normalized}"));
}

fn merge_private_and_local_order_row(
    mut private: Value,
    local: &Value,
    merged_source: &str,
) -> Value {
    let Some(private_object) = private.as_object_mut() else {
        return private;
    };
    let Some(local_object) = local.as_object() else {
        return private;
    };

    private_object.insert("source".to_string(), json!(merged_source));
    copy_if_missing_or_empty(private_object, local_object, "action");
    copy_if_missing_or_empty(private_object, local_object, "strategy_id");
    copy_if_missing_or_empty(private_object, local_object, "strategy_name");
    copy_if_missing_or_empty(private_object, local_object, "run_id");
    copy_if_missing_or_empty(private_object, local_object, "actual_order_id");
    copy_if_missing_or_empty(private_object, local_object, "actual_client_order_id");
    copy_if_missing_or_empty(private_object, local_object, "arrival_ts");
    copy_if_missing_or_empty(private_object, local_object, "arrival_mid_px");
    copy_if_missing_or_empty(private_object, local_object, "arrival_bid_px");
    copy_if_missing_or_empty(private_object, local_object, "arrival_ask_px");
    copy_if_missing_or_empty(private_object, local_object, "error_message");

    copy_local_metadata(private_object, local_object, "id", "local_order_record_id");
    copy_local_metadata(private_object, local_object, "status", "local_status");
    copy_local_metadata(private_object, local_object, "success", "local_success");
    copy_local_metadata(
        private_object,
        local_object,
        "error_message",
        "local_error_message",
    );
    copy_local_metadata(private_object, local_object, "timestamp", "local_timestamp");
    copy_local_metadata(
        private_object,
        local_object,
        "created_at",
        "local_created_at",
    );

    private
}

fn merge_private_order_rows(mut primary: Value, secondary: &Value, merged_source: &str) -> Value {
    let Some(primary_object) = primary.as_object_mut() else {
        return primary;
    };
    let Some(secondary_object) = secondary.as_object() else {
        return primary;
    };

    for (key, value) in secondary_object {
        if !private_order_context_value_missing(primary_object.get(key))
            || private_order_context_value_missing(Some(value))
        {
            continue;
        }
        primary_object.insert(key.clone(), value.clone());
    }
    primary_object.insert("source".to_string(), json!(merged_source));

    primary
}

fn private_order_context_value_missing(value: Option<&Value>) -> bool {
    matches!(value, None | Some(Value::Null))
        || matches!(value, Some(Value::String(item)) if item.trim().is_empty())
}

fn copy_if_missing_or_empty(
    target: &mut Map<String, Value>,
    source: &Map<String, Value>,
    key: &str,
) {
    if !value_is_missing_or_empty(target.get(key)) {
        return;
    }
    let Some(value) = source
        .get(key)
        .filter(|value| !value_is_missing_or_empty(Some(value)))
    else {
        return;
    };
    target.insert(key.to_string(), value.clone());
}

fn copy_local_metadata(
    target: &mut Map<String, Value>,
    source: &Map<String, Value>,
    source_key: &str,
    target_key: &str,
) {
    let Some(value) = source
        .get(source_key)
        .filter(|value| !value_is_missing_or_empty(Some(value)))
    else {
        return;
    };
    target.insert(target_key.to_string(), value.clone());
}

fn value_is_missing_or_empty(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::String(value)) => value.trim().is_empty(),
        Some(Value::Number(value)) => value.as_i64() == Some(0),
        _ => false,
    }
}

fn normalized_position_side(item: &Value, pos: f64) -> &'static str {
    position_side_from_okx_position(&value_string(item, "posSide", ""), pos)
        .map(|side| side.as_str())
        .unwrap_or("long")
}

enum PrivateOrderBucket {
    Open,
    Fill,
    Rejection,
}

impl PrivateOrderBucket {
    fn is_open(&self) -> bool {
        matches!(self, Self::Open)
    }

    fn is_fill(&self) -> bool {
        matches!(self, Self::Fill)
    }

    fn is_rejection(&self) -> bool {
        matches!(self, Self::Rejection)
    }
}

fn private_algo_order_bucket(item: &Value) -> PrivateOrderBucket {
    match normalized_algo_context_status(item).as_deref() {
        Some("algo_live") => PrivateOrderBucket::Open,
        Some("algo_effective") | Some("algo_partially_effective") => PrivateOrderBucket::Fill,
        _ => PrivateOrderBucket::Rejection,
    }
}

fn normalized_algo_context_status(item: &Value) -> Option<String> {
    match value_string(item, "state", "")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "live" | "algo_live" => Some("algo_live".to_string()),
        "effective" | "algo_effective" => Some("algo_effective".to_string()),
        "partially_effective"
        | "partially-effective"
        | "algo_partially_effective"
        | "algo_partially-effective" => Some("algo_partially_effective".to_string()),
        "canceled" | "cancelled" | "algo_canceled" | "algo_cancelled" => {
            Some("algo_canceled".to_string())
        }
        "order_failed" | "failed" | "fail" | "algo_failed" => Some("algo_failed".to_string()),
        _ => None,
    }
}

fn private_order_bucket(item: &Value) -> PrivateOrderBucket {
    match value_string(item, "state", "")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "live" | "pending" | "open" | "partially_filled" | "partial-filled"
        | "partially-filled" => PrivateOrderBucket::Open,
        "filled" | "fully_filled" | "fully-filled" => PrivateOrderBucket::Fill,
        _ => PrivateOrderBucket::Rejection,
    }
}

fn private_order_context_row(item: Value, mode: &str, source: &str) -> Value {
    let inst_id = value_string(&item, "instId", "").trim().to_uppercase();
    let state = value_string(&item, "state", "");
    let bucket = private_order_bucket(&item);
    let price = first_positive_f64(&item, &["px", "avgPx", "fillPx"]);
    let size = value_positive_f64(&item, "sz");
    let filled_size = value_non_negative_f64(&item, "fillSz");
    let avg_price = value_positive_f64(&item, "avgPx");
    let pnl = value_finite_f64(&item, "pnl");
    let created_ts = value_i64(&item, "cTime", 0);
    let order_type = value_string(&item, "ordType", "");
    json!({
        "source": source,
        "mode": mode,
        "symbol": inst_id,
        "inst_id": inst_id,
        "inst_type": value_string(&item, "instType", ""),
        "order_id": value_string(&item, "ordId", ""),
        "client_order_id": value_string(&item, "clOrdId", ""),
        "side": normalized_order_side(&item),
        "order_type": order_type,
        "action": private_order_action(&order_type),
        "price": price,
        "size": size,
        "quantity": size,
        "value": finite_product(price, size),
        "filled_size": filled_size,
        "avg_price": avg_price,
        "pnl": pnl,
        "success": !bucket.is_rejection(),
        "status": if state.trim().is_empty() { "unknown" } else { state.as_str() },
        "error_message": if bucket.is_rejection() { state.as_str() } else { "" },
        "timestamp": created_ts,
        "created_ts": created_ts,
        "updated_ts": value_i64(&item, "uTime", 0),
        "raw": item,
    })
}

fn private_algo_order_context_row(item: Value, mode: &str, source: &str) -> Value {
    let inst_id = value_string(&item, "instId", "").trim().to_uppercase();
    let status = normalized_algo_context_status(&item).unwrap_or_else(|| {
        let state = value_string(&item, "state", "");
        if state.trim().is_empty() {
            "unknown".to_string()
        } else {
            state
        }
    });
    let bucket = private_algo_order_bucket(&item);
    let actual_order_id = first_string(&item, &["actualOrdId", "ordId"]);
    let actual_client_order_id = first_string(&item, &["actualClOrdId", "clOrdId"]);
    let price = first_positive_f64(
        &item,
        &["actualPx", "triggerPx", "slTriggerPx", "tpTriggerPx", "px"],
    );
    let size = first_positive_f64(&item, &["actualSz", "sz"]);
    let created_ts = value_i64(&item, "cTime", 0);
    let updated_ts = value_i64(&item, "uTime", created_ts);
    let fail_message = algo_order_error_message(&item, &status);
    json!({
        "source": source,
        "mode": mode,
        "symbol": inst_id,
        "inst_id": inst_id,
        "inst_type": value_string(&item, "instType", ""),
        "order_id": value_string(&item, "algoId", ""),
        "client_order_id": value_string(&item, "algoClOrdId", ""),
        "algo_id": value_string(&item, "algoId", ""),
        "algo_client_order_id": value_string(&item, "algoClOrdId", ""),
        "actual_order_id": actual_order_id,
        "actual_client_order_id": actual_client_order_id,
        "side": first_string(&item, &["side", "actualSide"]).to_ascii_lowercase(),
        "order_type": value_string(&item, "ordType", ""),
        "action": "place_risk_order",
        "price": price,
        "size": size,
        "quantity": size,
        "value": finite_product(price, size),
        "filled_size": size,
        "avg_price": price,
        "pnl": Value::Null,
        "success": !bucket.is_rejection(),
        "status": status,
        "error_message": if bucket.is_rejection() { fail_message } else { String::new() },
        "timestamp": created_ts,
        "created_ts": created_ts,
        "updated_ts": updated_ts,
        "raw": item,
    })
}

fn algo_order_error_message(item: &Value, status: &str) -> String {
    match status {
        "algo_canceled" => "OKX protective algo order canceled".to_string(),
        "algo_failed" => {
            let code = value_string(item, "failCode", "");
            let reason = first_string(item, &["failReason", "sMsg"]);
            format!("OKX protective algo order failed: code={code}; reason={reason}")
        }
        _ => status.to_string(),
    }
}

fn private_fill_context_row(item: Value, mode: &str, source: &str) -> Value {
    let inst_id = value_string(&item, "instId", "").trim().to_uppercase();
    let price = value_positive_f64(&item, "fillPx");
    let quantity = value_positive_f64(&item, "fillSz");
    let fee = value_finite_f64(&item, "fee");
    let timestamp = value_i64(&item, "ts", 0);
    json!({
        "source": source,
        "mode": mode,
        "symbol": inst_id,
        "inst_id": inst_id,
        "inst_type": value_string(&item, "instType", ""),
        "trade_id": value_string(&item, "tradeId", ""),
        "order_id": value_string(&item, "ordId", ""),
        "side": normalized_order_side(&item),
        "order_type": "",
        "action": "",
        "price": price,
        "size": quantity,
        "quantity": quantity,
        "value": finite_product(price, quantity),
        "fee": fee,
        "fee_ccy": value_string(&item, "feeCcy", ""),
        "timestamp": timestamp,
        "success": true,
        "status": "filled",
        "error_message": "",
        "raw": item,
    })
}

fn private_order_action(order_type: &str) -> &'static str {
    match normalize_runtime_order_type_text(order_type).as_str() {
        "conditional" | "oco" | "move_order_stop" | "trigger" | "stop_market"
        | "take_profit_market" => "place_risk_order",
        _ => "",
    }
}

fn normalized_order_side(item: &Value) -> &'static str {
    match value_string(item, "side", "")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "buy" => "buy",
        "sell" => "sell",
        _ => "",
    }
}

fn first_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| parse_f64(value.get(*key)))
}

fn sum_finite(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut found = false;
    let mut total = 0.0;
    for value in values {
        found = true;
        total += value;
    }
    found.then_some(total)
}

fn first_positive_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| value_positive_f64(value, key))
}

fn first_string(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| {
            let value = value_string(value, key, "");
            (!value.trim().is_empty()).then_some(value)
        })
        .unwrap_or_default()
}

fn first_non_negative_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value_non_negative_f64(value, key))
}

fn value_finite_f64(value: &Value, key: &str) -> Option<f64> {
    parse_f64(value.get(key))
}

fn value_positive_f64(value: &Value, key: &str) -> Option<f64> {
    parse_f64(value.get(key)).filter(|item| *item > 0.0)
}

fn value_non_negative_f64(value: &Value, key: &str) -> Option<f64> {
    parse_f64(value.get(key)).filter(|item| *item >= 0.0)
}

fn value_f64(value: &Value, key: &str, default: f64) -> f64 {
    parse_f64(value.get(key)).unwrap_or(default)
}

fn value_i64(value: &Value, key: &str, default: i64) -> i64 {
    match value.get(key) {
        Some(Value::Number(item)) => item.as_i64().unwrap_or(default),
        Some(Value::String(item)) => item.parse::<i64>().unwrap_or(default),
        _ => default,
    }
}

fn parse_f64(value: Option<&Value>) -> Option<f64> {
    let parsed = match value? {
        Value::Number(item) => item.as_f64(),
        Value::String(item) => item.parse::<f64>().ok(),
        _ => None,
    }?;
    parsed.is_finite().then_some(parsed)
}

fn finite_product(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    let value = left? * right?;
    value.is_finite().then_some(value)
}

fn value_string(value: &Value, key: &str, default: &str) -> String {
    match value.get(key) {
        Some(Value::String(item)) => item.clone(),
        Some(Value::Number(item)) => item.to_string(),
        Some(Value::Bool(item)) => item.to_string(),
        _ => default.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        account_context_from_private_items, merge_order_contexts,
        merge_private_order_contexts_with_source, orders_context_from_private_items,
        orders_context_from_private_stream_items, positions_context_from_private_items,
        private_order_action, private_order_stream_cache_available,
    };

    #[test]
    fn private_account_context_maps_total_equity_and_usdt_cash() {
        let account = account_context_from_private_items(
            vec![json!({
                "totalEq": "1234.5",
                "adjEq": "1200",
                "details": [
                    {
                        "ccy": "USDT",
                        "cashBal": "1000",
                        "availBal": "950",
                        "eqUsd": "1001.5",
                        "uTime": "1780000000000"
                    }
                ]
            })],
            "live",
            10_000.0,
        );

        assert_eq!(account["source"], "okx_private_rest");
        assert_eq!(account["mode"], "live");
        assert_eq!(account["equity"], 1234.5);
        assert_eq!(account["cash"], 950.0);
        assert_eq!(account["usdt_balance"], 1000.0);
    }

    #[test]
    fn private_order_action_normalizes_protective_order_type_alias() {
        assert_eq!(private_order_action("stop-market"), "place_risk_order");
        assert_eq!(
            private_order_action("take-profit-market"),
            "place_risk_order"
        );
    }

    #[test]
    fn private_account_context_does_not_fabricate_zero_economics_from_invalid_numbers() {
        let account = account_context_from_private_items(
            vec![json!({
                "totalEq": "bad-total-equity",
                "isoEq": "bad-isolated-equity",
                "adjEq": "bad-adjusted-equity",
                "details": [
                    {
                        "ccy": "USDT",
                        "cashBal": "bad-cash",
                        "availBal": "bad-available",
                        "availEq": "bad-available-equity",
                        "frozenBal": "bad-frozen",
                        "ordFrozen": "bad-order-frozen",
                        "eq": "bad-equity",
                        "eqUsd": "bad-equity-usd",
                        "disEq": "bad-discount-equity",
                        "uTime": "1780000000000"
                    }
                ]
            })],
            "live",
            10_000.0,
        );

        assert!(account["equity"].is_null());
        assert!(account["total_equity"].is_null());
        assert!(account["cash"].is_null());
        assert!(account["usdt_balance"].is_null());
        assert!(account["usdt_available"].is_null());
        assert!(account["usdt_equity_usd"].is_null());
        assert!(account["details"][0]["avail_bal"].is_null());
        assert!(account["details"][0]["cash_bal"].is_null());
        assert!(account["details"][0]["eq_usd"].is_null());
    }

    #[test]
    fn private_account_context_does_not_fabricate_zero_economics_from_empty_payload() {
        let account = account_context_from_private_items(Vec::new(), "live", 10_000.0);

        assert_eq!(account["source"], "okx_private_rest");
        assert_eq!(account["mode"], "live");
        assert_eq!(account["initial_capital"], 10_000.0);
        assert!(account["equity"].is_null());
        assert!(account["total_equity"].is_null());
        assert!(account["cash"].is_null());
        assert!(account["usdt_balance"].is_null());
        assert!(account["usdt_available"].is_null());
        assert!(account["usdt_equity_usd"].is_null());
        assert_eq!(account["details"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn private_account_context_does_not_fabricate_zero_usdt_when_usdt_detail_is_absent() {
        let account = account_context_from_private_items(
            vec![json!({
                "totalEq": "70000",
                "details": [{
                    "ccy": "BTC",
                    "cashBal": "1",
                    "eqUsd": "70000",
                    "uTime": "1780000000000"
                }]
            })],
            "live",
            10_000.0,
        );

        assert_eq!(account["equity"], 70000.0);
        assert_eq!(account["total_equity"], 70000.0);
        assert_eq!(account["details"].as_array().map(Vec::len), Some(1));
        assert!(account["usdt_balance"].is_null());
        assert!(account["usdt_available"].is_null());
        assert!(account["usdt_equity_usd"].is_null());
    }

    #[test]
    fn private_positions_context_keys_open_positions_by_instrument() {
        let positions = positions_context_from_private_items(
            vec![
                json!({
                    "instId": "btc-usdt-swap",
                    "instType": "SWAP",
                    "posSide": "long",
                    "pos": "2",
                    "avgPx": "100",
                    "markPx": "105",
                    "upl": "10",
                    "uplRatio": "0.05",
                    "lever": "3",
                    "imr": "50"
                }),
                json!({"instId": "ETH-USDT-SWAP", "pos": "0"}),
            ],
            "live",
        );

        assert_eq!(positions["BTC-USDT-SWAP"]["source"], "okx_private_rest");
        assert_eq!(positions["BTC-USDT-SWAP"]["side"], "long");
        assert_eq!(positions["BTC-USDT-SWAP"]["quantity"], 2.0);
        assert_eq!(positions["BTC-USDT-SWAP"]["entry_price"], 100.0);
        assert_eq!(positions["BTC-USDT-SWAP:long"]["side"], "long");
        assert_eq!(positions["open"].as_array().map(Vec::len), Some(1));
        assert!(positions.get("ETH-USDT-SWAP").is_none());
    }

    #[test]
    fn private_positions_context_preserves_dual_side_positions_for_same_instrument() {
        let positions = positions_context_from_private_items(
            vec![
                json!({
                    "instId": "BTC-USDT-SWAP",
                    "instType": "SWAP",
                    "posSide": "long",
                    "pos": "2",
                    "avgPx": "100"
                }),
                json!({
                    "instId": "BTC-USDT-SWAP",
                    "instType": "SWAP",
                    "posSide": "short",
                    "pos": "1.5",
                    "avgPx": "110"
                }),
            ],
            "live",
        );

        let open = positions["open"]
            .as_array()
            .expect("positions.open should expose all active position rows");
        assert_eq!(open.len(), 2);
        assert!(open.iter().any(|row| row["pos_side"] == "long"));
        assert!(open.iter().any(|row| row["pos_side"] == "short"));
        assert_eq!(positions["BTC-USDT-SWAP:long"]["quantity"], 2.0);
        assert_eq!(positions["BTC-USDT-SWAP:short"]["quantity"], 1.5);
        assert_eq!(positions["BTC-USDT-SWAP:short"]["entry_price"], 110.0);
    }

    #[test]
    fn private_orders_context_groups_open_fills_and_terminal_states() {
        let orders = orders_context_from_private_items(
            vec![json!({
                "instId": "btc-usdt-swap",
                "instType": "SWAP",
                "ordId": "open-1",
                "clOrdId": "client-open",
                "side": "buy",
                "ordType": "limit",
                "px": "100",
                "sz": "2",
                "fillSz": "1",
                "state": "partially_filled",
                "cTime": "1780000000000",
                "uTime": "1780000001000"
            })],
            vec![json!({
                "instId": "ETH-USDT-SWAP",
                "instType": "SWAP",
                "tradeId": "fill-1",
                "ordId": "filled-1",
                "side": "sell",
                "fillPx": "120.5",
                "fillSz": "0.25",
                "fee": "-0.01",
                "feeCcy": "USDT",
                "ts": "1780000002000"
            })],
            vec![
                json!({"instId": "SOL-USDT-SWAP", "ordId": "cancel-1", "side": "buy", "state": "canceled"}),
                json!({"instId": "ETH-USDT-SWAP", "ordId": "filled-1", "side": "sell", "state": "filled"}),
            ],
            "live",
        );

        assert_eq!(orders["source"], "okx_private_rest");
        assert_eq!(orders["open"].as_array().map(Vec::len), Some(1));
        assert_eq!(orders["open"][0]["status"], "partially_filled");
        assert_eq!(orders["open"][0]["quantity"], 2.0);
        assert_eq!(orders["open"][0]["value"], 200.0);
        assert_eq!(orders["open"][0]["success"], true);
        assert_eq!(orders["open"][0]["action"], "");
        assert_eq!(orders["open"][0]["filled_size"], 1.0);
        assert_eq!(orders["recent_fills"].as_array().map(Vec::len), Some(1));
        assert_eq!(orders["recent_fills"][0]["status"], "filled");
        assert_eq!(orders["recent_fills"][0]["action"], "");
        assert_eq!(orders["recent_fills"][0]["size"], 0.25);
        assert_eq!(orders["recent_fills"][0]["quantity"], 0.25);
        assert_eq!(orders["recent_fills"][0]["value"], 30.125);
        assert_eq!(orders["recent_fills"][0]["success"], true);
        assert_eq!(
            orders["recent_rejections"].as_array().map(Vec::len),
            Some(1)
        );
        assert_eq!(orders["recent_rejections"][0]["status"], "canceled");
        assert_eq!(orders["recent_rejections"][0]["success"], false);
    }

    #[test]
    fn private_orders_context_from_stream_items_uses_ws_source_and_buckets_states() {
        let orders = orders_context_from_private_stream_items(
            vec![
                json!({
                    "instId": "BTC-USDT-SWAP",
                    "instType": "SWAP",
                    "ordId": "open-ws",
                    "side": "buy",
                    "ordType": "limit",
                    "px": "100",
                    "sz": "2",
                    "state": "live",
                    "cTime": "1780000000000",
                    "uTime": "1780000001000"
                }),
                json!({
                    "instId": "ETH-USDT-SWAP",
                    "instType": "SWAP",
                    "ordId": "cancel-ws",
                    "side": "sell",
                    "state": "canceled",
                    "cTime": "1780000002000",
                    "uTime": "1780000003000"
                }),
                json!({
                    "instId": "SOL-USDT-SWAP",
                    "instType": "SWAP",
                    "ordId": "filled-order-ws",
                    "side": "sell",
                    "state": "filled",
                    "cTime": "1780000004000",
                    "uTime": "1780000005000"
                }),
            ],
            Vec::new(),
            vec![json!({
                "instId": "SOL-USDT-SWAP",
                "instType": "SWAP",
                "tradeId": "fill-ws",
                "ordId": "filled-order-ws",
                "side": "sell",
                "fillPx": "120",
                "fillSz": "0.5",
                "ts": "1780000006000"
            })],
            "simulated",
            "okx_private_ws_cache",
        );

        assert_eq!(orders["source"], "okx_private_ws_cache");
        assert_eq!(orders["open"].as_array().map(Vec::len), Some(1));
        assert_eq!(orders["open"][0]["source"], "okx_private_ws_cache");
        assert_eq!(orders["open"][0]["order_id"], "open-ws");
        assert_eq!(orders["recent_fills"].as_array().map(Vec::len), Some(1));
        assert_eq!(orders["recent_fills"][0]["source"], "okx_private_ws_cache");
        assert_eq!(orders["recent_fills"][0]["trade_id"], "fill-ws");
        assert_eq!(
            orders["recent_rejections"].as_array().map(Vec::len),
            Some(1)
        );
        assert_eq!(orders["recent_rejections"][0]["order_id"], "cancel-ws");
        assert_eq!(
            orders["recent_rejections"][0]["source"],
            "okx_private_ws_cache"
        );
    }

    #[test]
    fn private_orders_context_from_stream_items_includes_algo_orders() {
        let orders = orders_context_from_private_stream_items(
            Vec::new(),
            vec![
                json!({
                    "instId": "BTC-USDT-SWAP",
                    "instType": "SWAP",
                    "algoId": "algo-live-1",
                    "algoClOrdId": "algo-client-live-1",
                    "ordType": "conditional",
                    "side": "sell",
                    "state": "live",
                    "slTriggerPx": "94",
                    "sz": "1",
                    "cTime": "1780000000000",
                    "uTime": "1780000001000"
                }),
                json!({
                    "instId": "ETH-USDT-SWAP",
                    "instType": "SWAP",
                    "algoId": "algo-effective-1",
                    "algoClOrdId": "algo-client-effective-1",
                    "actualOrdId": "actual-order-1",
                    "actualClOrdId": "actual-client-1",
                    "ordType": "conditional",
                    "side": "buy",
                    "actualSide": "buy",
                    "state": "effective",
                    "actualPx": "120",
                    "actualSz": "0.5",
                    "cTime": "1780000002000",
                    "uTime": "1780000003000"
                }),
                json!({
                    "instId": "SOL-USDT-SWAP",
                    "instType": "SWAP",
                    "algoId": "algo-failed-1",
                    "algoClOrdId": "algo-client-failed-1",
                    "ordType": "conditional",
                    "side": "sell",
                    "state": "order_failed",
                    "failCode": "51000",
                    "failReason": "bad trigger",
                    "cTime": "1780000004000",
                    "uTime": "1780000005000"
                }),
            ],
            Vec::new(),
            "simulated",
            "okx_private_ws_cache",
        );

        assert_eq!(orders["open"].as_array().map(Vec::len), Some(1));
        assert_eq!(orders["open"][0]["action"], "place_risk_order");
        assert_eq!(orders["open"][0]["order_id"], "algo-live-1");
        assert_eq!(orders["open"][0]["client_order_id"], "algo-client-live-1");
        assert_eq!(orders["open"][0]["status"], "algo_live");
        assert_eq!(orders["open"][0]["price"], 94.0);
        assert_eq!(orders["recent_fills"].as_array().map(Vec::len), Some(1));
        assert_eq!(orders["recent_fills"][0]["status"], "algo_effective");
        assert_eq!(
            orders["recent_fills"][0]["actual_order_id"],
            "actual-order-1"
        );
        assert_eq!(orders["recent_fills"][0]["size"], 0.5);
        assert_eq!(
            orders["recent_rejections"].as_array().map(Vec::len),
            Some(1)
        );
        assert_eq!(orders["recent_rejections"][0]["status"], "algo_failed");
        assert_eq!(orders["recent_rejections"][0]["success"], false);
        assert!(orders["recent_rejections"][0]["error_message"]
            .as_str()
            .unwrap_or_default()
            .contains("bad trigger"));
    }

    #[test]
    fn private_order_stream_cache_available_accepts_any_ws_delta_bucket() {
        assert!(!private_order_stream_cache_available(&None, &None, &None));
        assert!(!private_order_stream_cache_available(
            &Some(Vec::new()),
            &Some(Vec::new()),
            &Some(Vec::new()),
        ));
        assert!(private_order_stream_cache_available(
            &Some(vec![json!({"ordId": "order-1"})]),
            &None,
            &None,
        ));
        assert!(private_order_stream_cache_available(
            &None,
            &Some(vec![json!({"algoId": "algo-1"})]),
            &None,
        ));
        assert!(private_order_stream_cache_available(
            &None,
            &None,
            &Some(vec![json!({"tradeId": "fill-1"})]),
        ));
    }

    #[test]
    fn merge_private_order_contexts_keeps_rest_open_when_ws_has_only_fills() {
        let stream_orders = orders_context_from_private_stream_items(
            Vec::new(),
            Vec::new(),
            vec![json!({
                "instId": "ETH-USDT-SWAP",
                "instType": "SWAP",
                "tradeId": "fill-ws-1",
                "ordId": "filled-order-1",
                "side": "sell",
                "fillPx": "120",
                "fillSz": "0.5",
                "ts": "1780000006000"
            })],
            "live",
            "okx_private_ws_cache",
        );
        let rest_orders = orders_context_from_private_items(
            vec![json!({
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "ordId": "rest-open-1",
                "clOrdId": "rest-client-1",
                "side": "buy",
                "ordType": "limit",
                "px": "100",
                "sz": "2",
                "state": "live",
                "cTime": "1780000000000"
            })],
            Vec::new(),
            Vec::new(),
            "live",
        );

        let merged = merge_private_order_contexts_with_source(
            stream_orders,
            rest_orders,
            "okx_private_ws_cache+okx_private_rest",
        );

        assert_eq!(merged["source"], "okx_private_ws_cache+okx_private_rest");
        assert_eq!(merged["open"].as_array().map(Vec::len), Some(1));
        assert_eq!(merged["open"][0]["source"], "okx_private_rest");
        assert_eq!(merged["open"][0]["order_id"], "rest-open-1");
        assert_eq!(merged["recent_fills"].as_array().map(Vec::len), Some(1));
        assert_eq!(merged["recent_fills"][0]["source"], "okx_private_ws_cache");
        assert_eq!(merged["recent_fills"][0]["trade_id"], "fill-ws-1");
    }

    #[test]
    fn merge_private_order_contexts_keeps_ws_values_and_supplements_rest_fields() {
        let merged = merge_private_order_contexts_with_source(
            json!({
                "source": "okx_private_ws_cache",
                "mode": "live",
                "open": [{
                    "source": "okx_private_ws_cache",
                    "mode": "live",
                    "symbol": "BTC-USDT-SWAP",
                    "inst_id": "BTC-USDT-SWAP",
                    "order_id": "shared-order-1",
                    "client_order_id": "shared-client-1",
                    "price": 101.0,
                    "size": 2.0,
                    "status": "live",
                    "action": "",
                    "timestamp": 1_780_000_000_100i64
                }],
                "recent_fills": [],
                "recent_rejections": []
            }),
            json!({
                "source": "okx_private_rest",
                "mode": "live",
                "open": [{
                    "source": "okx_private_rest",
                    "mode": "live",
                    "symbol": "BTC-USDT-SWAP",
                    "inst_id": "BTC-USDT-SWAP",
                    "order_id": "shared-order-1",
                    "client_order_id": "shared-client-1",
                    "price": 100.0,
                    "size": 3.0,
                    "status": "partially_filled",
                    "action": "open_position",
                    "strategy_id": "strategy-a",
                    "timestamp": 1_780_000_000_000i64,
                    "raw": {"source": "rest"}
                }],
                "recent_fills": [],
                "recent_rejections": []
            }),
            "okx_private_ws_cache+okx_private_rest",
        );

        assert_eq!(merged["open"].as_array().map(Vec::len), Some(1));
        let row = &merged["open"][0];
        assert_eq!(row["source"], "okx_private_ws_cache+okx_private_rest");
        assert_eq!(row["price"], 101.0);
        assert_eq!(row["size"], 2.0);
        assert_eq!(row["status"], "live");
        assert_eq!(row["timestamp"], 1_780_000_000_100i64);
        assert_eq!(row["action"], "open_position");
        assert_eq!(row["strategy_id"], "strategy-a");
        assert_eq!(row["raw"], json!({"source": "rest"}));
    }

    #[test]
    fn private_order_context_does_not_fabricate_zero_economics_from_invalid_numbers() {
        let orders = orders_context_from_private_items(
            vec![json!({
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "ordId": "open-invalid",
                "side": "buy",
                "ordType": "limit",
                "px": "bad-price",
                "sz": "bad-size",
                "fillSz": "bad-fill",
                "avgPx": "bad-avg",
                "state": "partially_filled",
                "cTime": "1780000000000"
            })],
            vec![json!({
                "instId": "ETH-USDT-SWAP",
                "instType": "SWAP",
                "tradeId": "fill-invalid",
                "ordId": "filled-invalid",
                "side": "sell",
                "fillPx": "bad-fill-price",
                "fillSz": "bad-fill-size",
                "fee": "bad-fee",
                "ts": "1780000002000"
            })],
            Vec::new(),
            "live",
        );

        assert!(orders["open"][0]["price"].is_null());
        assert!(orders["open"][0]["quantity"].is_null());
        assert!(orders["open"][0]["value"].is_null());
        assert!(orders["open"][0]["filled_size"].is_null());
        assert!(orders["open"][0]["avg_price"].is_null());
        assert!(orders["recent_fills"][0]["price"].is_null());
        assert!(orders["recent_fills"][0]["quantity"].is_null());
        assert!(orders["recent_fills"][0]["value"].is_null());
        assert!(orders["recent_fills"][0]["fee"].is_null());
    }

    #[test]
    fn private_positions_context_does_not_fabricate_zero_prices_from_invalid_numbers() {
        let positions = positions_context_from_private_items(
            vec![json!({
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "posSide": "long",
                "pos": "2",
                "avgPx": "bad-entry",
                "markPx": "bad-mark",
                "upl": "bad-upl",
                "uplRatio": "bad-upl-ratio",
                "lever": "bad-leverage",
                "imr": "bad-margin"
            })],
            "live",
        );

        assert_eq!(positions["BTC-USDT-SWAP"]["quantity"], 2.0);
        assert!(positions["BTC-USDT-SWAP"]["entry_price"].is_null());
        assert!(positions["BTC-USDT-SWAP"]["mark_price"].is_null());
        assert!(positions["BTC-USDT-SWAP"]["unrealized_pnl"].is_null());
        assert!(positions["BTC-USDT-SWAP"]["unrealized_pnl_pct"].is_null());
        assert!(positions["BTC-USDT-SWAP"]["lever"].is_null());
        assert!(positions["BTC-USDT-SWAP"]["margin"].is_null());
    }

    #[test]
    fn merge_order_contexts_keeps_private_and_local_order_state() {
        let merged = merge_order_contexts(
            json!({
                "mode": "live",
                "open": [{"source": "okx_private_rest", "order_id": "okx-open"}],
                "recent_fills": [{"source": "okx_private_rest", "trade_id": "fill-1"}],
                "recent_rejections": []
            }),
            json!({
                "run_id": "run-local",
                "open": [{"source": "live_order_records", "order_id": "local-open"}],
                "recent_fills": [],
                "recent_rejections": [{"source": "live_order_records", "status": "blocked"}]
            }),
        );

        assert_eq!(merged["source"], "okx_private_rest+live_order_records");
        assert_eq!(merged["mode"], "live");
        assert_eq!(merged["run_id"], "run-local");
        assert_eq!(merged["open"].as_array().map(Vec::len), Some(2));
        assert_eq!(merged["recent_fills"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            merged["recent_rejections"].as_array().map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn merge_order_contexts_merges_local_metadata_into_private_order_identity() {
        let merged = merge_order_contexts(
            json!({
                "mode": "live",
                "open": [{
                    "source": "okx_private_rest",
                    "symbol": "BTC-USDT-SWAP",
                    "order_id": "okx-order-1",
                    "client_order_id": "client-order-1",
                    "status": "live",
                    "success": true,
                    "price": 100.0,
                    "size": 2.0,
                    "quantity": 2.0,
                    "action": "",
                    "timestamp": 1_780_000_000_100i64
                }],
                "recent_fills": [],
                "recent_rejections": []
            }),
            json!({
                "run_id": "run-local",
                "open": [{
                    "source": "live_order_records",
                    "id": 42,
                    "mode": "live",
                    "symbol": "BTC-USDT-SWAP",
                    "inst_id": "BTC-USDT-SWAP",
                    "order_id": "",
                    "client_order_id": "client-order-1",
                    "actual_order_id": "okx-order-1",
                    "actual_client_order_id": "client-order-1",
                    "status": "submitted",
                    "success": true,
                    "price": 90.0,
                    "size": 9.0,
                    "quantity": 9.0,
                    "action": "open_position",
                    "strategy_id": "strategy-a",
                    "strategy_name": "Strategy A",
                    "run_id": "run-local",
                    "timestamp": 1_780_000_000_000i64,
                    "error_message": "local submitted"
                }],
                "recent_fills": [],
                "recent_rejections": []
            }),
        );

        assert_eq!(merged["open"].as_array().map(Vec::len), Some(1));
        let row = &merged["open"][0];
        assert_eq!(row["source"], "okx_private_rest+live_order_records");
        assert_eq!(row["order_id"], "okx-order-1");
        assert_eq!(row["client_order_id"], "client-order-1");
        assert_eq!(row["status"], "live");
        assert_eq!(row["price"], 100.0);
        assert_eq!(row["size"], 2.0);
        assert_eq!(row["quantity"], 2.0);
        assert_eq!(row["action"], "open_position");
        assert_eq!(row["strategy_id"], "strategy-a");
        assert_eq!(row["run_id"], "run-local");
        assert_eq!(row["actual_order_id"], "okx-order-1");
        assert_eq!(row["local_order_record_id"], 42);
        assert_eq!(row["local_status"], "submitted");
        assert_eq!(row["local_timestamp"], 1_780_000_000_000i64);
        assert_eq!(row["timestamp"], 1_780_000_000_100i64);
    }

    #[test]
    fn merge_order_contexts_matches_actual_order_identity_without_dropping_unmatched_local_rows() {
        let merged = merge_order_contexts(
            json!({
                "mode": "live",
                "open": [{
                    "source": "okx_private_rest",
                    "symbol": "ETH-USDT-SWAP",
                    "order_id": "actual-order-1",
                    "client_order_id": "actual-client-1",
                    "status": "partially_filled",
                    "success": true,
                    "action": ""
                }],
                "recent_fills": [],
                "recent_rejections": []
            }),
            json!({
                "run_id": "run-local",
                "open": [
                    {
                        "source": "live_order_records",
                        "mode": "live",
                        "symbol": "ETH-USDT-SWAP",
                        "inst_id": "ETH-USDT-SWAP",
                        "order_id": "submitted-order-placeholder",
                        "client_order_id": "submitted-client-placeholder",
                        "actual_order_id": "actual-order-1",
                        "actual_client_order_id": "actual-client-1",
                        "status": "submitted",
                        "action": "open_position"
                    },
                    {
                        "source": "live_order_records",
                        "order_id": "local-only-order",
                        "client_order_id": "local-only-client",
                        "status": "submit_unknown",
                        "action": "open_position"
                    }
                ],
                "recent_fills": [],
                "recent_rejections": []
            }),
        );

        assert_eq!(merged["open"].as_array().map(Vec::len), Some(2));
        assert_eq!(merged["open"][0]["order_id"], "actual-order-1");
        assert_eq!(merged["open"][0]["action"], "open_position");
        assert_eq!(merged["open"][0]["local_status"], "submitted");
        assert_eq!(merged["open"][1]["order_id"], "local-only-order");
        assert_eq!(merged["open"][1]["status"], "submit_unknown");
    }

    #[test]
    fn merge_order_contexts_does_not_merge_identity_collision_across_symbols() {
        let merged = merge_order_contexts(
            json!({
                "mode": "live",
                "open": [{
                    "source": "okx_private_rest",
                    "mode": "live",
                    "symbol": "BTC-USDT-SWAP",
                    "inst_id": "BTC-USDT-SWAP",
                    "order_id": "btc-okx-order",
                    "client_order_id": "shared-client-id",
                    "status": "live",
                    "success": true,
                    "action": ""
                }],
                "recent_fills": [],
                "recent_rejections": []
            }),
            json!({
                "run_id": "run-local",
                "open": [{
                    "source": "live_order_records",
                    "id": 43,
                    "mode": "live",
                    "symbol": "ETH-USDT-SWAP",
                    "inst_id": "ETH-USDT-SWAP",
                    "order_id": "eth-local-order",
                    "client_order_id": "shared-client-id",
                    "status": "submitted",
                    "success": true,
                    "action": "open_position"
                }],
                "recent_fills": [],
                "recent_rejections": []
            }),
        );

        assert_eq!(merged["open"].as_array().map(Vec::len), Some(2));
        assert_eq!(merged["open"][0]["symbol"], "BTC-USDT-SWAP");
        assert_eq!(merged["open"][0]["client_order_id"], "shared-client-id");
        assert!(merged["open"][0]["local_order_record_id"].is_null());
        assert_eq!(merged["open"][1]["symbol"], "ETH-USDT-SWAP");
        assert_eq!(merged["open"][1]["client_order_id"], "shared-client-id");
        assert_eq!(merged["open"][1]["action"], "open_position");
    }
}
