use serde_json::Value;

use crate::{
    app_state::AppState, error::AppResult, market_candle_rows::load_latest_valid_candle_json,
};

use super::super::market_ops;
use super::super::*;

pub(in crate::commands::local_api) async fn market_candles(
    state: &AppState,
    inst_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let timeframe = param_string(req, "timeframe", "1H");
    let limit = param_i64(req, "limit", 100).clamp(1, 100_000);
    let inst_type = param_string(req, "inst_type", &infer_inst_type(inst_id));
    let fresh = param_bool(req, "fresh", false);
    let (inst_id, inst_type) = resolve_watched_market_inst(state, inst_id, &inst_type).await?;
    let sync_result = market_ops::ensure_local_candles_for_read(
        state, &inst_id, &inst_type, &timeframe, limit, fresh,
    )
    .await?;
    let data = load_market_candles(&state.db, &inst_id, &inst_type, &timeframe, limit).await?;
    let mut response = code_ok(Value::Array(data));
    if let Some(sync_result) = sync_result {
        if let Some(obj) = response.as_object_mut() {
            obj.insert("sync_result".to_string(), sync_result);
        }
    }
    Ok(response)
}

async fn load_market_candles(
    db: &sqlx::SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
) -> AppResult<Vec<Value>> {
    load_latest_valid_candle_json(db, inst_id, inst_type, timeframe, limit).await
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::load_market_candles;
    use crate::market_candle_rows::candle_json_from_row;

    #[tokio::test]
    async fn candle_row_does_not_fabricate_zero_from_invalid_required_price_text() {
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
              '1m' AS timeframe,
              1700000000000 AS timestamp,
              'bad-open' AS open,
              101.0 AS high,
              99.0 AS low,
              100.0 AS close,
              12.0 AS volume,
              1200.0 AS volume_ccy
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("test row");

        assert!(candle_json_from_row(&row).is_none());
    }

    #[tokio::test]
    async fn candle_row_rejects_invalid_volume_instead_of_fabricating_zero() {
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
              '1m' AS timeframe,
              1700000000000 AS timestamp,
              100.0 AS open,
              101.0 AS high,
              99.0 AS low,
              100.0 AS close,
              'bad-volume' AS volume,
              1200.0 AS volume_ccy
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("test row");

        assert!(candle_json_from_row(&row).is_none());
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
              timeframe TEXT NOT NULL,
              timestamp INTEGER NOT NULL,
              open REAL NOT NULL,
              high REAL NOT NULL,
              low REAL NOT NULL,
              close REAL NOT NULL,
              volume REAL NOT NULL,
              volume_ccy REAL DEFAULT 0,
              volume_quote REAL,
              created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
              UNIQUE(inst_id, inst_type, timeframe, timestamp)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create candles table");
        pool
    }

    async fn insert_candle(pool: &SqlitePool, timestamp: i64, close_sql: &str) {
        let sql = format!(
            r#"
            INSERT INTO candles (
              inst_id, inst_type, timeframe, timestamp,
              open, high, low, close, volume, volume_ccy, volume_quote
            ) VALUES ('BTC-USDT-SWAP', 'SWAP', '1m', ?, 100, 101, 99, {close_sql}, 1, 1, 1)
            "#
        );
        sqlx::query(&sql)
            .bind(timestamp)
            .execute(pool)
            .await
            .expect("insert candle");
    }

    #[tokio::test]
    async fn market_candles_apply_limit_after_filtering_invalid_market_rows() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, "100").await;
        insert_candle(&pool, 120_000, "101").await;
        insert_candle(&pool, 180_000, "'bad-close'").await;

        let candles = load_market_candles(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 2)
            .await
            .expect("load market candles");
        let closes = candles
            .iter()
            .map(|candle| candle["close"].as_f64().expect("close"))
            .collect::<Vec<_>>();

        assert_eq!(closes, vec![100.0, 101.0]);
        assert_eq!(candles[0]["volume_quote"].as_f64(), Some(1.0));
    }
}
