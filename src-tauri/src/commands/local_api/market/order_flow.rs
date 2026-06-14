use serde_json::{json, Value};
use sqlx::Row;

use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    market_candle_rows::positive_row_i64,
};

use super::super::*;

pub(in crate::commands::local_api) async fn market_recent_trades(
    state: &AppState,
    inst_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_type = param_string(req, "inst_type", &infer_inst_type(inst_id));
    let limit = param_i64(req, "limit", 20).clamp(1, 200);
    let (inst_id, inst_type) = resolve_watched_market_inst(state, inst_id, &inst_type).await?;
    if let Ok(client) = okx_client(state).await {
        if let Ok(trades) = client.get_trades(&inst_id, limit as u32).await {
            return Ok(code_ok(Value::Array(trades)));
        }
    }
    let rows = sqlx::query(
        r#"
        SELECT inst_id, inst_type, trade_id, payload_json, ts, created_at
        FROM market_recent_trades
        WHERE inst_id = ? AND inst_type = ?
        ORDER BY ts DESC
        LIMIT ?
        "#,
    )
    .bind(&inst_id)
    .bind(&inst_type)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    let data = rows
        .into_iter()
        .filter_map(recent_trade_from_row)
        .collect::<Vec<_>>();
    Ok(code_ok(Value::Array(data)))
}

fn recent_trade_from_row(row: sqlx::sqlite::SqliteRow) -> Option<Value> {
    let mut payload = json_from_text(
        row.try_get::<Option<String>, _>("payload_json")
            .ok()
            .flatten(),
        json!({}),
    );
    if positive_payload_f64(&payload, "price").is_none()
        || positive_payload_f64(&payload, "size").is_none()
    {
        return None;
    }
    let timestamp =
        positive_payload_i64(&payload, "ts").or_else(|| positive_row_i64(&row, "ts"))?;
    let inst_id = non_empty_payload_string(&payload, "inst_id")
        .or_else(|| non_empty_row_string(&row, "inst_id"))?;
    let trade_id = non_empty_payload_string(&payload, "trade_id")
        .or_else(|| non_empty_row_string(&row, "trade_id"))?;
    let side = normalized_trade_side(&payload)?;
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("inst_id".to_string(), Value::String(inst_id));
        obj.entry("inst_type".to_string()).or_insert_with(|| {
            Value::String(row.try_get::<String, _>("inst_type").unwrap_or_default())
        });
        obj.insert("trade_id".to_string(), Value::String(trade_id));
        obj.insert("side".to_string(), Value::String(side.to_string()));
        obj.insert("ts".to_string(), Value::Number(timestamp.into()));
    }
    Some(payload)
}

fn positive_payload_f64(payload: &Value, key: &str) -> Option<f64> {
    let value = payload.get(key).and_then(Value::as_f64)?;
    (value.is_finite() && value > 0.0).then_some(value)
}

fn positive_payload_i64(payload: &Value, key: &str) -> Option<i64> {
    let value = payload.get(key)?;
    let parsed = value
        .as_i64()
        .or_else(|| value.as_str()?.parse::<i64>().ok())?;
    (parsed > 0).then_some(parsed)
}

fn non_empty_payload_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn non_empty_row_string(row: &sqlx::sqlite::SqliteRow, column: &str) -> Option<String> {
    row.try_get::<String, _>(column)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalized_trade_side(payload: &Value) -> Option<&'static str> {
    match non_empty_payload_string(payload, "side")?
        .to_ascii_lowercase()
        .as_str()
    {
        "buy" => Some("buy"),
        "sell" => Some("sell"),
        _ => None,
    }
}

pub(in crate::commands::local_api) async fn market_orderbook(
    state: &AppState,
    inst_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_type = param_string(req, "inst_type", &infer_inst_type(inst_id));
    let size = param_i64(req, "size", 20).clamp(1, 5000);
    let (inst_id, inst_type) = resolve_watched_market_inst(state, inst_id, &inst_type).await?;
    let client = okx_client(state).await?;
    let book = match client.get_orderbook(&inst_id, size as u32).await {
        Ok(book) => book,
        Err(error) => return unavailable_orderbook_result(&inst_id, &inst_type, error),
    };
    Ok(code_ok(book))
}

fn unavailable_orderbook_result(
    inst_id: &str,
    inst_type: &str,
    error: impl std::fmt::Display,
) -> AppResult<Value> {
    Err(AppError::Runtime(format!(
        "OKX 盘口不可用: inst_id={inst_id}, inst_type={inst_type}: {error}"
    )))
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

    use super::{recent_trade_from_row, unavailable_orderbook_result};

    #[tokio::test]
    async fn recent_trade_row_does_not_expose_invalid_price_or_size_payload() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        let row = sqlx::query(
            r#"
            SELECT
              'bad-price' AS trade_id,
              '{"inst_id":"BTC-USDT-SWAP","trade_id":"bad-price","price":"bad-price","size":1,"side":"buy","ts":1700000000000}' AS payload_json,
              1700000000000 AS ts,
              '2026-06-05T00:00:00Z' AS created_at
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("test row");

        assert!(recent_trade_from_row(row).is_none());
    }

    #[tokio::test]
    async fn recent_trade_row_keeps_valid_payload_and_row_fallback_fields() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        let row = sqlx::query(
            r#"
            SELECT
              'BTC-USDT-SWAP' AS inst_id,
              'SWAP' AS inst_type,
              'valid-trade' AS trade_id,
              '{"price":70000.5,"size":2,"side":"sell"}' AS payload_json,
              1700000000000 AS ts,
              '2026-06-05T00:00:00Z' AS created_at
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("test row");

        let trade = recent_trade_from_row(row).expect("valid trade");

        assert_eq!(trade["inst_id"], "BTC-USDT-SWAP");
        assert_eq!(trade["inst_type"], "SWAP");
        assert_eq!(trade["trade_id"], "valid-trade");
        assert_eq!(trade["price"], 70000.5);
        assert_eq!(trade["size"], 2);
        assert_eq!(trade["ts"], 1700000000000_i64);
    }

    #[tokio::test]
    async fn recent_trade_row_rejects_invalid_timestamp_instead_of_fabricating_epoch() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        let row = sqlx::query(
            r#"
            SELECT
              'BTC-USDT-SWAP' AS inst_id,
              'SWAP' AS inst_type,
              'bad-ts-trade' AS trade_id,
              '{"price":70000.5,"size":2,"side":"sell"}' AS payload_json,
              'bad-ts' AS ts,
              '2026-06-05T00:00:00Z' AS created_at
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("test row");

        assert!(recent_trade_from_row(row).is_none());
    }

    #[tokio::test]
    async fn recent_trade_row_rejects_missing_trade_id_or_invalid_side() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        let missing_trade_id = sqlx::query(
            r#"
            SELECT
              'BTC-USDT-SWAP' AS inst_id,
              'SWAP' AS inst_type,
              '' AS trade_id,
              '{"price":70000.5,"size":2,"side":"buy","ts":1700000000000}' AS payload_json,
              1700000000000 AS ts,
              '2026-06-05T00:00:00Z' AS created_at
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("missing trade id row");
        let invalid_side = sqlx::query(
            r#"
            SELECT
              'BTC-USDT-SWAP' AS inst_id,
              'SWAP' AS inst_type,
              'bad-side' AS trade_id,
              '{"price":70000.5,"size":2,"side":"hold","ts":1700000000000}' AS payload_json,
              1700000000000 AS ts,
              '2026-06-05T00:00:00Z' AS created_at
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("invalid side row");

        assert!(recent_trade_from_row(missing_trade_id).is_none());
        assert!(recent_trade_from_row(invalid_side).is_none());
    }

    #[test]
    fn unavailable_orderbook_result_is_error_not_zero_timestamp_success() {
        let error = unavailable_orderbook_result("BTC-USDT-SWAP", "SWAP", "network down")
            .expect_err("unavailable OKX orderbook must not return success payload");

        let message = error.to_string();
        assert!(message.contains("BTC-USDT-SWAP"));
        assert!(message.contains("network down"));
    }

    #[tokio::test]
    async fn recent_trades_query_uses_scope_recent_index() {
        let db_path = temp_db_path("recent_trades_scope_recent_index");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");

        let plan = recent_trades_query_plan(&pool).await.join("\n");

        assert!(
            plan.contains("idx_market_trades_scope_recent"),
            "recent trades query should use scope/recent index, got:\n{plan}"
        );
        assert!(
            !plan.contains("USE TEMP B-TREE"),
            "recent trades query should not sort with temp b-tree, got:\n{plan}"
        );

        cleanup_db(pool, &db_path).await;
    }

    fn temp_db_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("okxq_{name}_{}_{}", std::process::id(), suffix))
            .join("market.db")
    }

    async fn recent_trades_query_plan(pool: &SqlitePool) -> Vec<String> {
        sqlx::query(
            r#"
            EXPLAIN QUERY PLAN
            SELECT inst_id, inst_type, trade_id, payload_json, ts, created_at
            FROM market_recent_trades
            WHERE inst_id = ? AND inst_type = ?
            ORDER BY ts DESC
            LIMIT ?
            "#,
        )
        .bind("BTC-USDT-SWAP")
        .bind("SWAP")
        .bind(50_i64)
        .fetch_all(pool)
        .await
        .expect("recent trades explain")
        .into_iter()
        .map(|row| row.try_get::<String, _>("detail").unwrap_or_default())
        .collect()
    }

    async fn cleanup_db(pool: SqlitePool, db_path: &std::path::Path) {
        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }
}
