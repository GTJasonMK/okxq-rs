use sqlx::SqlitePool;

use crate::okx::OkxCandle;

use super::{
    derivation::{aggregate_candles_from_source, source_candles_required_for_target},
    fetch::newer_gap_exceeds_target_window,
    persistence::{load_local_strategy_candles, save_strategy_candles},
    storage_kind::{resolve_timeframe_storage_kind_from_db, TimeframeStorageKind},
};

#[tokio::test]
async fn local_strategy_candles_backfill_valid_rows_after_invalid_recent_candle() {
    let pool = SqlitePool::connect(":memory:")
        .await
        .expect("in-memory db should connect");
    create_candles_table(&pool).await;

    insert_candle(&pool, 1_000, "100").await;
    insert_candle(&pool, 2_000, "101").await;
    insert_candle(&pool, 3_000, "102").await;
    insert_candle(&pool, 4_000, "bad-open").await;

    let candles = load_local_strategy_candles(&pool, "BTC-USDT-SWAP", "SWAP", "15m", 3)
        .await
        .expect("local candle query should not fail");

    assert_eq!(
        candles
            .iter()
            .map(|item| item.timestamp)
            .collect::<Vec<_>>(),
        vec![1_000, 2_000, 3_000],
        "invalid recent rows must not reduce the requested count when older valid candles exist"
    );
}

#[tokio::test]
async fn local_strategy_candles_match_canonical_okx_timeframe() {
    let pool = SqlitePool::connect(":memory:")
        .await
        .expect("in-memory db should connect");
    create_candles_table(&pool).await;

    insert_candle_with_timeframe(&pool, "1H", 60_000, "100").await;
    insert_candle_with_timeframe(&pool, "1H", 120_000, "101").await;
    insert_candle_with_timeframe(&pool, "1H", 180_000, "102").await;

    let candles = load_local_strategy_candles(&pool, "BTC-USDT-SWAP", "SWAP", "1h", 3)
        .await
        .expect("local candle query should not fail");

    assert_eq!(
        candles
            .iter()
            .map(|item| item.timestamp)
            .collect::<Vec<_>>(),
        vec![60_000, 120_000, 180_000]
    );
}

#[tokio::test]
async fn save_strategy_candles_persists_fetched_rows_for_next_decision() {
    let pool = SqlitePool::connect(":memory:")
        .await
        .expect("in-memory db should connect");
    create_candles_table(&pool).await;
    let candles = vec![
        sample_candle(60_000, 100.0),
        sample_candle(120_000, 101.0),
        sample_candle(180_000, 102.0),
    ];

    let saved = save_strategy_candles(&pool, "BTC-USDT-SWAP", "SWAP", "15m", &candles)
        .await
        .expect("fetched candles should persist");

    assert_eq!(saved, 3);
    let local = load_local_strategy_candles(&pool, "BTC-USDT-SWAP", "SWAP", "15m", 3)
        .await
        .expect("persisted fetched candles should be readable locally");
    assert_eq!(
        local.iter().map(|item| item.timestamp).collect::<Vec<_>>(),
        vec![60_000, 120_000, 180_000]
    );
}

#[tokio::test]
async fn timeframe_storage_kind_uses_completed_sync_job_derivation_record() {
    let pool = SqlitePool::connect(":memory:")
        .await
        .expect("in-memory db should connect");
    create_sync_jobs_table(&pool).await;
    create_sync_records_table(&pool).await;
    insert_sync_job(
        &pool,
        "job_derived",
        "completed",
        "15m",
        "1m",
        r#"["1m","15m"]"#,
        "2026-06-06T08:00:00Z",
    )
    .await;

    let kind = resolve_timeframe_storage_kind_from_db(&pool, "BTC-USDT-SWAP", "SWAP", "15m")
        .await
        .expect("storage kind should resolve from sync_jobs");

    assert_eq!(
        kind,
        TimeframeStorageKind::Derived {
            source_timeframe: "1m".to_string()
        }
    );
}

#[tokio::test]
async fn newer_direct_sync_job_overrides_older_derived_job() {
    let pool = SqlitePool::connect(":memory:")
        .await
        .expect("in-memory db should connect");
    create_sync_jobs_table(&pool).await;
    create_sync_records_table(&pool).await;
    insert_sync_job(
        &pool,
        "job_derived",
        "completed",
        "15m",
        "1m",
        r#"["1m","15m"]"#,
        "2026-06-06T08:00:00Z",
    )
    .await;
    insert_sync_job(
        &pool,
        "job_direct",
        "completed",
        "15m",
        "15m",
        r#"["15m"]"#,
        "2026-06-06T09:00:00Z",
    )
    .await;

    let kind = resolve_timeframe_storage_kind_from_db(&pool, "BTC-USDT-SWAP", "SWAP", "15m")
        .await
        .expect("latest direct sync job should resolve");

    assert_eq!(kind, TimeframeStorageKind::Direct);
}

#[tokio::test]
async fn timeframe_storage_kind_defaults_to_direct_without_db_derivation_record() {
    let pool = SqlitePool::connect(":memory:")
        .await
        .expect("in-memory db should connect");
    create_sync_jobs_table(&pool).await;
    create_sync_records_table(&pool).await;

    let kind = resolve_timeframe_storage_kind_from_db(&pool, "BTC-USDT-SWAP", "SWAP", "15m")
        .await
        .expect("missing derivation record should not fail");

    assert_eq!(kind, TimeframeStorageKind::Direct);
}

#[tokio::test]
async fn direct_sync_record_does_not_imply_derivation() {
    let pool = SqlitePool::connect(":memory:")
        .await
        .expect("in-memory db should connect");
    create_sync_jobs_table(&pool).await;
    create_sync_records_table(&pool).await;
    insert_sync_record(&pool, "15m", "window").await;

    let kind = resolve_timeframe_storage_kind_from_db(&pool, "BTC-USDT-SWAP", "SWAP", "15m")
        .await
        .expect("direct sync record should resolve");

    assert_eq!(kind, TimeframeStorageKind::Direct);
}

#[tokio::test]
async fn legacy_derive_sync_record_uses_base_source_timeframe() {
    let pool = SqlitePool::connect(":memory:")
        .await
        .expect("in-memory db should connect");
    create_sync_jobs_table(&pool).await;
    create_sync_records_table(&pool).await;
    insert_sync_record(&pool, "15m", "derive").await;

    let kind = resolve_timeframe_storage_kind_from_db(&pool, "BTC-USDT-SWAP", "SWAP", "15m")
        .await
        .expect("legacy derive record should resolve");

    assert_eq!(
        kind,
        TimeframeStorageKind::Derived {
            source_timeframe: "1m".to_string()
        }
    );
}

#[test]
fn newer_gap_exceeds_target_window_only_for_unusable_old_cache() {
    assert!(
        newer_gap_exceeds_target_window(Some(0), Some(10 * 60_000), 5, "1m"),
        "cache older than the requested target window must not be mixed with latest candles"
    );
    assert!(
        !newer_gap_exceeds_target_window(Some(0), Some(3 * 60_000), 5, "1m"),
        "cache inside the requested target window can be incrementally filled"
    );
}

#[test]
fn derived_timeframe_source_count_covers_target_window() {
    let required =
        source_candles_required_for_target(10_000, "1m", "15m").expect("15m should derive from 1m");

    assert_eq!(required, 150_015);
}

#[test]
fn aggregate_candles_from_source_builds_target_buckets() {
    let source = vec![
        sample_candle(0, 100.0),
        sample_candle(60_000, 101.0),
        sample_candle(120_000, 102.0),
        sample_candle(300_000, 105.0),
    ];

    let derived = aggregate_candles_from_source(&source, "5m");

    assert_eq!(
        derived
            .iter()
            .map(|item| item.timestamp)
            .collect::<Vec<_>>(),
        vec![0, 300_000]
    );
    assert_eq!(derived[0].open, 100.0);
    assert_eq!(derived[0].high, 103.0);
    assert_eq!(derived[0].low, 99.0);
    assert_eq!(derived[0].close, 102.0);
    assert_eq!(derived[0].volume, 3.0);
    assert_eq!(derived[1].open, 105.0);
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
          volume_quote REAL,
          UNIQUE(inst_id, inst_type, timeframe, timestamp)
        )
        "#,
    )
    .execute(pool)
    .await
    .expect("candles table should be created");
}

async fn create_sync_jobs_table(pool: &SqlitePool) {
    sqlx::query(
        r#"
        CREATE TABLE sync_jobs (
          task_id TEXT PRIMARY KEY,
          inst_id TEXT NOT NULL,
          inst_type TEXT NOT NULL DEFAULT 'SPOT',
          timeframe TEXT NOT NULL,
          source_timeframe TEXT NOT NULL DEFAULT '1m',
          target_timeframes TEXT NOT NULL DEFAULT '[]',
          status TEXT NOT NULL DEFAULT 'queued',
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await
    .expect("sync_jobs table should be created");
}

async fn create_sync_records_table(pool: &SqlitePool) {
    sqlx::query(
        r#"
        CREATE TABLE sync_records (
          inst_id TEXT NOT NULL,
          inst_type TEXT NOT NULL DEFAULT 'SPOT',
          timeframe TEXT NOT NULL,
          last_sync_mode TEXT NOT NULL DEFAULT 'window',
          UNIQUE(inst_id, inst_type, timeframe)
        )
        "#,
    )
    .execute(pool)
    .await
    .expect("sync_records table should be created");
}

async fn insert_sync_job(
    pool: &SqlitePool,
    task_id: &str,
    status: &str,
    timeframe: &str,
    source_timeframe: &str,
    target_timeframes: &str,
    updated_at: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO sync_jobs (
          task_id, inst_id, inst_type, timeframe, source_timeframe,
          target_timeframes, status, created_at, updated_at
        )
        VALUES (?, 'BTC-USDT-SWAP', 'SWAP', ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(task_id)
    .bind(timeframe)
    .bind(source_timeframe)
    .bind(target_timeframes)
    .bind(status)
    .bind(updated_at)
    .bind(updated_at)
    .execute(pool)
    .await
    .expect("sync job row should insert");
}

async fn insert_sync_record(pool: &SqlitePool, timeframe: &str, last_sync_mode: &str) {
    sqlx::query(
        r#"
        INSERT INTO sync_records (inst_id, inst_type, timeframe, last_sync_mode)
        VALUES ('BTC-USDT-SWAP', 'SWAP', ?, ?)
        "#,
    )
    .bind(timeframe)
    .bind(last_sync_mode)
    .execute(pool)
    .await
    .expect("sync record row should insert");
}

async fn insert_candle(pool: &SqlitePool, timestamp: i64, open: &str) {
    insert_candle_with_timeframe(pool, "15m", timestamp, open).await;
}

async fn insert_candle_with_timeframe(
    pool: &SqlitePool,
    timeframe: &str,
    timestamp: i64,
    open: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO candles (
          inst_id, inst_type, timeframe, timestamp,
          open, high, low, close, volume, volume_ccy, volume_quote
        )
        VALUES ('BTC-USDT-SWAP', 'SWAP', ?, ?, ?, 105.0, 95.0, 100.0, 1.0, 1.0, ?)
        "#,
    )
    .bind(timeframe)
    .bind(timestamp)
    .bind(open)
    .bind(1.0)
    .execute(pool)
    .await
    .expect("candle row should insert");
}

fn sample_candle(timestamp: i64, close: f64) -> OkxCandle {
    OkxCandle {
        timestamp,
        open: close,
        high: close + 1.0,
        low: close - 1.0,
        close,
        volume: 1.0,
        volume_ccy: close,
        volume_quote: close,
        confirm: "1".to_string(),
    }
}
