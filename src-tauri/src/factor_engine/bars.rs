use sqlx::SqlitePool;

use crate::{
    feature_bar_rows::{load_latest_feature_bar_rows, FeatureBarTimestampMode},
    market_candle_rows::{basic_candle_ohlcv_from_row, load_latest_basic_candle_rows},
    ohlcv::Ohlcv,
};

pub(super) type FactorBar = (f64, f64, f64, f64, f64);

/// 从 feature_bars_1s 表读取指定币种的秒级 OHLCV 数据。
pub async fn load_feature_bars(
    db: &SqlitePool,
    inst_id: &str,
    bar_count: i64,
) -> Result<Vec<FactorBar>, sqlx::Error> {
    Ok(load_latest_feature_bar_rows(
        db,
        inst_id,
        bar_count.clamp(1, 500),
        FeatureBarTimestampMode::Any,
    )
    .await?
    .into_iter()
    .map(|row| factor_bar_from_ohlcv(row.ohlcv))
    .collect())
}

pub(super) async fn load_factor_bars(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    bar_count: i64,
) -> Result<Vec<FactorBar>, String> {
    let mut bars = load_feature_bars(db, inst_id, bar_count)
        .await
        .map_err(|e| format!("加载 feature bars 失败: {e}"))?;
    if bars.len() < 20 {
        bars = load_candle_bars(db, inst_id, inst_type, timeframe, bar_count)
            .await
            .map_err(|e| format!("加载 candles 失败: {e}"))?;
    }
    Ok(bars)
}

async fn load_candle_bars(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    bar_count: i64,
) -> Result<Vec<FactorBar>, sqlx::Error> {
    let rows =
        load_latest_basic_candle_rows(db, inst_id, inst_type, timeframe, bar_count.clamp(1, 500))
            .await?;
    let mut bars = rows
        .into_iter()
        .filter_map(|row| basic_candle_ohlcv_from_row(&row).map(factor_bar_from_ohlcv))
        .collect::<Vec<_>>();
    bars.dedup_by(|left, right| left.3 == right.3 && left.4 == right.4);
    Ok(bars)
}

fn factor_bar_from_ohlcv(ohlcv: Ohlcv) -> FactorBar {
    (ohlcv.open, ohlcv.high, ohlcv.low, ohlcv.close, ohlcv.volume)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::{load_candle_bars, load_feature_bars};
    use crate::feature_bar_rows::feature_ohlcv_from_payload;

    #[test]
    fn feature_bar_payload_does_not_fabricate_zero_from_invalid_required_price() {
        let payload = json!({
            "open": "bad-open",
            "high": 101.0,
            "low": 99.0,
            "close": 100.0,
            "volume": 12.0
        });

        assert!(feature_ohlcv_from_payload(&payload).is_none());
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
    async fn factor_candle_bars_apply_limit_after_filtering_invalid_market_rows() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, "100").await;
        insert_candle(&pool, 120_000, "101").await;
        insert_candle(&pool, 180_000, "'bad-close'").await;

        let bars = load_candle_bars(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 2)
            .await
            .expect("load candle bars");
        let closes = bars.iter().map(|bar| bar.3).collect::<Vec<_>>();

        assert_eq!(closes, vec![100.0, 101.0]);
    }

    #[tokio::test]
    async fn feature_bars_apply_limit_after_filtering_invalid_payloads() {
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
            r#"{"open":100,"high":101,"low":99,"close":100,"volume":1}"#,
        )
        .await;
        insert_feature_bar(
            &pool,
            120_000,
            r#"{"open":101,"high":102,"low":100,"close":101,"volume":1}"#,
        )
        .await;
        insert_feature_bar(
            &pool,
            180_000,
            r#"{"open":102,"high":103,"low":101,"close":"bad-close","volume":1}"#,
        )
        .await;

        let bars = load_feature_bars(&pool, "BTC-USDT-SWAP", 2)
            .await
            .expect("load feature bars");
        let closes = bars.iter().map(|bar| bar.3).collect::<Vec<_>>();

        assert_eq!(closes, vec![100.0, 101.0]);
    }
}
