use sqlx::SqlitePool;

use crate::{
    feature_bar_rows::{
        load_latest_feature_bar_rows, non_negative_feature_payload_f64, FeatureBarTimestampMode,
    },
    market_candle_rows::{
        basic_candle_ohlcv_from_row, load_latest_basic_candle_rows, positive_row_i64,
    },
    ohlcv::Ohlcv,
};

use super::RawBar;

/// 从 feature_bars_1s 加载原始数据，按时间升序返回 (ts, open, high, low, close, volume, mid_price)。
pub(crate) async fn load_raw_bars(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    bar_count: i64,
) -> Result<Vec<RawBar>, sqlx::Error> {
    let mut bars = load_raw_feature_bars(db, inst_id, bar_count).await?;
    if bars.len() < 120 {
        bars = load_raw_candle_bars(db, inst_id, inst_type, timeframe, bar_count).await?;
    }
    Ok(bars)
}

async fn load_raw_feature_bars(
    db: &SqlitePool,
    inst_id: &str,
    bar_count: i64,
) -> Result<Vec<RawBar>, sqlx::Error> {
    Ok(load_latest_feature_bar_rows(
        db,
        inst_id,
        bar_count.clamp(1, 10000),
        FeatureBarTimestampMode::Positive,
    )
    .await?
    .into_iter()
    .filter_map(|row| raw_bar_from_feature_row(row.ts, row.ohlcv, &row.payload))
    .collect())
}

async fn load_raw_candle_bars(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    bar_count: i64,
) -> Result<Vec<RawBar>, sqlx::Error> {
    let rows =
        load_latest_basic_candle_rows(db, inst_id, inst_type, timeframe, bar_count.clamp(1, 10000))
            .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| raw_bar_from_candle_row(&row))
        .collect())
}

fn raw_bar_from_feature_row(ts: i64, ohlcv: Ohlcv, payload: &serde_json::Value) -> Option<RawBar> {
    if ts <= 0 {
        return None;
    }
    Some((
        ts,
        ohlcv.open,
        ohlcv.high,
        ohlcv.low,
        ohlcv.close,
        ohlcv.volume,
        non_negative_feature_payload_f64(payload, "mid_price").unwrap_or(0.0),
    ))
}

fn raw_bar_from_candle_row(row: &sqlx::sqlite::SqliteRow) -> Option<RawBar> {
    let timestamp = positive_row_i64(row, "timestamp")?;
    let ohlcv = basic_candle_ohlcv_from_row(row)?;
    Some((
        timestamp,
        ohlcv.open,
        ohlcv.high,
        ohlcv.low,
        ohlcv.close,
        ohlcv.volume,
        (ohlcv.high + ohlcv.low) / 2.0,
    ))
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::{load_raw_candle_bars, load_raw_feature_bars, raw_bar_from_candle_row};

    #[tokio::test]
    async fn raw_candle_row_does_not_fallback_invalid_required_price_to_close() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        let row = sqlx::query(
            r#"
            SELECT
              1700000000000 AS timestamp,
              'bad-open' AS open,
              101.0 AS high,
              99.0 AS low,
              100.0 AS close,
              12.0 AS volume
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("test row");

        assert!(raw_bar_from_candle_row(&row).is_none());
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
              open, high, low, close, volume, volume_ccy
            ) VALUES ('BTC-USDT-SWAP', 'SWAP', '1m', ?, 100, 101, 99, {close_sql}, 1, 1)
            "#
        );
        sqlx::query(&sql)
            .bind(timestamp)
            .execute(pool)
            .await
            .expect("insert candle");
    }

    async fn insert_feature_bar(pool: &SqlitePool, timestamp: i64, payload_json: &str) {
        sqlx::query(
            r#"
            INSERT INTO feature_bars_1s (inst_id, ts, payload_json, created_at)
            VALUES ('BTC-USDT-SWAP', ?, ?, 0)
            "#,
        )
        .bind(timestamp)
        .bind(payload_json)
        .execute(pool)
        .await
        .expect("insert feature bar");
    }

    #[tokio::test]
    async fn raw_candle_bars_apply_limit_after_filtering_invalid_market_rows() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, "100").await;
        insert_candle(&pool, 120_000, "101").await;
        insert_candle(&pool, 180_000, "'bad-close'").await;

        let bars = load_raw_candle_bars(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 2)
            .await
            .expect("load raw candle bars");
        let closes = bars.iter().map(|bar| bar.4).collect::<Vec<_>>();

        assert_eq!(closes, vec![100.0, 101.0]);
    }

    #[tokio::test]
    async fn raw_feature_bars_apply_limit_after_filtering_invalid_payloads() {
        let pool = test_pool().await;
        sqlx::query(
            r#"
            CREATE TABLE feature_bars_1s (
              inst_id TEXT NOT NULL,
              ts INTEGER NOT NULL,
              payload_json TEXT NOT NULL DEFAULT '{}',
              created_at REAL NOT NULL,
              PRIMARY KEY(inst_id, ts)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create feature bars table");
        insert_feature_bar(
            &pool,
            60_000,
            r#"{"open":100,"high":101,"low":99,"close":100,"volume":1,"mid_price":100}"#,
        )
        .await;
        insert_feature_bar(
            &pool,
            120_000,
            r#"{"open":101,"high":102,"low":100,"close":101,"volume":1,"mid_price":101}"#,
        )
        .await;
        insert_feature_bar(
            &pool,
            180_000,
            r#"{"open":102,"high":103,"low":101,"close":"bad-close","volume":1,"mid_price":102}"#,
        )
        .await;

        let bars = load_raw_feature_bars(&pool, "BTC-USDT-SWAP", 2)
            .await
            .expect("load raw feature bars");
        let closes = bars.iter().map(|bar| bar.4).collect::<Vec<_>>();

        assert_eq!(closes, vec![100.0, 101.0]);
    }
}
