use sqlx::SqlitePool;

use crate::{error::AppResult, sync_jobs::SyncJobRequest};

use super::super::super::*;

pub(super) async fn update_base_sync_record(
    pool: &SqlitePool,
    request: &SyncJobRequest,
    source_timeframe: &str,
    fetch_result: &SyncFetchResult,
) -> AppResult<SyncRecordStats> {
    if request.mode == "derive" {
        return match get_sync_record_stats(
            pool,
            &request.inst_id,
            &request.inst_type,
            source_timeframe,
        )
        .await?
        {
            Some(record) => Ok(record),
            None => {
                update_sync_record(
                    pool,
                    &request.inst_id,
                    &request.inst_type,
                    source_timeframe,
                    None,
                    None,
                )
                .await
            }
        };
    }

    let history_update = if request.mode == "full" {
        Some(fetch_result.history_complete)
    } else {
        None
    };
    update_sync_record(
        pool,
        &request.inst_id,
        &request.inst_type,
        source_timeframe,
        history_update,
        Some(&request.mode),
    )
    .await
}
