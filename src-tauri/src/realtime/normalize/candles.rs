use anyhow::{anyhow, Result};
use serde_json::Value;
use sqlx::{Row, SqlitePool};

use crate::{instrument::infer_spot_swap_inst_type, okx::parse_okx_candle};

use super::values::*;

pub(in crate::realtime) fn normalize_candle(
    candle: Value,
    arg_inst_id: &str,
    timeframe: &str,
) -> Option<Value> {
    let candle = parse_okx_candle(candle)?;
    let inst_id = arg_inst_id.trim().to_uppercase();
    if inst_id.is_empty() {
        return None;
    }
    let inst_type = infer_spot_swap_inst_type(&inst_id);
    let mut value = candle.to_json_with_confirm();
    if let Some(obj) = value.as_object_mut() {
        obj.insert("inst_id".to_string(), Value::String(inst_id));
        obj.insert(
            "inst_type".to_string(),
            Value::String(inst_type.to_string()),
        );
        obj.insert(
            "timeframe".to_string(),
            Value::String(timeframe.to_string()),
        );
    }
    Some(value)
}

pub(in crate::realtime) async fn persist_confirmed_candle(
    pool: &SqlitePool,
    candle: &Value,
) -> Result<()> {
    if candle.get("confirm").and_then(Value::as_str) != Some("1") {
        return Ok(());
    }

    let inst_id = value_string(candle, "inst_id")
        .ok_or_else(|| anyhow!("realtime candle missing inst_id"))?;
    let inst_type = value_string(candle, "inst_type")
        .unwrap_or_else(|| infer_spot_swap_inst_type(&inst_id).to_string());
    let timeframe = value_string(candle, "timeframe")
        .ok_or_else(|| anyhow!("realtime candle missing timeframe"))?;
    let timestamp = positive_value_i64(candle, "timestamp")?;
    let open = positive_value_f64(candle, "open")?;
    let high = positive_value_f64(candle, "high")?;
    let low = positive_value_f64(candle, "low")?;
    let close = positive_value_f64(candle, "close")?;
    let volume = non_negative_value_f64(candle, "volume")?;
    let volume_ccy = non_negative_value_f64(candle, "volume_ccy")?;
    let volume_quote = non_negative_value_f64(candle, "volume_quote")?;

    let insert_result = sqlx::query(
        r#"
        INSERT INTO candles (
          inst_id, inst_type, timeframe, timestamp,
          open, high, low, close, volume, volume_ccy, volume_quote
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(inst_id, inst_type, timeframe, timestamp) DO NOTHING
        "#,
    )
    .bind(&inst_id)
    .bind(&inst_type)
    .bind(&timeframe)
    .bind(timestamp)
    .bind(open)
    .bind(high)
    .bind(low)
    .bind(close)
    .bind(volume)
    .bind(volume_ccy)
    .bind(volume_quote)
    .execute(pool)
    .await?;
    let inserted = insert_result.rows_affected() > 0;

    if !inserted {
        sqlx::query(
            r#"
            UPDATE candles SET
              open = ?,
              high = ?,
              low = ?,
              close = ?,
              volume = ?,
              volume_ccy = ?,
              volume_quote = ?
            WHERE inst_id = ? AND inst_type = ? AND timeframe = ? AND timestamp = ?
            "#,
        )
        .bind(open)
        .bind(high)
        .bind(low)
        .bind(close)
        .bind(volume)
        .bind(volume_ccy)
        .bind(volume_quote)
        .bind(&inst_id)
        .bind(&inst_type)
        .bind(&timeframe)
        .bind(timestamp)
        .execute(pool)
        .await?;
    }

    refresh_realtime_sync_record(pool, &inst_id, &inst_type, &timeframe, timestamp, inserted)
        .await?;

    Ok(())
}

async fn refresh_realtime_sync_record(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    timestamp: i64,
    inserted: bool,
) -> Result<()> {
    let record = sqlx::query(
        r#"
        SELECT oldest_timestamp, newest_timestamp, candle_count
        FROM sync_records
        WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
        "#,
    )
    .bind(inst_id)
    .bind(inst_type)
    .bind(timeframe)
    .fetch_optional(pool)
    .await?;

    let can_increment = match record.as_ref() {
        Some(row) => {
            let oldest = row.try_get::<Option<i64>, _>("oldest_timestamp")?;
            let newest = row.try_get::<Option<i64>, _>("newest_timestamp")?;
            let count = row.try_get::<i64, _>("candle_count")?;
            match (oldest, newest) {
                (Some(oldest), Some(newest)) if count > 0 => {
                    inserted || (timestamp >= oldest && timestamp <= newest)
                }
                _ => false,
            }
        }
        None => false,
    };

    if !can_increment {
        return refresh_realtime_sync_record_from_candles(pool, inst_id, inst_type, timeframe)
            .await;
    }

    sqlx::query(
        r#"
        UPDATE sync_records SET
          last_sync_time = CURRENT_TIMESTAMP,
          oldest_timestamp = CASE
            WHEN oldest_timestamp IS NULL OR oldest_timestamp > ? THEN ?
            ELSE oldest_timestamp
          END,
          newest_timestamp = CASE
            WHEN newest_timestamp IS NULL OR newest_timestamp < ? THEN ?
            ELSE newest_timestamp
          END,
          candle_count = COALESCE(candle_count, 0) + ?,
          last_sync_mode = 'realtime'
        WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
        "#,
    )
    .bind(timestamp)
    .bind(timestamp)
    .bind(timestamp)
    .bind(timestamp)
    .bind(if inserted { 1 } else { 0 })
    .bind(inst_id)
    .bind(inst_type)
    .bind(timeframe)
    .execute(pool)
    .await?;

    Ok(())
}

async fn refresh_realtime_sync_record_from_candles(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO sync_records (
          inst_id, inst_type, timeframe, last_sync_time,
          oldest_timestamp, newest_timestamp, candle_count,
          history_complete, last_sync_mode
        )
        SELECT ?, ?, ?, CURRENT_TIMESTAMP,
               MIN(timestamp), MAX(timestamp), COUNT(*),
               COALESCE((
                 SELECT history_complete
                 FROM sync_records
                 WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
               ), 0),
               'realtime'
        FROM candles
        WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
        ON CONFLICT(inst_id, inst_type, timeframe) DO UPDATE SET
          last_sync_time = CURRENT_TIMESTAMP,
          oldest_timestamp = excluded.oldest_timestamp,
          newest_timestamp = excluded.newest_timestamp,
          candle_count = excluded.candle_count,
          history_complete = sync_records.history_complete,
          last_sync_mode = 'realtime'
        "#,
    )
    .bind(inst_id)
    .bind(inst_type)
    .bind(timeframe)
    .bind(inst_id)
    .bind(inst_type)
    .bind(timeframe)
    .bind(inst_id)
    .bind(inst_type)
    .bind(timeframe)
    .execute(pool)
    .await?;

    Ok(())
}

fn positive_value_f64(value: &Value, key: &str) -> Result<f64> {
    let parsed =
        parse_f64(value.get(key)).ok_or_else(|| anyhow!("realtime candle missing {key}"))?;
    if parsed.is_finite() && parsed > 0.0 {
        Ok(parsed)
    } else {
        Err(anyhow!("realtime candle invalid {key}"))
    }
}

fn positive_value_i64(value: &Value, key: &str) -> Result<i64> {
    let parsed =
        value_i64(value.get(key)).ok_or_else(|| anyhow!("realtime candle missing {key}"))?;
    if parsed > 0 {
        Ok(parsed)
    } else {
        Err(anyhow!("realtime candle invalid {key}"))
    }
}

fn non_negative_value_f64(value: &Value, key: &str) -> Result<f64> {
    let parsed =
        parse_f64(value.get(key)).ok_or_else(|| anyhow!("realtime candle missing {key}"))?;
    if parsed.is_finite() && parsed >= 0.0 {
        Ok(parsed)
    } else {
        Err(anyhow!("realtime candle invalid {key}"))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

    use super::{normalize_candle, persist_confirmed_candle};

    #[test]
    fn normalize_candle_rejects_zero_timestamp_instead_of_fabricating_epoch_bar() {
        let candle = normalize_candle(
            json!(["0", "100", "101", "99", "100", "1", "1", "1", "1"]),
            "BTC-USDT-SWAP",
            "1m",
        );

        assert!(candle.is_none());
    }

    #[test]
    fn normalize_candle_rejects_missing_volume_or_confirm_fields() {
        let missing_volume = normalize_candle(
            json!(["1700000000000", "100", "101", "99", "100"]),
            "BTC-USDT-SWAP",
            "1m",
        );
        let missing_confirm = normalize_candle(
            json!(["1700000000000", "100", "101", "99", "100", "1", "1", "1"]),
            "BTC-USDT-SWAP",
            "1m",
        );

        assert!(missing_volume.is_none());
        assert!(missing_confirm.is_none());
    }

    #[test]
    fn normalize_candle_rejects_invalid_volume_instead_of_fabricating_zero() {
        let candle = normalize_candle(
            json!([
                "1700000000000",
                "100",
                "101",
                "99",
                "100",
                "bad-volume",
                "1",
                "1",
                "1"
            ]),
            "BTC-USDT-SWAP",
            "1m",
        );

        assert!(candle.is_none());
    }

    #[test]
    fn normalize_candle_rejects_non_finite_or_non_positive_prices() {
        let non_finite = normalize_candle(
            json!([
                "1700000000000",
                "NaN",
                "101",
                "99",
                "100",
                "1",
                "1",
                "1",
                "1"
            ]),
            "BTC-USDT-SWAP",
            "1m",
        );
        let zero_close = normalize_candle(
            json!(["1700000000000", "100", "101", "99", "0", "1", "1", "1", "1"]),
            "BTC-USDT-SWAP",
            "1m",
        );

        assert!(non_finite.is_none());
        assert!(zero_close.is_none());
    }

    #[tokio::test]
    async fn persist_confirmed_candle_rejects_missing_volume_instead_of_writing_zero() {
        let pool = test_pool().await;
        let candle = json!({
            "inst_id": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "1m",
            "timestamp": 1700000000000i64,
            "open": 100.0,
            "high": 101.0,
            "low": 99.0,
            "close": 100.5,
            "volume_ccy": 12.0,
            "confirm": "1"
        });

        let error = persist_confirmed_candle(&pool, &candle)
            .await
            .expect_err("confirmed realtime candle without volume must not be persisted");

        assert!(error.to_string().contains("volume"));
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM candles")
            .fetch_one(&pool)
            .await
            .expect("count candles");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn persist_confirmed_candle_rejects_non_positive_timestamp() {
        let pool = test_pool().await;
        let candle = json!({
            "inst_id": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "1m",
            "timestamp": 0,
            "open": 100.0,
            "high": 101.0,
            "low": 99.0,
            "close": 100.5,
            "volume": 7.0,
            "volume_ccy": 12.0,
            "volume_quote": 12.5,
            "confirm": "1"
        });

        let error = persist_confirmed_candle(&pool, &candle)
            .await
            .expect_err("confirmed realtime candle with non-positive timestamp must not persist");

        assert!(error.to_string().contains("timestamp"));
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM candles")
            .fetch_one(&pool)
            .await
            .expect("count candles");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn persist_confirmed_candle_writes_complete_market_values() {
        let pool = test_pool().await;
        let candle = json!({
            "inst_id": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "1m",
            "timestamp": 1700000000000i64,
            "open": 100.0,
            "high": 101.0,
            "low": 99.0,
            "close": 100.5,
            "volume": 7.0,
            "volume_ccy": 12.0,
            "volume_quote": 12.5,
            "confirm": "1"
        });

        persist_confirmed_candle(&pool, &candle)
            .await
            .expect("complete confirmed candle should persist");

        let row =
            sqlx::query("SELECT volume, volume_ccy, volume_quote FROM candles WHERE inst_id = ?")
                .bind("BTC-USDT-SWAP")
                .fetch_one(&pool)
                .await
                .expect("persisted candle row");
        assert_eq!(row.get::<f64, _>("volume"), 7.0);
        assert_eq!(row.get::<f64, _>("volume_ccy"), 12.0);
        assert_eq!(row.get::<f64, _>("volume_quote"), 12.5);
        let sync_count: i64 = sqlx::query_scalar("SELECT candle_count FROM sync_records")
            .fetch_one(&pool)
            .await
            .expect("sync record");
        assert_eq!(sync_count, 1);
    }

    #[tokio::test]
    async fn persist_confirmed_candle_updates_existing_timestamp_without_incrementing_count() {
        let pool = test_pool().await;
        let first = json!({
            "inst_id": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "1m",
            "timestamp": 1700000000000i64,
            "open": 100.0,
            "high": 101.0,
            "low": 99.0,
            "close": 100.5,
            "volume": 7.0,
            "volume_ccy": 12.0,
            "volume_quote": 12.5,
            "confirm": "1"
        });
        let updated = json!({
            "inst_id": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "1m",
            "timestamp": 1700000000000i64,
            "open": 101.0,
            "high": 102.0,
            "low": 100.0,
            "close": 101.5,
            "volume": 8.0,
            "volume_ccy": 13.0,
            "volume_quote": 13.5,
            "confirm": "1"
        });

        persist_confirmed_candle(&pool, &first)
            .await
            .expect("first candle should persist");
        persist_confirmed_candle(&pool, &updated)
            .await
            .expect("updated candle should persist");

        let candle_row = sqlx::query(
            "SELECT close, volume, volume_ccy, volume_quote FROM candles WHERE inst_id = ? AND timestamp = ?",
        )
        .bind("BTC-USDT-SWAP")
        .bind(1700000000000i64)
        .fetch_one(&pool)
        .await
        .expect("updated candle row");
        assert_eq!(candle_row.get::<f64, _>("close"), 101.5);
        assert_eq!(candle_row.get::<f64, _>("volume"), 8.0);
        assert_eq!(candle_row.get::<f64, _>("volume_ccy"), 13.0);
        assert_eq!(candle_row.get::<f64, _>("volume_quote"), 13.5);

        let sync_row = sqlx::query(
            r#"
            SELECT oldest_timestamp, newest_timestamp, candle_count
            FROM sync_records
            WHERE inst_id = 'BTC-USDT-SWAP'
              AND inst_type = 'SWAP'
              AND timeframe = '1m'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("sync record row");
        assert_eq!(
            sync_row.try_get::<i64, _>("oldest_timestamp").unwrap(),
            1700000000000i64
        );
        assert_eq!(
            sync_row.try_get::<i64, _>("newest_timestamp").unwrap(),
            1700000000000i64
        );
        assert_eq!(sync_row.try_get::<i64, _>("candle_count").unwrap(), 1);
    }

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
        sqlx::query(
            "CREATE INDEX idx_candles_query ON candles(inst_id, inst_type, timeframe, timestamp)",
        )
        .execute(&pool)
        .await
        .expect("create candles index");
        sqlx::query(
            r#"
            CREATE TABLE sync_records (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              inst_id TEXT NOT NULL,
              inst_type TEXT NOT NULL DEFAULT 'SPOT',
              timeframe TEXT NOT NULL,
              last_sync_time TIMESTAMP,
              oldest_timestamp INTEGER,
              newest_timestamp INTEGER,
              candle_count INTEGER DEFAULT 0,
              history_complete INTEGER NOT NULL DEFAULT 0,
              last_sync_mode TEXT NOT NULL DEFAULT 'window',
              UNIQUE(inst_id, inst_type, timeframe)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create sync_records table");
        pool
    }
}
