use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde_json::{json, Value};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

use crate::error::{AppError, AppResult};

use super::super::super::payload::live_order_row_to_json;
use super::super::super::types::LiveStrategyStatus;

pub async fn query_live_orders(
    pool: &SqlitePool,
    limit: i64,
    mode: &str,
    run_id: &str,
) -> AppResult<Vec<Value>> {
    let mut sql = String::from(
        r#"
        SELECT id, strategy_id, strategy_name, symbol,
               inst_id, inst_type,
               side, order_type, size, price, order_id, client_order_id,
               parent_order_id, parent_client_order_id,
               actual_order_id, actual_client_order_id,
               status, action, error_message, mode,
               run_id, action_timestamp, arrival_ts,
               arrival_mid_px, arrival_bid_px, arrival_ask_px,
               created_at,
               success
        FROM live_order_records
        WHERE 1 = 1
        "#,
    );
    if !mode.trim().is_empty() {
        sql.push_str(" AND mode = ?");
    }
    if !run_id.trim().is_empty() {
        sql.push_str(" AND run_id = ?");
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ?");

    let mut query = sqlx::query(&sql);
    if !mode.trim().is_empty() {
        query = query.bind(mode);
    }
    if !run_id.trim().is_empty() {
        query = query.bind(run_id.trim());
    }
    query = query.bind(limit.clamp(1, 500));

    let mut orders: Vec<Value> = query
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(live_order_row_to_json)
        .collect::<AppResult<Vec<_>>>()?;
    enrich_orders_with_fill_aggregates(pool, &mut orders).await?;
    Ok(orders)
}

pub async fn query_live_order_context(
    pool: &SqlitePool,
    status: &LiveStrategyStatus,
) -> AppResult<Value> {
    let rows = if status.run_id.trim().is_empty() {
        Vec::new()
    } else {
        query_live_orders(pool, 50, &status.mode, &status.run_id).await?
    };
    let mut open = Vec::new();
    let mut recent_fills = Vec::new();
    let mut recent_rejections = Vec::new();
    for row in &rows {
        let context_row = strategy_context_order_row(row);
        match order_bucket(row)? {
            OrderBucket::Open => open.push(context_row),
            OrderBucket::Fill => recent_fills.push(context_row),
            OrderBucket::Rejection => recent_rejections.push(context_row),
        }
    }
    Ok(json!({
        "open": open,
        "recent_fills": recent_fills,
        "recent_rejections": recent_rejections,
        "run_id": &status.run_id,
        "last_order_candle_ts": status.last_order_candle_ts,
        "total_orders": status.total_orders,
        "successful_orders": status.successful_orders,
        "failed_orders": status.failed_orders,
    }))
}

fn strategy_context_order_row(row: &Value) -> Value {
    let mut context_row = row.clone();
    if let Some(object) = context_row.as_object_mut() {
        object.insert(
            "quantity".to_string(),
            row.get("size").cloned().unwrap_or(Value::Null),
        );
    }
    context_row
}

enum OrderBucket {
    Open,
    Fill,
    Rejection,
}

fn order_bucket(row: &Value) -> AppResult<OrderBucket> {
    let status = row
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Runtime("live order context 缺少 status".to_string()))?
        .to_ascii_lowercase();
    let success = row
        .get("success")
        .and_then(Value::as_bool)
        .ok_or_else(|| AppError::Runtime("live order context 缺少 success".to_string()))?;
    let fill_count = row
        .get("fill_count")
        .and_then(Value::as_i64)
        .ok_or_else(|| AppError::Runtime("live order context 缺少 fill_count".to_string()))?;
    if matches!(
        status.as_str(),
        "submitting"
            | "submit_unknown"
            | "algo_submitted"
            | "algo_submit_unknown"
            | "algo_live"
            | "algo_cancel_requested"
            | "algo_modify_requested"
            | "open"
            | "submitted"
            | "pending"
            | "live"
            | "cancel_requested"
            | "modify_requested"
            | "partially_filled"
            | "partial-filled"
            | "partially-filled"
    ) {
        return Ok(OrderBucket::Open);
    }
    if success
        && matches!(
            status.as_str(),
            "filled"
                | "fully_filled"
                | "fully-filled"
                | "algo_effective"
                | "algo_partially_effective"
        )
    {
        return Ok(OrderBucket::Fill);
    }
    if success && fill_count > 0 {
        return Ok(OrderBucket::Fill);
    }
    Ok(OrderBucket::Rejection)
}

#[derive(Clone, Debug)]
struct FillRecord {
    id: i64,
    mode: String,
    order_id: String,
    client_order_id: String,
    fill_px: Option<f64>,
    fill_sz: Option<f64>,
    fee: Option<f64>,
    fee_ccy: String,
    ts: Option<i64>,
    source: String,
}

#[derive(Default)]
struct FillAggregate {
    seen_ids: HashSet<i64>,
    fill_count: i64,
    filled_size: f64,
    fill_notional: f64,
    total_fee: f64,
    valid_size_count: i64,
    valid_fee_count: i64,
    fee_ccy: Option<String>,
    mixed_fee_ccy: bool,
    first_fill_ts: Option<i64>,
    last_fill_ts: Option<i64>,
    sources: BTreeSet<String>,
}

impl FillAggregate {
    fn add(&mut self, fill: &FillRecord) {
        if !self.seen_ids.insert(fill.id) {
            return;
        }
        self.fill_count += 1;
        if let (Some(px), Some(sz)) = (fill.fill_px, fill.fill_sz) {
            self.valid_size_count += 1;
            self.filled_size += sz;
            self.fill_notional += px * sz;
        }
        if let Some(fee) = fill.fee {
            self.valid_fee_count += 1;
            self.total_fee += fee;
            let fee_ccy = fill.fee_ccy.trim();
            if !fee_ccy.is_empty() {
                match &self.fee_ccy {
                    None => self.fee_ccy = Some(fee_ccy.to_string()),
                    Some(current) if current.eq_ignore_ascii_case(fee_ccy) => {}
                    Some(_) => self.mixed_fee_ccy = true,
                }
            }
        }
        if let Some(ts) = fill.ts.filter(|value| *value > 0) {
            self.first_fill_ts = Some(self.first_fill_ts.map_or(ts, |current| current.min(ts)));
            self.last_fill_ts = Some(self.last_fill_ts.map_or(ts, |current| current.max(ts)));
        }
        if !fill.source.trim().is_empty() {
            self.sources.insert(fill.source.trim().to_string());
        }
    }

    fn apply_to_order(&self, order: &mut Value) {
        let size = order_positive_f64(order, "size");
        let Some(object) = order.as_object_mut() else {
            return;
        };
        object.insert("fill_count".to_string(), json!(self.fill_count));
        if self.valid_size_count > 0 && self.filled_size.is_finite() && self.filled_size > 0.0 {
            object.insert("filled_size".to_string(), json!(self.filled_size));
            object.insert("filled_quantity".to_string(), json!(self.filled_size));
            object.insert("fill_notional".to_string(), json!(self.fill_notional));
            object.insert(
                "avg_fill_price".to_string(),
                json!(self.fill_notional / self.filled_size),
            );
            if let Some(size) = size {
                object.insert(
                    "remaining_size".to_string(),
                    json!((size - self.filled_size).max(0.0)),
                );
            } else {
                object.insert("remaining_size".to_string(), Value::Null);
            }
        } else {
            object.insert("filled_size".to_string(), Value::Null);
            object.insert("filled_quantity".to_string(), Value::Null);
            object.insert("fill_notional".to_string(), Value::Null);
            object.insert("avg_fill_price".to_string(), Value::Null);
            object.insert("remaining_size".to_string(), Value::Null);
        }
        if self.valid_fee_count > 0 && self.total_fee.is_finite() {
            object.insert("total_fee".to_string(), json!(self.total_fee));
            object.insert(
                "fee_ccy".to_string(),
                json!(if self.mixed_fee_ccy {
                    "mixed".to_string()
                } else {
                    self.fee_ccy.clone().unwrap_or_default()
                }),
            );
        } else {
            object.insert("total_fee".to_string(), Value::Null);
            object.insert("fee_ccy".to_string(), Value::Null);
        }
        object.insert("first_fill_ts".to_string(), json!(self.first_fill_ts));
        object.insert("last_fill_ts".to_string(), json!(self.last_fill_ts));
        object.insert(
            "fill_source".to_string(),
            json!(self.sources.iter().cloned().collect::<Vec<_>>().join(",")),
        );
    }
}

async fn enrich_orders_with_fill_aggregates(
    pool: &SqlitePool,
    orders: &mut [Value],
) -> AppResult<()> {
    if orders.is_empty() {
        return Ok(());
    }
    let identities_by_mode = collect_order_identities_by_mode(orders);
    if identities_by_mode.is_empty() {
        return Ok(());
    }
    let mut fills = Vec::new();
    for (mode, identities) in identities_by_mode {
        fills.extend(query_matching_fill_records(pool, &mode, &identities).await?);
    }
    for order in orders {
        let mode = order_text(order, "mode").to_ascii_lowercase();
        let order_id = order_text(order, "order_id");
        let client_order_id = order_text(order, "client_order_id");
        let actual_order_id = order_text(order, "actual_order_id");
        let actual_client_order_id = order_text(order, "actual_client_order_id");
        let mut aggregate = FillAggregate::default();
        for fill in &fills {
            if !fill.mode.eq_ignore_ascii_case(&mode) {
                continue;
            }
            let order_match = !order_id.is_empty() && fill.order_id == order_id;
            let client_match =
                !client_order_id.is_empty() && fill.client_order_id == client_order_id;
            let actual_order_match =
                !actual_order_id.is_empty() && fill.order_id == actual_order_id;
            let actual_client_match = !actual_client_order_id.is_empty()
                && fill.client_order_id == actual_client_order_id;
            if order_match || client_match || actual_order_match || actual_client_match {
                aggregate.add(fill);
            }
        }
        aggregate.apply_to_order(order);
    }
    Ok(())
}

#[derive(Default)]
struct OrderIdentities {
    order_ids: BTreeSet<String>,
    client_order_ids: BTreeSet<String>,
}

fn collect_order_identities_by_mode(orders: &[Value]) -> BTreeMap<String, OrderIdentities> {
    let mut by_mode = BTreeMap::<String, OrderIdentities>::new();
    for order in orders {
        let mode = order_text(order, "mode").to_ascii_lowercase();
        if mode.is_empty() {
            continue;
        }
        let entry = by_mode.entry(mode).or_default();
        let order_id = order_text(order, "order_id");
        if !order_id.is_empty() {
            entry.order_ids.insert(order_id);
        }
        let client_order_id = order_text(order, "client_order_id");
        if !client_order_id.is_empty() {
            entry.client_order_ids.insert(client_order_id);
        }
        let actual_order_id = order_text(order, "actual_order_id");
        if !actual_order_id.is_empty() {
            entry.order_ids.insert(actual_order_id);
        }
        let actual_client_order_id = order_text(order, "actual_client_order_id");
        if !actual_client_order_id.is_empty() {
            entry.client_order_ids.insert(actual_client_order_id);
        }
    }
    by_mode.retain(|_, identities| {
        !identities.order_ids.is_empty() || !identities.client_order_ids.is_empty()
    });
    by_mode
}

async fn query_matching_fill_records(
    pool: &SqlitePool,
    mode: &str,
    identities: &OrderIdentities,
) -> AppResult<Vec<FillRecord>> {
    let mut records = Vec::new();
    let mut seen_ids = HashSet::<i64>::new();
    let order_ids = identities.order_ids.iter().cloned().collect::<Vec<_>>();
    let client_order_ids = identities
        .client_order_ids
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let chunk_count = order_ids
        .len()
        .max(client_order_ids.len())
        .div_ceil(200)
        .max(1);
    for chunk_index in 0..chunk_count {
        let order_chunk = chunk_slice(&order_ids, chunk_index, 200);
        let client_chunk = chunk_slice(&client_order_ids, chunk_index, 200);
        if order_chunk.is_empty() && client_chunk.is_empty() {
            continue;
        }
        let rows = query_matching_fill_chunk(pool, mode, order_chunk, client_chunk).await?;
        for row in rows {
            if seen_ids.insert(row.id) {
                records.push(row);
            }
        }
    }
    Ok(records)
}

async fn query_matching_fill_chunk(
    pool: &SqlitePool,
    mode: &str,
    order_ids: &[String],
    client_order_ids: &[String],
) -> AppResult<Vec<FillRecord>> {
    let mut predicates = Vec::new();
    if !order_ids.is_empty() {
        predicates.push(format!(
            "(LENGTH(TRIM(COALESCE(order_id, ''))) > 0 AND order_id IN ({}))",
            placeholders(order_ids.len())
        ));
    }
    if !client_order_ids.is_empty() {
        predicates.push(format!(
            "(LENGTH(TRIM(COALESCE(client_order_id, ''))) > 0 AND client_order_id IN ({}))",
            placeholders(client_order_ids.len())
        ));
    }
    if predicates.is_empty() {
        return Ok(Vec::new());
    }
    let sql = format!(
        r#"
        SELECT id, mode, order_id, client_order_id,
               fill_px, fill_sz, fee, fee_ccy, ts, source
        FROM local_fills
        WHERE mode = ?
          AND ({})
        ORDER BY ts DESC, id DESC
        LIMIT 5000
        "#,
        predicates.join(" OR ")
    );
    let mut query = sqlx::query(&sql).bind(mode);
    for value in order_ids {
        query = query.bind(value);
    }
    for value in client_order_ids {
        query = query.bind(value);
    }
    query
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(fill_record_from_row)
        .collect::<AppResult<Vec<_>>>()
}

fn fill_record_from_row(row: SqliteRow) -> AppResult<FillRecord> {
    Ok(FillRecord {
        id: row.try_get::<i64, _>("id")?,
        mode: required_row_text(&row, "mode")?,
        order_id: optional_row_text(&row, "order_id")?,
        client_order_id: optional_row_text(&row, "client_order_id")?,
        fill_px: row_text_positive_f64(&row, "fill_px")?,
        fill_sz: row_text_positive_f64(&row, "fill_sz")?,
        fee: row_text_f64(&row, "fee")?,
        fee_ccy: optional_row_text(&row, "fee_ccy")?,
        ts: Some(required_positive_i64(&row, "ts")?),
        source: optional_row_text(&row, "source")?,
    })
}

fn chunk_slice<T>(items: &[T], chunk_index: usize, chunk_size: usize) -> &[T] {
    let start = chunk_index.saturating_mul(chunk_size);
    if start >= items.len() {
        return &[];
    }
    let end = (start + chunk_size).min(items.len());
    &items[start..end]
}

fn placeholders(count: usize) -> String {
    std::iter::repeat("?")
        .take(count)
        .collect::<Vec<_>>()
        .join(",")
}

fn required_row_text(row: &SqliteRow, column: &str) -> AppResult<String> {
    let value = row.try_get::<String, _>(column)?;
    if value.trim().is_empty() {
        Err(AppError::Runtime(format!("local fill {column} 不能为空")))
    } else {
        Ok(value)
    }
}

fn optional_row_text(row: &SqliteRow, column: &str) -> AppResult<String> {
    Ok(row
        .try_get::<Option<String>, _>(column)?
        .unwrap_or_default())
}

fn row_text_f64(row: &SqliteRow, column: &str) -> AppResult<Option<f64>> {
    let value = row.try_get::<String, _>(column)?;
    Ok(parse_finite_f64(&value))
}

fn row_text_positive_f64(row: &SqliteRow, column: &str) -> AppResult<Option<f64>> {
    Ok(row_text_f64(row, column)?.filter(|value| *value > 0.0))
}

fn required_positive_i64(row: &SqliteRow, column: &str) -> AppResult<i64> {
    let value = row.try_get::<i64, _>(column)?;
    if value > 0 {
        Ok(value)
    } else {
        Err(AppError::Runtime(format!(
            "local fill {column} 必须是正整数"
        )))
    }
}

fn parse_finite_f64(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn order_text(order: &Value, field: &str) -> String {
    order
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn order_positive_f64(order: &Value, field: &str) -> Option<f64> {
    order
        .get(field)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
}
