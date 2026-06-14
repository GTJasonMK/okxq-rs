use sqlx::{Row, SqlitePool};

use crate::error::AppResult;

use super::progress::update_rebuild_progress;

#[derive(Clone, Debug)]
pub(super) struct CandleGroupScope {
    pub(super) inst_id: String,
    pub(super) inst_type: String,
    pub(super) timeframe: String,
}

#[derive(Clone, Debug)]
pub(super) struct CandleGroupStats {
    pub(super) scope: CandleGroupScope,
    pub(super) oldest_timestamp: i64,
    pub(super) newest_timestamp: i64,
    pub(super) candle_count: i64,
}

pub(super) async fn scan_candle_groups(
    pool: &SqlitePool,
    task_id: Option<&str>,
) -> AppResult<Vec<CandleGroupStats>> {
    update_rebuild_progress(task_id, |progress| {
        progress.phase = "scanning".to_string();
        progress.progress = 5;
        progress.scan_concurrency = 1;
        progress.message = "单次聚合扫描 candles 周期分组中".to_string();
    })
    .await;
    let rows = sqlx::query(
        r#"
        SELECT inst_id, inst_type, timeframe,
               MIN(timestamp) AS oldest_timestamp,
               MAX(timestamp) AS newest_timestamp,
               COUNT(*) AS candle_count
        FROM candles INDEXED BY idx_candles_query
        GROUP BY inst_id, inst_type, timeframe
        ORDER BY inst_id, inst_type, timeframe
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut groups = Vec::with_capacity(rows.len());
    let mut processed_candles = 0i64;
    for row in rows {
        let Some(group) = candle_group_stats_from_row(row)? else {
            continue;
        };
        processed_candles += group.candle_count;
        groups.push(group);
    }

    let processed_groups = groups.len() as i64;
    update_rebuild_progress(task_id, |progress| {
        progress.processed_groups = processed_groups;
        progress.target_groups = processed_groups;
        progress.candle_groups_scanned = processed_groups;
        progress.processed_candles = processed_candles;
        progress.scan_concurrency = 1;
        progress.progress = 75;
        progress.message = format!(
            "已单次聚合扫描 candles：{} 组，{} 根 K 线",
            processed_groups, processed_candles
        );
    })
    .await;

    Ok(groups)
}

fn candle_group_stats_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> AppResult<Option<CandleGroupStats>> {
    let inst_id = row.try_get::<String, _>("inst_id")?.trim().to_uppercase();
    if inst_id.is_empty() {
        return Ok(None);
    }
    let inst_type = row.try_get::<String, _>("inst_type")?.trim().to_uppercase();
    let timeframe = row.try_get::<String, _>("timeframe")?.trim().to_string();
    let candle_count = row.try_get::<i64, _>("candle_count")?;
    if inst_type.is_empty() || timeframe.is_empty() || candle_count <= 0 {
        return Ok(None);
    }
    let Some(oldest_timestamp) = row.try_get::<Option<i64>, _>("oldest_timestamp")? else {
        return Ok(None);
    };
    let Some(newest_timestamp) = row.try_get::<Option<i64>, _>("newest_timestamp")? else {
        return Ok(None);
    };

    Ok(Some(CandleGroupStats {
        scope: CandleGroupScope {
            inst_id,
            inst_type,
            timeframe,
        },
        oldest_timestamp,
        newest_timestamp,
        candle_count,
    }))
}
