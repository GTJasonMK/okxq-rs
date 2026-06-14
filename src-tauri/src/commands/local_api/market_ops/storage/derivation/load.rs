use sqlx::SqlitePool;

use crate::{
    market_candle_rows::{
        load_valid_candle_rows_in_range, load_valid_candle_rows_since, okx_candle_from_row,
    },
    okx::OkxCandle,
};

use super::super::super::{check_sync_cancel, AppResult, SyncCancelGuard};

pub(super) async fn load_base_candles(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    min_timestamp: Option<i64>,
    cancel_guard: Option<&SyncCancelGuard>,
) -> AppResult<Vec<OkxCandle>> {
    check_sync_cancel(cancel_guard).await?;
    let rows =
        load_valid_candle_rows_since(pool, inst_id, inst_type, timeframe, min_timestamp).await?;
    okx_candles_from_rows_with_cancel(rows, cancel_guard).await
}

pub(super) async fn load_base_candles_range(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    start_ts: i64,
    end_ts: i64,
    cancel_guard: Option<&SyncCancelGuard>,
) -> AppResult<Vec<OkxCandle>> {
    check_sync_cancel(cancel_guard).await?;
    let rows = load_valid_candle_rows_in_range(
        pool, inst_id, inst_type, timeframe, start_ts, end_ts, None,
    )
    .await?;

    okx_candles_from_rows_with_cancel(rows, cancel_guard).await
}

async fn okx_candles_from_rows_with_cancel(
    rows: Vec<sqlx::sqlite::SqliteRow>,
    cancel_guard: Option<&SyncCancelGuard>,
) -> AppResult<Vec<OkxCandle>> {
    let mut candles = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        if index % 10_000 == 0 {
            check_sync_cancel(cancel_guard).await?;
        }
        if let Some(candle) = okx_candle_from_row(&row, "1") {
            candles.push(candle);
        }
    }
    Ok(candles)
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite pool");
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

    async fn insert_candle(pool: &SqlitePool, timestamp: i64, open: f64) {
        sqlx::query(
            r#"
            INSERT INTO candles (
              inst_id, inst_type, timeframe, timestamp,
              open, high, low, close, volume, volume_ccy, volume_quote
            ) VALUES ('BTC-USDT-SWAP', 'SWAP', '1m', ?, ?, 101, 99, 100, 1, 1, 1)
            "#,
        )
        .bind(timestamp)
        .bind(open)
        .execute(pool)
        .await
        .expect("insert candle");
    }

    #[tokio::test]
    async fn load_base_candles_skips_invalid_market_rows() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, 0.0).await;
        insert_candle(&pool, 120_000, 100.0).await;

        let candles = load_base_candles(&pool, "BTC-USDT-SWAP", "SWAP", "1m", None, None)
            .await
            .expect("load base candles");

        assert_eq!(candles.len(), 1);
        assert_eq!(candles[0].timestamp, 120_000);
        assert_eq!(candles[0].open, 100.0);
    }

    #[tokio::test]
    async fn load_base_candles_range_skips_invalid_market_rows() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, 0.0).await;
        insert_candle(&pool, 120_000, 100.0).await;

        let candles =
            load_base_candles_range(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 0, 180_000, None)
                .await
                .expect("load base candle range");

        assert_eq!(candles.len(), 1);
        assert_eq!(candles[0].timestamp, 120_000);
        assert_eq!(candles[0].open, 100.0);
    }
}
