use sqlx::{Row, SqlitePool};

use crate::market_candle_rows::{load_valid_candle_boundary_timestamp, load_valid_candle_stats};

use super::super::*;

pub(in crate::commands::local_api::market_ops) async fn update_sync_record(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    history_complete: Option<bool>,
    last_sync_mode: Option<&str>,
) -> AppResult<SyncRecordStats> {
    let existing = get_sync_record_stats(pool, inst_id, inst_type, timeframe).await?;
    let history_complete_value = history_complete
        .or_else(|| existing.as_ref().map(|item| item.history_complete))
        .unwrap_or(false);
    let last_sync_mode_value = last_sync_mode
        .map(ToOwned::to_owned)
        .or_else(|| existing.as_ref().map(|item| item.last_sync_mode.clone()))
        .unwrap_or_else(|| "window".to_string());

    let (oldest_timestamp, newest_timestamp, candle_count) =
        candle_stats(pool, inst_id, inst_type, timeframe).await?;

    sqlx::query(
        r#"
        INSERT INTO sync_records (
          inst_id, inst_type, timeframe, last_sync_time,
          oldest_timestamp, newest_timestamp, candle_count,
          history_complete, last_sync_mode
        ) VALUES (?, ?, ?, CURRENT_TIMESTAMP, ?, ?, ?, ?, ?)
        ON CONFLICT(inst_id, inst_type, timeframe) DO UPDATE SET
          last_sync_time = CURRENT_TIMESTAMP,
          oldest_timestamp = excluded.oldest_timestamp,
          newest_timestamp = excluded.newest_timestamp,
          candle_count = excluded.candle_count,
          history_complete = excluded.history_complete,
          last_sync_mode = excluded.last_sync_mode
        "#,
    )
    .bind(inst_id)
    .bind(inst_type)
    .bind(timeframe)
    .bind(oldest_timestamp)
    .bind(newest_timestamp)
    .bind(candle_count)
    .bind(if history_complete_value { 1 } else { 0 })
    .bind(last_sync_mode_value)
    .execute(pool)
    .await?;

    Ok(get_sync_record_stats(pool, inst_id, inst_type, timeframe)
        .await?
        .unwrap_or(SyncRecordStats {
            last_sync_time: None,
            oldest_timestamp,
            newest_timestamp,
            candle_count,
            history_complete: history_complete_value,
            last_sync_mode: last_sync_mode.unwrap_or("window").to_string(),
        }))
}

pub(in crate::commands::local_api::market_ops) async fn get_sync_record_stats(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
) -> AppResult<Option<SyncRecordStats>> {
    let row = sqlx::query(
        r#"
        SELECT last_sync_time, oldest_timestamp, newest_timestamp, candle_count,
               history_complete, last_sync_mode
        FROM sync_records
        WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
        "#,
    )
    .bind(inst_id)
    .bind(inst_type)
    .bind(timeframe)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    Ok(Some(SyncRecordStats {
        last_sync_time: row.try_get::<Option<String>, _>("last_sync_time")?,
        oldest_timestamp: row.try_get::<Option<i64>, _>("oldest_timestamp")?,
        newest_timestamp: row.try_get::<Option<i64>, _>("newest_timestamp")?,
        candle_count: row.try_get::<i64, _>("candle_count")?,
        history_complete: row.try_get::<i64, _>("history_complete")? != 0,
        last_sync_mode: row.try_get::<String, _>("last_sync_mode")?,
    }))
}

pub(in crate::commands::local_api::market_ops) async fn update_sync_record_metadata(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    history_complete: Option<bool>,
    last_sync_mode: Option<&str>,
) -> AppResult<SyncRecordStats> {
    let Some(existing) = get_sync_record_stats(pool, inst_id, inst_type, timeframe).await? else {
        return update_sync_record(
            pool,
            inst_id,
            inst_type,
            timeframe,
            history_complete,
            last_sync_mode,
        )
        .await;
    };
    let history_complete_value = history_complete.unwrap_or(existing.history_complete);
    let last_sync_mode_value = last_sync_mode
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| existing.last_sync_mode.clone());

    let result = sqlx::query(
        r#"
        UPDATE sync_records
        SET last_sync_time = CURRENT_TIMESTAMP,
            history_complete = ?,
            last_sync_mode = ?
        WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
        "#,
    )
    .bind(if history_complete_value { 1 } else { 0 })
    .bind(last_sync_mode_value)
    .bind(inst_id)
    .bind(inst_type)
    .bind(timeframe)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return update_sync_record(
            pool,
            inst_id,
            inst_type,
            timeframe,
            history_complete,
            last_sync_mode,
        )
        .await;
    }

    Ok(get_sync_record_stats(pool, inst_id, inst_type, timeframe)
        .await?
        .unwrap_or(SyncRecordStats {
            last_sync_time: existing.last_sync_time,
            oldest_timestamp: existing.oldest_timestamp,
            newest_timestamp: existing.newest_timestamp,
            candle_count: existing.candle_count,
            history_complete: history_complete_value,
            last_sync_mode: last_sync_mode
                .unwrap_or(&existing.last_sync_mode)
                .to_string(),
        }))
}

pub(in crate::commands::local_api::market_ops) async fn local_candle_bounds(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
) -> AppResult<Option<(i64, i64)>> {
    let Some(oldest) = local_candle_boundary(pool, inst_id, inst_type, timeframe, false).await?
    else {
        return Ok(None);
    };
    let newest = local_candle_boundary(pool, inst_id, inst_type, timeframe, true)
        .await?
        .unwrap_or(oldest);
    Ok(Some((oldest, newest)))
}

async fn local_candle_boundary(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    descending: bool,
) -> AppResult<Option<i64>> {
    load_valid_candle_boundary_timestamp(pool, inst_id, inst_type, timeframe, descending).await
}

pub(in crate::commands::local_api::market_ops) async fn candle_stats(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
) -> AppResult<(Option<i64>, Option<i64>, i64)> {
    let stats = load_valid_candle_stats(pool, inst_id, inst_type, timeframe).await?;
    Ok((
        stats.oldest_timestamp,
        stats.newest_timestamp,
        stats.candle_count,
    ))
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, QueryBuilder, Sqlite};

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

    async fn insert_candle(pool: &SqlitePool, timestamp: i64, open: f64) {
        sqlx::query(
            r#"
            INSERT INTO candles (
              inst_id, inst_type, timeframe, timestamp,
              open, high, low, close, volume, volume_ccy
            ) VALUES ('BTC-USDT-SWAP', 'SWAP', '1m', ?, ?, 101, 99, 100, 1, 1)
            "#,
        )
        .bind(timestamp)
        .bind(open)
        .execute(pool)
        .await
        .expect("insert candle");
    }

    #[tokio::test]
    async fn candle_stats_ignores_invalid_market_rows() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, 0.0).await;
        insert_candle(&pool, 120_000, 100.0).await;

        let (oldest, newest, count) = candle_stats(&pool, "BTC-USDT-SWAP", "SWAP", "1m")
            .await
            .expect("candle stats");

        assert_eq!(oldest, Some(120_000));
        assert_eq!(newest, Some(120_000));
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn candle_stats_reports_empty_when_only_invalid_rows_exist() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, 0.0).await;

        let (oldest, newest, count) = candle_stats(&pool, "BTC-USDT-SWAP", "SWAP", "1m")
            .await
            .expect("candle stats");

        assert_eq!(oldest, None);
        assert_eq!(newest, None);
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn local_candle_bounds_matches_stats_for_valid_market_rows() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, 0.0).await;
        insert_candle(&pool, 120_000, 100.0).await;
        insert_candle(&pool, 180_000, 100.0).await;
        insert_candle(&pool, 240_000, 0.0).await;

        let bounds = local_candle_bounds(&pool, "BTC-USDT-SWAP", "SWAP", "1m")
            .await
            .expect("local candle bounds");
        let (oldest, newest, count) = candle_stats(&pool, "BTC-USDT-SWAP", "SWAP", "1m")
            .await
            .expect("candle stats");

        assert_eq!(bounds, Some((oldest.unwrap(), newest.unwrap())));
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn local_candle_bounds_reports_empty_when_only_invalid_rows_exist() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, 0.0).await;

        let bounds = local_candle_bounds(&pool, "BTC-USDT-SWAP", "SWAP", "1m")
            .await
            .expect("local candle bounds");

        assert_eq!(bounds, None);
    }

    #[tokio::test]
    async fn update_sync_record_metadata_preserves_cached_range_and_count() {
        let pool = test_pool().await;
        insert_valid_candle_range(&pool, 3).await;
        let refreshed = update_sync_record(
            &pool,
            "BTC-USDT-SWAP",
            "SWAP",
            "1m",
            Some(false),
            Some("window"),
        )
        .await
        .expect("refresh sync record");
        assert_eq!(refreshed.candle_count, 3);

        let updated = update_sync_record_metadata(
            &pool,
            "BTC-USDT-SWAP",
            "SWAP",
            "1m",
            Some(true),
            Some("full"),
        )
        .await
        .expect("update sync record metadata");

        assert_eq!(updated.oldest_timestamp, Some(60_000));
        assert_eq!(updated.newest_timestamp, Some(180_000));
        assert_eq!(updated.candle_count, 3);
        assert!(updated.history_complete);
        assert_eq!(updated.last_sync_mode, "full");
    }

    async fn insert_valid_candle_range(pool: &SqlitePool, rows: usize) {
        for chunk_start in (0..rows).step_by(500) {
            let chunk_end = (chunk_start + 500).min(rows);
            let mut query = QueryBuilder::<Sqlite>::new(
                r#"
                INSERT INTO candles (
                  inst_id, inst_type, timeframe, timestamp,
                  open, high, low, close, volume, volume_ccy
                )
                "#,
            );
            query.push_values(chunk_start..chunk_end, |mut row, index| {
                row.push_bind("BTC-USDT-SWAP")
                    .push_bind("SWAP")
                    .push_bind("1m")
                    .push_bind((index as i64 + 1) * 60_000)
                    .push_bind(1.0_f64)
                    .push_bind(1.0_f64)
                    .push_bind(1.0_f64)
                    .push_bind(1.0_f64)
                    .push_bind(1.0_f64)
                    .push_bind(1.0_f64);
            });
            query
                .build()
                .execute(pool)
                .await
                .expect("insert valid candle range");
        }
    }
}
