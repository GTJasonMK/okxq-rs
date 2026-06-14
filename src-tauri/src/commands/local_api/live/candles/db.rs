use sqlx::SqlitePool;

use crate::{
    app_state::AppState,
    error::AppResult,
    market_candle_rows::{load_latest_valid_candle_rows, okx_candle_from_row},
    okx::OkxCandle,
};

use super::canonical_diagnostic_timeframe;

pub(in crate::commands::local_api::live) async fn load_latest_diagnostic_candles(
    state: &AppState,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
) -> AppResult<Vec<OkxCandle>> {
    load_latest_diagnostic_candles_from_pool(&state.db, symbol, inst_type, timeframe, limit).await
}

async fn load_latest_diagnostic_candles_from_pool(
    pool: &SqlitePool,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
) -> AppResult<Vec<OkxCandle>> {
    let timeframe = canonical_diagnostic_timeframe(timeframe);
    Ok(
        load_latest_valid_candle_rows(pool, symbol, inst_type, &timeframe, limit)
            .await?
            .into_iter()
            .filter_map(|row| okx_candle_from_row(&row, "1"))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    #[tokio::test]
    async fn load_latest_diagnostic_candles_matches_canonical_okx_timeframe() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(":memory:")
            .await
            .expect("connect sqlite");
        create_candles_table(&pool).await;
        for timestamp in [60_000_i64, 120_000, 180_000] {
            insert_candle(&pool, "1H", timestamp).await;
        }

        let candles =
            load_latest_diagnostic_candles_from_pool(&pool, "BTC-USDT-SWAP", "SWAP", "1h", 3)
                .await
                .expect("load candles");

        assert_eq!(candles.len(), 3);
        assert_eq!(candles[0].timestamp, 60_000);
        assert_eq!(candles[2].timestamp, 180_000);
    }

    #[tokio::test]
    async fn load_latest_diagnostic_candles_requires_and_preserves_quote_volume() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(":memory:")
            .await
            .expect("connect sqlite");
        create_candles_table(&pool).await;
        insert_candle_without_quote_volume(&pool, 60_000).await;
        insert_candle_with_quote_volume(&pool, 120_000, 1_225.0).await;

        let candles =
            load_latest_diagnostic_candles_from_pool(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 3)
                .await
                .expect("load candles");

        assert_eq!(candles.len(), 1);
        assert_eq!(candles[0].timestamp, 120_000);
        assert_eq!(candles[0].volume_ccy, 100.5);
        assert_eq!(candles[0].volume_quote, 1_225.0);
    }

    async fn create_candles_table(pool: &SqlitePool) {
        sqlx::query(
            r#"
            CREATE TABLE candles (
                inst_id TEXT NOT NULL,
                inst_type TEXT NOT NULL,
                timeframe TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                open REAL NOT NULL,
                high REAL NOT NULL,
                low REAL NOT NULL,
                close REAL NOT NULL,
                volume REAL NOT NULL,
                volume_ccy REAL NOT NULL,
                volume_quote REAL
            )
            "#,
        )
        .execute(pool)
        .await
        .expect("create candles table");
    }

    async fn insert_candle(pool: &SqlitePool, timeframe: &str, timestamp: i64) {
        sqlx::query(
            r#"
            INSERT INTO candles (
                inst_id, inst_type, timeframe, timestamp,
                open, high, low, close, volume, volume_ccy, volume_quote
            ) VALUES ('BTC-USDT-SWAP', 'SWAP', ?, ?, 100.0, 101.0, 99.0, 100.5, 1.0, 100.5, 100.5)
            "#,
        )
        .bind(timeframe)
        .bind(timestamp)
        .execute(pool)
        .await
        .expect("insert candle");
    }

    async fn insert_candle_without_quote_volume(pool: &SqlitePool, timestamp: i64) {
        sqlx::query(
            r#"
            INSERT INTO candles (
                inst_id, inst_type, timeframe, timestamp,
                open, high, low, close, volume, volume_ccy
            ) VALUES ('BTC-USDT-SWAP', 'SWAP', '1m', ?, 100.0, 101.0, 99.0, 100.5, 1.0, 100.5)
            "#,
        )
        .bind(timestamp)
        .execute(pool)
        .await
        .expect("insert candle without quote volume");
    }

    async fn insert_candle_with_quote_volume(pool: &SqlitePool, timestamp: i64, volume_quote: f64) {
        sqlx::query(
            r#"
            INSERT INTO candles (
                inst_id, inst_type, timeframe, timestamp,
                open, high, low, close, volume, volume_ccy, volume_quote
            ) VALUES ('BTC-USDT-SWAP', 'SWAP', '1m', ?, 100.0, 101.0, 99.0, 100.5, 1.0, 100.5, ?)
            "#,
        )
        .bind(timestamp)
        .bind(volume_quote)
        .execute(pool)
        .await
        .expect("insert candle with quote volume");
    }
}
