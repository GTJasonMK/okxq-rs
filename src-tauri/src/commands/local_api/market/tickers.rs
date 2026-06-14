use std::collections::BTreeSet;

use serde_json::{json, Value};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use crate::{
    app_state::AppState,
    error::AppResult,
    market_candle_rows::{non_negative_row_f64, positive_row_f64, positive_row_i64},
};

use super::super::*;

pub(in crate::commands::local_api) async fn market_ticker(
    state: &AppState,
    inst_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_type = param_string(req, "inst_type", &infer_inst_type(inst_id));
    let fresh = param_bool(req, "fresh", true);
    let (inst_id, inst_type) = resolve_watched_market_inst(state, inst_id, &inst_type).await?;
    if fresh {
        if let Ok(client) = okx_client(state).await {
            if let Ok(ticker) = client.get_ticker(&inst_id).await {
                if !ticker.as_object().map(|obj| obj.is_empty()).unwrap_or(true) {
                    if let Some(ticker) = normalize_okx_ticker(ticker, &inst_id, &inst_type) {
                        return Ok(code_ok(ticker));
                    }
                }
            }
        }
    }
    let data = load_latest_local_ticker(&state.db, &inst_id, &inst_type)
        .await?
        .unwrap_or_else(|| {
            json!({
                "inst_id": inst_id,
                "inst_type": inst_type,
                "message": "本地行情暂不可用"
            })
        });
    Ok(code_ok(data))
}

async fn load_latest_local_ticker(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
) -> AppResult<Option<Value>> {
    let row = sqlx::query(
        r#"
        SELECT inst_id, inst_type, timestamp, open, high, low, close, volume
        FROM candles INDEXED BY idx_candles_scope_time
        WHERE inst_id = ? AND inst_type = ?
          AND timestamp > 0
          AND typeof(open) IN ('integer', 'real') AND open > 0
          AND typeof(high) IN ('integer', 'real') AND high > 0
          AND typeof(low) IN ('integer', 'real') AND low > 0
          AND typeof(close) IN ('integer', 'real') AND close > 0
          AND typeof(volume) IN ('integer', 'real') AND volume >= 0
          AND typeof(volume_ccy) IN ('integer', 'real') AND volume_ccy >= 0
        ORDER BY timestamp DESC
        LIMIT 1
        "#,
    )
    .bind(inst_id)
    .bind(inst_type)
    .fetch_optional(db)
    .await?;

    Ok(row.and_then(ticker_from_candle_row))
}

pub(in crate::commands::local_api) async fn market_tickers(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let requested_inst_type = param_string(req, "inst_type", "");
    let requested_inst_type = if requested_inst_type.trim().is_empty() {
        None
    } else {
        Some(normalize_market_inst_type(&requested_inst_type, "")?)
    };
    let mut scopes = enabled_market_scopes(state).await?;
    if let Some(inst_type) = requested_inst_type.as_deref() {
        scopes.retain(|scope| scope.inst_type == inst_type);
    }
    if scopes.is_empty() {
        return Ok(code_ok(Value::Array(Vec::new())));
    }
    let enabled = scope_keys(&scopes);
    let fresh = param_bool(req, "fresh", false);
    if fresh {
        if let Ok(client) = okx_client(state).await {
            let mut tickers = Vec::new();
            for scope in &scopes {
                match client.get_ticker(&scope.inst_id).await {
                    Ok(ticker) => {
                        if !ticker.as_object().map(|obj| obj.is_empty()).unwrap_or(true) {
                            if let Some(ticker) =
                                normalize_okx_ticker(ticker, &scope.inst_id, &scope.inst_type)
                            {
                                tickers.push(ticker);
                            }
                        }
                    }
                    Err(error) => tracing::warn!(
                        inst_id = scope.inst_id.as_str(),
                        inst_type = scope.inst_type.as_str(),
                        error = %error,
                        "failed to refresh watched ticker"
                    ),
                }
            }
            if !tickers.is_empty() {
                return Ok(code_ok(Value::Array(tickers)));
            }
        }
    }
    let data = load_latest_local_tickers(&state.db, &enabled).await?;
    Ok(code_ok(Value::Array(data)))
}

async fn load_latest_local_tickers(
    db: &SqlitePool,
    enabled: &BTreeSet<(String, String)>,
) -> AppResult<Vec<Value>> {
    if enabled.is_empty() {
        return Ok(Vec::new());
    }

    let mut query = QueryBuilder::<Sqlite>::new(
        r#"
        WITH enabled(inst_id, inst_type) AS (
        "#,
    );
    query.push_values(enabled.iter(), |mut row, (inst_id, inst_type)| {
        row.push_bind(inst_id).push_bind(inst_type);
    });
    query.push(
        r#"
        )
        SELECT c.inst_id, c.inst_type, c.timestamp, c.open, c.high, c.low, c.close, c.volume
        FROM enabled e
        INNER JOIN candles c
          ON c.id = (
            SELECT latest.id
            FROM candles latest INDEXED BY idx_candles_scope_time
            WHERE latest.inst_id = e.inst_id
              AND latest.inst_type = e.inst_type
              AND latest.timestamp > 0
              AND typeof(latest.open) IN ('integer', 'real') AND latest.open > 0
              AND typeof(latest.high) IN ('integer', 'real') AND latest.high > 0
              AND typeof(latest.low) IN ('integer', 'real') AND latest.low > 0
              AND typeof(latest.close) IN ('integer', 'real') AND latest.close > 0
              AND typeof(latest.volume) IN ('integer', 'real') AND latest.volume >= 0
              AND typeof(latest.volume_ccy) IN ('integer', 'real') AND latest.volume_ccy >= 0
            ORDER BY latest.timestamp DESC
            LIMIT 1
          )
        ORDER BY c.inst_id
        "#,
    );

    let rows = query.build().fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .filter_map(ticker_from_candle_row)
        .collect::<Vec<_>>())
}

fn ticker_from_candle_row(row: sqlx::sqlite::SqliteRow) -> Option<Value> {
    let close = positive_row_f64(&row, "close")?;
    let open = positive_row_f64(&row, "open")?;
    let high = positive_row_f64(&row, "high")?;
    let low = positive_row_f64(&row, "low")?;
    let volume = non_negative_row_f64(&row, "volume")?;
    let timestamp = positive_row_i64(&row, "timestamp")?;
    Some(json!({
        "inst_id": row.try_get::<String, _>("inst_id").ok()?,
        "inst_type": row.try_get::<String, _>("inst_type").ok()?,
        "last": close,
        "ask": close,
        "bid": close,
        "open24h": open,
        "high24h": high,
        "low24h": low,
        "vol24h": volume,
        "change24h": 0.0,
        "ts": timestamp
    }))
}

fn normalize_okx_ticker(
    ticker: Value,
    request_inst_id: &str,
    request_inst_type: &str,
) -> Option<Value> {
    let inst_id = ticker
        .get("instId")
        .and_then(Value::as_str)
        .unwrap_or(request_inst_id);
    let last_price = positive_ticker_f64(&ticker, "last")?;
    Some(json!({
        "inst_id": inst_id,
        "inst_type": ticker.get("instType").and_then(Value::as_str).unwrap_or(request_inst_type),
        "last": last_price,
        "ask": positive_ticker_f64(&ticker, "askPx").unwrap_or(last_price),
        "bid": positive_ticker_f64(&ticker, "bidPx").unwrap_or(last_price),
        "open24h": positive_ticker_f64(&ticker, "open24h").unwrap_or(last_price),
        "high24h": positive_ticker_f64(&ticker, "high24h").unwrap_or(last_price),
        "low24h": positive_ticker_f64(&ticker, "low24h").unwrap_or(last_price),
        "vol24h": ticker.get("vol24h").and_then(Value::as_str).unwrap_or("0").parse::<f64>().unwrap_or(0.0),
        "change24h": calculate_change_24h(&ticker, last_price),
        "ts": ticker.get("ts").and_then(Value::as_str).and_then(|item| item.parse::<i64>().ok()).unwrap_or(0)
    }))
}

fn positive_ticker_f64(ticker: &Value, key: &str) -> Option<f64> {
    let value = ticker
        .get(key)
        .and_then(Value::as_str)
        .and_then(|item| item.parse::<f64>().ok())?;
    (value.is_finite() && value > 0.0).then_some(value)
}

fn calculate_change_24h(ticker: &Value, last: f64) -> f64 {
    let open = positive_ticker_f64(ticker, "open24h").unwrap_or(0.0);
    if open > 0.0 {
        (last - open) / open * 100.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::json;
    use sqlx::{sqlite::SqlitePoolOptions, QueryBuilder, Row, Sqlite, SqlitePool};

    use super::{
        load_latest_local_ticker, load_latest_local_tickers, normalize_okx_ticker,
        ticker_from_candle_row,
    };

    #[test]
    fn normalize_okx_ticker_does_not_fabricate_zero_from_invalid_last() {
        let ticker = normalize_okx_ticker(
            json!({
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "last": "bad-last",
                "askPx": "70100",
                "bidPx": "70000",
                "open24h": "69900",
                "ts": "1700000000000"
            }),
            "BTC-USDT-SWAP",
            "SWAP",
        );

        assert!(ticker.is_none());
    }

    #[tokio::test]
    async fn ticker_from_candle_row_does_not_fabricate_zero_from_invalid_close_text() {
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
              1700000000000 AS timestamp,
              100.0 AS open,
              101.0 AS high,
              99.0 AS low,
              'bad-close' AS close,
              12.0 AS volume
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("test row");

        assert!(ticker_from_candle_row(row).is_none());
    }

    #[tokio::test]
    async fn ticker_from_candle_row_rejects_invalid_timestamp_instead_of_fabricating_epoch() {
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
              'bad-ts' AS timestamp,
              100.0 AS open,
              101.0 AS high,
              99.0 AS low,
              100.0 AS close,
              12.0 AS volume
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("test row");

        assert!(ticker_from_candle_row(row).is_none());
    }

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        sqlx::query(
            r#"
            CREATE TABLE candles (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              inst_id TEXT NOT NULL,
              inst_type TEXT NOT NULL DEFAULT 'SPOT',
              timeframe TEXT NOT NULL DEFAULT '1m',
              timestamp INTEGER NOT NULL,
              open REAL NOT NULL,
              high REAL NOT NULL,
              low REAL NOT NULL,
              close REAL NOT NULL,
              volume REAL NOT NULL,
              volume_ccy REAL DEFAULT 0,
              created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
              UNIQUE(inst_id, inst_type, timeframe, timestamp)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create candles table");
        sqlx::query(
            "CREATE INDEX idx_candles_scope_time ON candles(inst_id, inst_type, timestamp DESC)",
        )
        .execute(&pool)
        .await
        .expect("create candles scope timestamp index");
        pool
    }

    async fn insert_candle(pool: &SqlitePool, timestamp: i64, close_sql: &str) {
        insert_candle_for_scope(pool, "BTC-USDT-SWAP", "SWAP", timestamp, close_sql).await;
    }

    async fn insert_candle_for_scope(
        pool: &SqlitePool,
        inst_id: &str,
        inst_type: &str,
        timestamp: i64,
        close_sql: &str,
    ) {
        let sql = format!(
            r#"
            INSERT INTO candles (
              inst_id, inst_type, timeframe, timestamp,
              open, high, low, close, volume, volume_ccy
            ) VALUES (?, ?, '1m', ?, 100, 101, 99, {close_sql}, 1, 1)
            "#
        );
        sqlx::query(&sql)
            .bind(inst_id)
            .bind(inst_type)
            .bind(timestamp)
            .execute(pool)
            .await
            .expect("insert candle");
    }

    #[tokio::test]
    async fn latest_local_ticker_uses_latest_valid_candle() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, "100").await;
        insert_candle(&pool, 120_000, "'bad-close'").await;

        let ticker = load_latest_local_ticker(&pool, "BTC-USDT-SWAP", "SWAP")
            .await
            .expect("load local ticker")
            .expect("ticker");

        assert_eq!(ticker["last"].as_f64(), Some(100.0));
        assert_eq!(ticker["ts"].as_i64(), Some(60_000));
    }

    #[tokio::test]
    async fn latest_local_tickers_use_latest_valid_candle_per_enabled_scope() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, "100").await;
        insert_candle(&pool, 120_000, "'bad-close'").await;
        let enabled = BTreeSet::from([("BTC-USDT-SWAP".to_string(), "SWAP".to_string())]);

        let tickers = load_latest_local_tickers(&pool, &enabled)
            .await
            .expect("load local tickers");

        assert_eq!(tickers.len(), 1);
        assert_eq!(tickers[0]["last"].as_f64(), Some(100.0));
        assert_eq!(tickers[0]["ts"].as_i64(), Some(60_000));
    }

    #[tokio::test]
    async fn latest_local_tickers_filters_enabled_scopes_in_sql() {
        let pool = test_pool().await;
        insert_candle_for_scope(&pool, "BTC-USDT-SWAP", "SWAP", 60_000, "100").await;
        insert_candle_for_scope(&pool, "ETH-USDT-SWAP", "SWAP", 120_000, "200").await;
        let enabled = BTreeSet::from([("BTC-USDT-SWAP".to_string(), "SWAP".to_string())]);

        let tickers = load_latest_local_tickers(&pool, &enabled)
            .await
            .expect("load local tickers");

        assert_eq!(tickers.len(), 1);
        assert_eq!(tickers[0]["inst_id"].as_str(), Some("BTC-USDT-SWAP"));
        assert_eq!(tickers[0]["last"].as_f64(), Some(100.0));
    }

    #[tokio::test]
    async fn latest_local_tickers_use_scope_time_index_for_recent_lookup() {
        let pool = test_pool().await;
        let enabled = BTreeSet::from([
            ("BTC-USDT-SWAP".to_string(), "SWAP".to_string()),
            ("ETH-USDT-SWAP".to_string(), "SWAP".to_string()),
        ]);
        let mut query = QueryBuilder::<Sqlite>::new(
            r#"
            EXPLAIN QUERY PLAN
            WITH enabled(inst_id, inst_type) AS (
            "#,
        );
        query.push_values(enabled.iter(), |mut row, (inst_id, inst_type)| {
            row.push_bind(inst_id).push_bind(inst_type);
        });
        query.push(
            r#"
            )
            SELECT c.id
            FROM enabled e
            INNER JOIN candles c
              ON c.id = (
                SELECT latest.id
                FROM candles latest INDEXED BY idx_candles_scope_time
                WHERE latest.inst_id = e.inst_id
                  AND latest.inst_type = e.inst_type
                  AND latest.timestamp > 0
                ORDER BY latest.timestamp DESC
                LIMIT 1
              )
            "#,
        );

        let plan = query
            .build()
            .fetch_all(&pool)
            .await
            .expect("explain latest local ticker query")
            .into_iter()
            .map(|row| row.try_get::<String, _>("detail").unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            plan.contains("idx_candles_scope_time"),
            "latest local tickers should use scope timestamp index, got:\n{plan}"
        );
        assert!(
            !plan.contains("USE TEMP B-TREE FOR GROUP BY"),
            "latest local tickers should not group historical candles, got:\n{plan}"
        );
    }
}
