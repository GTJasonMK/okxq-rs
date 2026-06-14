use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::{market_candle_rows::count_valid_candle_rows_at_timestamps, okx::OkxCandle};

use super::super::super::{
    candle_upsert_transaction_chunk, check_sync_cancel, AppResult, SyncCancelGuard,
    SyncProgressReporter, SyncProgressUpdate, SyncRecordStats,
};
use super::super::{get_sync_record_stats, update_sync_record};
use super::progress::BatchSaveProgress;

struct SyncRecordRefreshBatch {
    timestamps: Vec<i64>,
    existing_valid_count: i64,
}

impl SyncRecordRefreshBatch {
    fn oldest_timestamp(&self) -> i64 {
        self.timestamps[0]
    }

    fn newest_timestamp(&self) -> i64 {
        self.timestamps[self.timestamps.len() - 1]
    }

    fn inserted_valid_count(&self) -> i64 {
        (self.timestamps.len() as i64).saturating_sub(self.existing_valid_count)
    }
}

fn valid_market_candle_refs(candles: &[OkxCandle]) -> Vec<&OkxCandle> {
    candles
        .iter()
        .filter(|candle| candle.is_valid_market_candle())
        .collect()
}

pub(in crate::commands::local_api::market_ops) struct CandleSaveScope<'a> {
    pub(in crate::commands::local_api::market_ops) pool: &'a SqlitePool,
    pub(in crate::commands::local_api::market_ops) inst_id: &'a str,
    pub(in crate::commands::local_api::market_ops) inst_type: &'a str,
    pub(in crate::commands::local_api::market_ops) timeframe: &'a str,
    pub(in crate::commands::local_api::market_ops) cancel_guard: Option<&'a SyncCancelGuard>,
    pub(in crate::commands::local_api::market_ops) progress: &'a SyncProgressReporter,
}

#[derive(Clone, Copy)]
pub(in crate::commands::local_api::market_ops) struct CandleSaveProgress {
    pub(in crate::commands::local_api::market_ops) fetched_count: i64,
    pub(in crate::commands::local_api::market_ops) target_fetch_count: i64,
    pub(in crate::commands::local_api::market_ops) target_save_count: i64,
    pub(in crate::commands::local_api::market_ops) batches: i64,
    pub(in crate::commands::local_api::market_ops) target_batches: i64,
    pub(in crate::commands::local_api::market_ops) api_calls: i64,
    pub(in crate::commands::local_api::market_ops) saved_offset: i64,
    pub(in crate::commands::local_api::market_ops) derived_offset: i64,
    pub(in crate::commands::local_api::market_ops) target_derive_count: i64,
    pub(in crate::commands::local_api::market_ops) derived: bool,
}

impl CandleSaveProgress {
    fn batch_progress<'a>(self, progress: &'a SyncProgressReporter) -> BatchSaveProgress<'a> {
        BatchSaveProgress {
            progress,
            fetched_count: self.fetched_count,
            target_fetch_count: self.target_fetch_count,
            target_save_count: self.target_save_count,
            batches: self.batches,
            target_batches: self.target_batches,
            api_calls: self.api_calls,
            saved_offset: self.saved_offset,
            derived_offset: self.derived_offset,
            target_derive_count: self.target_derive_count,
            derived: self.derived,
            base_progress: if self.derived { 88 } else { 70 },
            span: if self.derived { 8 } else { 18 },
        }
    }

    fn base_batch_progress<'a>(
        self,
        progress: &'a SyncProgressReporter,
        fetch_progress: i64,
    ) -> BatchSaveProgress<'a> {
        BatchSaveProgress {
            progress,
            fetched_count: self.fetched_count,
            target_fetch_count: self.target_fetch_count,
            target_save_count: self.target_save_count,
            batches: self.batches,
            target_batches: self.target_batches,
            api_calls: self.api_calls,
            saved_offset: self.saved_offset,
            derived_offset: 0,
            target_derive_count: 0,
            derived: false,
            base_progress: fetch_progress.max(20),
            span: 8,
        }
    }

    fn base_batch_update(
        self,
        progress: i64,
        message: String,
        saved_count: i64,
    ) -> SyncProgressUpdate {
        SyncProgressUpdate {
            progress,
            message,
            fetched_count: self.fetched_count,
            target_fetch_count: self.target_fetch_count,
            saved_count,
            target_save_count: self.target_save_count,
            inserted_count: saved_count,
            batches: self.batches,
            target_batches: self.target_batches,
            api_calls: self.api_calls,
            ..Default::default()
        }
    }
}

pub(super) struct BaseCandleBatchSave {
    pub(super) pool: SqlitePool,
    pub(super) inst_id: String,
    pub(super) inst_type: String,
    pub(super) timeframe: String,
    pub(super) candles: Vec<OkxCandle>,
    pub(super) cancel_guard: Option<SyncCancelGuard>,
    pub(super) progress: SyncProgressReporter,
    pub(super) progress_counts: CandleSaveProgress,
    pub(super) fetch_message: String,
    pub(super) fetch_progress: i64,
}

pub(super) async fn save_base_candle_batch(batch: BaseCandleBatchSave) -> AppResult<i64> {
    let valid_candles = valid_market_candle_refs(&batch.candles);
    let saved = upsert_candles(
        &batch.pool,
        &batch.inst_id,
        &batch.inst_type,
        &batch.timeframe,
        &valid_candles,
        batch.cancel_guard.as_ref(),
        Some(
            batch
                .progress_counts
                .base_batch_progress(&batch.progress, batch.fetch_progress),
        ),
    )
    .await?;
    tracing::debug!(
        inst_id = %batch.inst_id,
        inst_type = %batch.inst_type,
        timeframe = %batch.timeframe,
        fetched_count = batch.progress_counts.fetched_count,
        saved_count = batch.progress_counts.saved_offset + saved,
        target_fetch_count = batch.progress_counts.target_fetch_count,
        target_save_count = batch.progress_counts.target_save_count,
        batches = batch.progress_counts.batches,
        api_calls = batch.progress_counts.api_calls,
        "base candle batch saved"
    );
    let saved_count = batch.progress_counts.saved_offset + saved;
    let message = if batch.fetch_message.starts_with("全量回补 ") {
        format!("{}；已落库 {} 条", batch.fetch_message, saved_count)
    } else {
        format!(
            "{}；已落库 {}/{} 条",
            batch.fetch_message, saved_count, batch.progress_counts.target_save_count
        )
    };
    batch
        .progress
        .report(batch.progress_counts.base_batch_update(
            batch.fetch_progress.clamp(20, 68),
            message,
            saved_count,
        ))
        .await?;
    Ok(saved)
}

async fn upsert_candles(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    candles: &[&OkxCandle],
    cancel_guard: Option<&SyncCancelGuard>,
    progress: Option<BatchSaveProgress<'_>>,
) -> AppResult<i64> {
    let mut saved_count = 0i64;
    let mut processed_count = 0i64;
    for chunk in candles.chunks(candle_upsert_transaction_chunk()) {
        check_sync_cancel(cancel_guard).await?;
        let mut tx = pool.begin().await?;
        let result = upsert_candle_chunk(&mut tx, inst_id, inst_type, timeframe, chunk).await?;
        saved_count += result.rows_affected() as i64;
        tx.commit().await?;
        processed_count += chunk.len() as i64;

        if let Some(progress) = progress.as_ref() {
            progress
                .report_chunk_saved(timeframe, saved_count, processed_count, candles.len())
                .await?;
        }
    }
    Ok(saved_count)
}

async fn upsert_candle_chunk(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    candles: &[&OkxCandle],
) -> AppResult<sqlx::sqlite::SqliteQueryResult> {
    let mut query = QueryBuilder::<Sqlite>::new(
        r#"
        INSERT INTO candles (
          inst_id, inst_type, timeframe, timestamp,
          open, high, low, close, volume, volume_ccy, volume_quote
        )
        "#,
    );
    query.push_values(candles.iter(), |mut row, candle| {
        let candle = *candle;
        row.push_bind(inst_id)
            .push_bind(inst_type)
            .push_bind(timeframe)
            .push_bind(candle.timestamp)
            .push_bind(candle.open)
            .push_bind(candle.high)
            .push_bind(candle.low)
            .push_bind(candle.close)
            .push_bind(candle.volume)
            .push_bind(candle.volume_ccy)
            .push_bind(candle.volume_quote);
    });
    query.push(
        r#"
        ON CONFLICT(inst_id, inst_type, timeframe, timestamp) DO UPDATE SET
          open = excluded.open,
          high = excluded.high,
          low = excluded.low,
          close = excluded.close,
          volume = excluded.volume,
          volume_ccy = excluded.volume_ccy,
          volume_quote = excluded.volume_quote
        WHERE candles.open IS NOT excluded.open
           OR candles.high IS NOT excluded.high
           OR candles.low IS NOT excluded.low
           OR candles.close IS NOT excluded.close
           OR candles.volume IS NOT excluded.volume
           OR candles.volume_ccy IS NOT excluded.volume_ccy
           OR candles.volume_quote IS NOT excluded.volume_quote
        "#,
    );
    Ok(query.build().execute(&mut **tx).await?)
}

pub(in crate::commands::local_api::market_ops) async fn save_candles(
    scope: CandleSaveScope<'_>,
    candles: &[OkxCandle],
    progress_counts: CandleSaveProgress,
) -> AppResult<i64> {
    let valid_candles = valid_market_candle_refs(candles);
    let refresh_batch = sync_record_refresh_batch(
        scope.pool,
        scope.inst_id,
        scope.inst_type,
        scope.timeframe,
        &valid_candles,
    )
    .await?;
    let saved = upsert_candles(
        scope.pool,
        scope.inst_id,
        scope.inst_type,
        scope.timeframe,
        &valid_candles,
        scope.cancel_guard,
        Some(progress_counts.batch_progress(scope.progress)),
    )
    .await?;
    refresh_sync_record_cache(
        scope.pool,
        scope.inst_id,
        scope.inst_type,
        scope.timeframe,
        refresh_batch.as_ref(),
    )
    .await?;
    Ok(saved)
}

async fn refresh_sync_record_cache(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    batch: Option<&SyncRecordRefreshBatch>,
) -> AppResult<()> {
    let Some(batch) = batch else {
        return Ok(());
    };
    if let Some(existing) = get_sync_record_stats(pool, inst_id, inst_type, timeframe).await? {
        if can_increment_sync_record(&existing, batch) {
            increment_sync_record(pool, inst_id, inst_type, timeframe, batch).await?;
            return Ok(());
        }
    }
    update_sync_record(pool, inst_id, inst_type, timeframe, None, None).await?;
    Ok(())
}

async fn sync_record_refresh_batch(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    candles: &[&OkxCandle],
) -> AppResult<Option<SyncRecordRefreshBatch>> {
    let mut timestamps = candles
        .iter()
        .map(|candle| candle.timestamp)
        .collect::<Vec<_>>();
    timestamps.sort_unstable();
    timestamps.dedup();
    if timestamps.is_empty() {
        return Ok(None);
    }
    let existing_valid_count =
        count_valid_candle_rows_at_timestamps(pool, inst_id, inst_type, timeframe, &timestamps)
            .await?;
    Ok(Some(SyncRecordRefreshBatch {
        timestamps,
        existing_valid_count,
    }))
}

fn can_increment_sync_record(existing: &SyncRecordStats, batch: &SyncRecordRefreshBatch) -> bool {
    if existing.candle_count <= 0 {
        return false;
    }
    let (Some(oldest), Some(newest)) = (existing.oldest_timestamp, existing.newest_timestamp)
    else {
        return false;
    };
    let outside_existing_range = batch
        .timestamps
        .iter()
        .filter(|timestamp| **timestamp < oldest || **timestamp > newest)
        .count() as i64;
    batch.inserted_valid_count() >= outside_existing_range
}

async fn increment_sync_record(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    batch: &SyncRecordRefreshBatch,
) -> AppResult<()> {
    let result = sqlx::query(
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
          candle_count = COALESCE(candle_count, 0) + ?
        WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
        "#,
    )
    .bind(batch.oldest_timestamp())
    .bind(batch.oldest_timestamp())
    .bind(batch.newest_timestamp())
    .bind(batch.newest_timestamp())
    .bind(batch.inserted_valid_count())
    .bind(inst_id)
    .bind(inst_type)
    .bind(timeframe)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        update_sync_record(pool, inst_id, inst_type, timeframe, None, None).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, Row};

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

    fn candle(timestamp: i64) -> OkxCandle {
        OkxCandle {
            timestamp,
            open: 1.0,
            high: 1.0,
            low: 1.0,
            close: 1.0,
            volume: 1.0,
            volume_ccy: 1.0,
            volume_quote: 1.0,
            confirm: "1".to_string(),
        }
    }

    fn invalid_price_candle(timestamp: i64) -> OkxCandle {
        OkxCandle {
            open: 0.0,
            ..candle(timestamp)
        }
    }

    fn test_save_progress(count: i64) -> CandleSaveProgress {
        CandleSaveProgress {
            fetched_count: count,
            target_fetch_count: count,
            target_save_count: count,
            batches: 1,
            target_batches: 1,
            api_calls: 1,
            saved_offset: 0,
            derived_offset: 0,
            target_derive_count: 0,
            derived: false,
        }
    }

    async fn save_test_candles(
        pool: &SqlitePool,
        candles: &[OkxCandle],
        progress_counts: CandleSaveProgress,
    ) -> i64 {
        let progress = SyncProgressReporter::default();
        save_candles(
            CandleSaveScope {
                pool,
                inst_id: "BTC-USDT-SWAP",
                inst_type: "SWAP",
                timeframe: "1m",
                cancel_guard: None,
                progress: &progress,
            },
            candles,
            progress_counts,
        )
        .await
        .expect("save candles")
    }

    async fn save_test_base_batch(pool: SqlitePool, candles: Vec<OkxCandle>) -> i64 {
        save_base_candle_batch(BaseCandleBatchSave {
            pool,
            inst_id: "BTC-USDT-SWAP".to_string(),
            inst_type: "SWAP".to_string(),
            timeframe: "1m".to_string(),
            candles,
            cancel_guard: None,
            progress: SyncProgressReporter::default(),
            progress_counts: test_save_progress(2),
            fetch_message: "test".to_string(),
            fetch_progress: 50,
        })
        .await
        .expect("save base candle batch")
    }

    #[tokio::test]
    async fn save_candles_refreshes_sync_record_cache() {
        let pool = test_pool().await;
        let candles = vec![candle(60_000), candle(120_000)];

        let saved = save_test_candles(&pool, &candles, test_save_progress(2)).await;

        assert_eq!(saved, 2);
        let row = sqlx::query(
            r#"
            SELECT oldest_timestamp, newest_timestamp, candle_count,
                   history_complete, last_sync_mode
            FROM sync_records
            WHERE inst_id = 'BTC-USDT-SWAP'
              AND inst_type = 'SWAP'
              AND timeframe = '1m'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("sync record row");

        assert_eq!(row.try_get::<i64, _>("oldest_timestamp").unwrap(), 60_000);
        assert_eq!(row.try_get::<i64, _>("newest_timestamp").unwrap(), 120_000);
        assert_eq!(row.try_get::<i64, _>("candle_count").unwrap(), 2);
        assert_eq!(row.try_get::<i64, _>("history_complete").unwrap(), 0);
        assert_eq!(
            row.try_get::<String, _>("last_sync_mode").unwrap(),
            "window"
        );
    }

    #[tokio::test]
    async fn save_candles_skips_invalid_market_rows_before_persisting() {
        let pool = test_pool().await;
        let candles = vec![invalid_price_candle(60_000), candle(120_000)];

        let saved = save_test_candles(&pool, &candles, test_save_progress(2)).await;

        assert_eq!(saved, 1);
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) AS candle_count,
                   MIN(timestamp) AS oldest_timestamp,
                   MAX(timestamp) AS newest_timestamp
            FROM candles
            WHERE inst_id = 'BTC-USDT-SWAP'
              AND inst_type = 'SWAP'
              AND timeframe = '1m'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("candle stats");

        assert_eq!(row.try_get::<i64, _>("candle_count").unwrap(), 1);
        assert_eq!(row.try_get::<i64, _>("oldest_timestamp").unwrap(), 120_000);
        assert_eq!(row.try_get::<i64, _>("newest_timestamp").unwrap(), 120_000);
    }

    #[tokio::test]
    async fn save_candles_persists_okx_quote_volume() {
        let pool = test_pool().await;
        let candles = vec![OkxCandle {
            volume: 12.0,
            volume_ccy: 1_224.0,
            volume_quote: 1_225.0,
            ..candle(60_000)
        }];

        let saved = save_test_candles(&pool, &candles, test_save_progress(1)).await;

        assert_eq!(saved, 1);
        let row = sqlx::query(
            r#"
            SELECT volume, volume_ccy, volume_quote
            FROM candles
            WHERE inst_id = 'BTC-USDT-SWAP'
              AND inst_type = 'SWAP'
              AND timeframe = '1m'
              AND timestamp = 60000
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("saved candle");

        assert_eq!(row.try_get::<f64, _>("volume").unwrap(), 12.0);
        assert_eq!(row.try_get::<f64, _>("volume_ccy").unwrap(), 1_224.0);
        assert_eq!(
            row.try_get::<Option<f64>, _>("volume_quote").unwrap(),
            Some(1_225.0)
        );
    }

    #[tokio::test]
    async fn save_candles_counts_only_inserted_or_changed_rows() {
        let pool = test_pool().await;
        let candles = vec![candle(60_000), candle(120_000)];

        let inserted = save_test_candles(&pool, &candles, test_save_progress(2)).await;
        assert_eq!(inserted, 2);

        let unchanged = save_test_candles(
            &pool,
            &candles,
            CandleSaveProgress {
                saved_offset: inserted,
                ..test_save_progress(2)
            },
        )
        .await;
        assert_eq!(unchanged, 0);

        let changed = vec![OkxCandle {
            close: 1.5,
            ..candle(120_000)
        }];
        let updated = save_test_candles(
            &pool,
            &changed,
            CandleSaveProgress {
                saved_offset: inserted,
                ..test_save_progress(1)
            },
        )
        .await;
        assert_eq!(updated, 1);
    }

    #[tokio::test]
    async fn save_candles_incrementally_extends_existing_sync_record_cache() {
        let pool = test_pool().await;
        let candles = vec![candle(60_000), candle(120_000)];
        save_test_candles(&pool, &candles, test_save_progress(2)).await;
        sqlx::query(
            r#"
            UPDATE sync_records
            SET history_complete = 1,
                last_sync_mode = 'full'
            WHERE inst_id = 'BTC-USDT-SWAP'
              AND inst_type = 'SWAP'
              AND timeframe = '1m'
            "#,
        )
        .execute(&pool)
        .await
        .expect("mark sync record fields");

        let changed_and_appended = vec![
            OkxCandle {
                close: 1.5,
                ..candle(120_000)
            },
            candle(180_000),
        ];
        let saved = save_test_candles(
            &pool,
            &changed_and_appended,
            CandleSaveProgress {
                saved_offset: 2,
                ..test_save_progress(2)
            },
        )
        .await;
        assert_eq!(saved, 2);

        let row = sqlx::query(
            r#"
            SELECT oldest_timestamp, newest_timestamp, candle_count,
                   history_complete, last_sync_mode
            FROM sync_records
            WHERE inst_id = 'BTC-USDT-SWAP'
              AND inst_type = 'SWAP'
              AND timeframe = '1m'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("sync record row");

        assert_eq!(row.try_get::<i64, _>("oldest_timestamp").unwrap(), 60_000);
        assert_eq!(row.try_get::<i64, _>("newest_timestamp").unwrap(), 180_000);
        assert_eq!(row.try_get::<i64, _>("candle_count").unwrap(), 3);
        assert_eq!(row.try_get::<i64, _>("history_complete").unwrap(), 1);
        assert_eq!(row.try_get::<String, _>("last_sync_mode").unwrap(), "full");
    }

    #[tokio::test]
    async fn save_base_candle_batch_defers_sync_record_refresh() {
        let pool = test_pool().await;
        let candles = vec![candle(60_000), candle(120_000)];

        let saved = save_test_base_batch(pool.clone(), candles).await;
        assert_eq!(saved, 2);

        let sync_record = sqlx::query(
            r#"
            SELECT id
            FROM sync_records
            WHERE inst_id = 'BTC-USDT-SWAP'
              AND inst_type = 'SWAP'
              AND timeframe = '1m'
            "#,
        )
        .fetch_optional(&pool)
        .await
        .expect("sync record query");
        assert!(sync_record.is_none());

        let refreshed = update_sync_record(&pool, "BTC-USDT-SWAP", "SWAP", "1m", None, None)
            .await
            .expect("final sync record refresh");
        assert_eq!(refreshed.oldest_timestamp, Some(60_000));
        assert_eq!(refreshed.newest_timestamp, Some(120_000));
        assert_eq!(refreshed.candle_count, 2);
    }

    #[tokio::test]
    async fn save_candles_does_not_refresh_sync_record_for_all_invalid_batch() {
        let pool = test_pool().await;
        let candles = vec![invalid_price_candle(60_000)];

        let saved = save_test_candles(&pool, &candles, test_save_progress(1)).await;

        assert_eq!(saved, 0);
        let candle_count = sqlx::query(
            r#"
            SELECT COUNT(*) AS candle_count
            FROM candles
            WHERE inst_id = 'BTC-USDT-SWAP'
              AND inst_type = 'SWAP'
              AND timeframe = '1m'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("candle count")
        .try_get::<i64, _>("candle_count")
        .unwrap();
        assert_eq!(candle_count, 0);

        let sync_record = sqlx::query(
            r#"
            SELECT id
            FROM sync_records
            WHERE inst_id = 'BTC-USDT-SWAP'
              AND inst_type = 'SWAP'
              AND timeframe = '1m'
            "#,
        )
        .fetch_optional(&pool)
        .await
        .expect("sync record query");
        assert!(sync_record.is_none());
    }
}
