use super::payload::{build_inventory_payload, InventoryBuildOptions};
use super::*;

mod progress;
mod scan;

use self::progress::{
    current_rebuild_progress, current_rebuild_progress_value, mark_rebuild_completed,
    mark_rebuild_failed, next_rebuild_task_id, set_rebuild_progress, update_rebuild_progress,
    InventoryCacheRebuildProgress,
};
use self::scan::scan_candle_groups;

const DEFAULT_REBUILD_SCAN_CONCURRENCY: usize = 8;
const MAX_REBUILD_SCAN_CONCURRENCY: usize = 8;

#[derive(Debug, Default)]
struct InventoryCacheRebuildReport {
    candle_groups_scanned: i64,
    sync_records_rebuilt: i64,
    stale_sync_records_deleted: i64,
    sync_records_total: i64,
    cached_candles_total: i64,
}

pub(crate) async fn rebuild_inventory_cache(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let concurrency = rebuild_scan_concurrency(request_i64(
        req,
        "concurrency",
        DEFAULT_REBUILD_SCAN_CONCURRENCY as i64,
    ));
    if request_bool(req, "background", false) {
        let (progress, reused_existing) =
            start_inventory_cache_rebuild_job(&state.db, concurrency).await;
        return Ok(code_ok(json!({
            "reused_existing": reused_existing,
            "progress": progress.to_value(),
        })));
    }

    let include_storage_counts = request_bool(req, "include_storage_counts", false);
    let task_id = next_rebuild_task_id();
    set_rebuild_progress(InventoryCacheRebuildProgress::queued(task_id.clone())).await;
    let report =
        match rebuild_sync_records_from_candles(&state.db, concurrency, Some(&task_id)).await {
            Ok(report) => report,
            Err(error) => {
                mark_rebuild_failed(&task_id, error.to_string()).await;
                return Err(error);
            }
        };
    mark_rebuild_completed(&task_id, &report).await;
    let inventory = build_inventory_payload(
        state,
        InventoryBuildOptions {
            include_storage_counts,
            ..Default::default()
        },
    )
    .await?;
    Ok(code_ok(json!({
        "message": "库存缓存已按 candles 全库扫描重建",
        "candle_groups_scanned": report.candle_groups_scanned,
        "sync_records_rebuilt": report.sync_records_rebuilt,
        "stale_sync_records_deleted": report.stale_sync_records_deleted,
        "sync_records_total": report.sync_records_total,
        "cached_candles_total": report.cached_candles_total,
        "inventory": inventory,
        "progress": current_rebuild_progress_value().await,
    })))
}

pub(crate) async fn inventory_cache_rebuild_status() -> AppResult<Value> {
    Ok(code_ok(json!({
        "progress": current_rebuild_progress_value().await,
    })))
}

async fn rebuild_sync_records_from_candles(
    pool: &SqlitePool,
    _concurrency: usize,
    task_id: Option<&str>,
) -> AppResult<InventoryCacheRebuildReport> {
    update_rebuild_progress(task_id, |progress| {
        progress.status = "running".to_string();
        progress.phase = "scanning".to_string();
        progress.progress = 2;
        progress.scan_concurrency = 1;
        progress.message = "单次聚合扫描 candles 周期分组中".to_string();
    })
    .await;

    let groups = scan_candle_groups(pool, task_id).await?;
    let processed_candles = groups.iter().map(|group| group.candle_count).sum::<i64>();
    update_rebuild_progress(task_id, |progress| {
        progress.processed_candles = processed_candles;
        progress.target_candles = processed_candles;
        progress.processed_groups = groups.len() as i64;
        progress.target_groups = groups.len() as i64;
        progress.candle_groups_scanned = groups.len() as i64;
        progress.phase = "rebuilding".to_string();
        progress.progress = 75;
        progress.message = format!("重建 sync_records 中：0 / {} 组", groups.len());
    })
    .await;

    let mut tx = pool.begin().await?;
    let mut rebuilt = 0i64;
    let group_total = groups.len().max(1) as i64;
    for group in groups {
        sqlx::query(
            r#"
            INSERT INTO sync_records (
              inst_id, inst_type, timeframe, last_sync_time,
              oldest_timestamp, newest_timestamp, candle_count,
              history_complete, last_sync_mode
            ) VALUES (?, ?, ?, CURRENT_TIMESTAMP, ?, ?, ?, 0, 'inventory_rebuild')
            ON CONFLICT(inst_id, inst_type, timeframe) DO UPDATE SET
              last_sync_time = CURRENT_TIMESTAMP,
              oldest_timestamp = excluded.oldest_timestamp,
              newest_timestamp = excluded.newest_timestamp,
              candle_count = excluded.candle_count,
              history_complete = sync_records.history_complete,
              last_sync_mode = CASE
                WHEN sync_records.last_sync_mode IS NULL OR sync_records.last_sync_mode = ''
                THEN excluded.last_sync_mode
                ELSE sync_records.last_sync_mode
              END
            "#,
        )
        .bind(group.scope.inst_id)
        .bind(group.scope.inst_type)
        .bind(group.scope.timeframe)
        .bind(group.oldest_timestamp)
        .bind(group.newest_timestamp)
        .bind(group.candle_count)
        .execute(&mut *tx)
        .await?;
        rebuilt += 1;
        if rebuilt == group_total || rebuilt % 25 == 0 {
            let rebuild_progress = 75 + ((rebuilt * 15) / group_total);
            update_rebuild_progress(task_id, |progress| {
                progress.sync_records_rebuilt = rebuilt;
                progress.progress = rebuild_progress.min(90);
                progress.message =
                    format!("重建 sync_records 中：{} / {} 组", rebuilt, group_total);
            })
            .await;
        }
    }

    update_rebuild_progress(task_id, |progress| {
        progress.phase = "cleanup".to_string();
        progress.progress = 92;
        progress.message = "清理陈旧 sync_records 中".to_string();
    })
    .await;
    let stale_deleted = sqlx::query(
        r#"
        DELETE FROM sync_records
        WHERE NOT EXISTS (
          SELECT 1
          FROM candles
          WHERE candles.inst_id = sync_records.inst_id
            AND candles.inst_type = sync_records.inst_type
            AND candles.timeframe = sync_records.timeframe
          LIMIT 1
        )
        "#,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected() as i64;
    update_rebuild_progress(task_id, |progress| {
        progress.stale_sync_records_deleted = stale_deleted;
        progress.progress = 96;
        progress.message = format!("已清理陈旧缓存 {} 条，汇总缓存状态中", stale_deleted);
    })
    .await;

    let totals = sqlx::query(
        r#"
        SELECT COUNT(*) AS sync_records_total,
               COALESCE(SUM(candle_count), 0) AS cached_candles_total
        FROM sync_records
        "#,
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    let report = InventoryCacheRebuildReport {
        candle_groups_scanned: rebuilt,
        sync_records_rebuilt: rebuilt,
        stale_sync_records_deleted: stale_deleted,
        sync_records_total: totals.try_get::<i64, _>("sync_records_total")?,
        cached_candles_total: totals.try_get::<i64, _>("cached_candles_total")?,
    };
    update_rebuild_progress(task_id, |progress| {
        progress.sync_records_total = report.sync_records_total;
        progress.cached_candles_total = report.cached_candles_total;
        progress.progress = 98;
        progress.message = "库存缓存重建已写入，准备刷新库存视图".to_string();
    })
    .await;
    Ok(report)
}

async fn start_inventory_cache_rebuild_job(
    pool: &SqlitePool,
    concurrency: usize,
) -> (InventoryCacheRebuildProgress, bool) {
    if let Some(progress) = current_rebuild_progress().await {
        if progress.is_active() {
            return (progress, true);
        }
    }

    let task_id = next_rebuild_task_id();
    let progress = InventoryCacheRebuildProgress::queued(task_id.clone());
    set_rebuild_progress(progress.clone()).await;
    let pool = pool.clone();
    tokio::spawn(async move {
        match rebuild_sync_records_from_candles(&pool, concurrency, Some(&task_id)).await {
            Ok(report) => mark_rebuild_completed(&task_id, &report).await,
            Err(error) => mark_rebuild_failed(&task_id, error.to_string()).await,
        }
    });
    (progress, false)
}

fn rebuild_scan_concurrency(value: i64) -> usize {
    value.clamp(1, MAX_REBUILD_SCAN_CONCURRENCY as i64) as usize
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

    async fn insert_candle(
        pool: &SqlitePool,
        inst_id: &str,
        inst_type: &str,
        timeframe: &str,
        timestamp: i64,
    ) {
        sqlx::query(
            r#"
            INSERT INTO candles (
              inst_id, inst_type, timeframe, timestamp,
              open, high, low, close, volume, volume_ccy
            ) VALUES (?, ?, ?, ?, 1, 1, 1, 1, 1, 1)
            "#,
        )
        .bind(inst_id)
        .bind(inst_type)
        .bind(timeframe)
        .bind(timestamp)
        .execute(pool)
        .await
        .expect("insert candle");
    }

    #[tokio::test]
    async fn rebuild_sync_records_from_candles_refreshes_ranges_and_removes_stale_rows() {
        let pool = test_pool().await;
        insert_candle(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 60_000).await;
        insert_candle(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 120_000).await;
        insert_candle(&pool, "ETH-USDT", "SPOT", "3m", 180_000).await;
        sqlx::query(
            r#"
            INSERT INTO sync_records (
              inst_id, inst_type, timeframe, last_sync_time,
              oldest_timestamp, newest_timestamp, candle_count,
              history_complete, last_sync_mode
            ) VALUES
              ('BTC-USDT-SWAP', 'SWAP', '1m', CURRENT_TIMESTAMP, 0, 0, 0, 1, 'full'),
              ('DOGE-USDT-SWAP', 'SWAP', '1m', CURRENT_TIMESTAMP, 0, 0, 1, 0, 'window')
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert stale sync records");

        let report = rebuild_sync_records_from_candles(&pool, 2, None)
            .await
            .expect("rebuild cache");

        assert_eq!(report.candle_groups_scanned, 2);
        assert_eq!(report.sync_records_rebuilt, 2);
        assert_eq!(report.stale_sync_records_deleted, 1);
        assert_eq!(report.sync_records_total, 2);
        assert_eq!(report.cached_candles_total, 3);

        let btc = sqlx::query(
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
        .expect("btc sync record");
        assert_eq!(btc.try_get::<i64, _>("oldest_timestamp").unwrap(), 60_000);
        assert_eq!(btc.try_get::<i64, _>("newest_timestamp").unwrap(), 120_000);
        assert_eq!(btc.try_get::<i64, _>("candle_count").unwrap(), 2);
        assert_eq!(btc.try_get::<i64, _>("history_complete").unwrap(), 1);
        assert_eq!(btc.try_get::<String, _>("last_sync_mode").unwrap(), "full");

        let eth_mode: String = sqlx::query(
            "SELECT last_sync_mode FROM sync_records WHERE inst_id = 'ETH-USDT' AND inst_type = 'SPOT' AND timeframe = '3m'",
        )
        .fetch_one(&pool)
        .await
        .expect("eth sync record")
        .try_get("last_sync_mode")
        .unwrap();
        assert_eq!(eth_mode, "inventory_rebuild");

        let stale_count: i64 = sqlx::query(
            "SELECT COUNT(*) AS count FROM sync_records WHERE inst_id = 'DOGE-USDT-SWAP'",
        )
        .fetch_one(&pool)
        .await
        .expect("stale count")
        .try_get("count")
        .unwrap();
        assert_eq!(stale_count, 0);
    }
}
