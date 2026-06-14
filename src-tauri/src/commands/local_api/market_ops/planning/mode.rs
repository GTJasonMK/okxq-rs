use sqlx::SqlitePool;

use super::{super::*, plans::WatchSyncPlan};

pub(super) async fn select_watch_sync_mode(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    plan: &WatchSyncPlan,
    archive_all_history: bool,
    eager_full_on_missing: bool,
) -> AppResult<String> {
    let Some(record) = get_sync_record_stats(pool, inst_id, inst_type, &plan.timeframe).await?
    else {
        if eager_full_on_missing && (archive_all_history || plan.archive_mode == "full") {
            return Ok("full".to_string());
        }
        return Ok("window".to_string());
    };
    if record.candle_count <= 0 {
        if eager_full_on_missing && (archive_all_history || plan.archive_mode == "full") {
            return Ok("full".to_string());
        }
        return Ok("window".to_string());
    }
    if record.history_complete {
        return Ok("incremental".to_string());
    }
    if archive_all_history || plan.archive_mode == "full" {
        return Ok("full".to_string());
    }

    update_sync_record(pool, inst_id, inst_type, &plan.timeframe, Some(true), None).await?;
    Ok("incremental".to_string())
}
