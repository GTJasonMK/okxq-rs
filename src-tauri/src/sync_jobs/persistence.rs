use anyhow::Result;

use super::{
    manager::SyncJobManager, timeframes::target_timeframes_json, types::SyncJob, utils::bool_i64,
};

impl SyncJobManager {
    pub(in crate::sync_jobs) async fn persist_running_job_if_active(
        &self,
        job: &SyncJob,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sync_jobs
            SET status = ?,
                progress = ?,
                message = ?,
                started_at = ?,
                updated_at = ?,
                fetched_count = ?,
                target_fetch_count = ?,
                saved_count = ?,
                target_save_count = ?,
                inserted_count = ?,
                derived_count = ?,
                target_derive_count = ?,
                batches = ?,
                target_batches = ?,
                api_calls = ?
            WHERE task_id = ? AND status IN ('queued', 'running')
            "#,
        )
        .bind(&job.status)
        .bind(job.progress)
        .bind(&job.message)
        .bind(&job.started_at)
        .bind(&job.updated_at)
        .bind(job.fetched_count)
        .bind(job.target_fetch_count)
        .bind(job.saved_count)
        .bind(job.target_save_count)
        .bind(job.inserted_count)
        .bind(job.derived_count)
        .bind(job.target_derive_count)
        .bind(job.batches)
        .bind(job.target_batches)
        .bind(job.api_calls)
        .bind(&job.task_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub(in crate::sync_jobs) async fn persist_job(&self, job: &SyncJob) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sync_jobs (
              task_id, inst_id, inst_type, timeframe, source_timeframe, target_timeframes,
              mode, days, start_ts, end_ts, repair_method, status, progress,
              message, created_at, started_at, updated_at, finished_at, error,
              fetched_count, target_fetch_count,
              saved_count, target_save_count,
              inserted_count, derived_count, target_derive_count,
              batches, target_batches, api_calls, candle_count,
              history_complete, last_sync_mode, last_sync_time,
              oldest_timestamp, newest_timestamp, oldest_time, newest_time,
              reused_existing, truncated, cancel_requested
            ) VALUES (
              ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
              ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
              ?, ?, ?
            )
            ON CONFLICT(task_id) DO UPDATE SET
              inst_id = excluded.inst_id,
              inst_type = excluded.inst_type,
              timeframe = excluded.timeframe,
              source_timeframe = excluded.source_timeframe,
              target_timeframes = excluded.target_timeframes,
              mode = excluded.mode,
              days = excluded.days,
              start_ts = excluded.start_ts,
              end_ts = excluded.end_ts,
              repair_method = excluded.repair_method,
              status = excluded.status,
              progress = excluded.progress,
              message = excluded.message,
              started_at = excluded.started_at,
              updated_at = excluded.updated_at,
              finished_at = excluded.finished_at,
              error = excluded.error,
              fetched_count = excluded.fetched_count,
              target_fetch_count = excluded.target_fetch_count,
              saved_count = excluded.saved_count,
              target_save_count = excluded.target_save_count,
              inserted_count = excluded.inserted_count,
              derived_count = excluded.derived_count,
              target_derive_count = excluded.target_derive_count,
              batches = excluded.batches,
              target_batches = excluded.target_batches,
              api_calls = excluded.api_calls,
              candle_count = excluded.candle_count,
              history_complete = excluded.history_complete,
              last_sync_mode = excluded.last_sync_mode,
              last_sync_time = excluded.last_sync_time,
              oldest_timestamp = excluded.oldest_timestamp,
              newest_timestamp = excluded.newest_timestamp,
              oldest_time = excluded.oldest_time,
              newest_time = excluded.newest_time,
              reused_existing = excluded.reused_existing,
              truncated = excluded.truncated,
              cancel_requested = excluded.cancel_requested
            "#,
        )
        .bind(&job.task_id)
        .bind(&job.inst_id)
        .bind(&job.inst_type)
        .bind(&job.timeframe)
        .bind(&job.source_timeframe)
        .bind(target_timeframes_json(&job.target_timeframes)?)
        .bind(&job.mode)
        .bind(job.days)
        .bind(job.start_ts)
        .bind(job.end_ts)
        .bind(&job.repair_method)
        .bind(&job.status)
        .bind(job.progress)
        .bind(&job.message)
        .bind(&job.created_at)
        .bind(&job.started_at)
        .bind(&job.updated_at)
        .bind(&job.finished_at)
        .bind(&job.error)
        .bind(job.fetched_count)
        .bind(job.target_fetch_count)
        .bind(job.saved_count)
        .bind(job.target_save_count)
        .bind(job.inserted_count)
        .bind(job.derived_count)
        .bind(job.target_derive_count)
        .bind(job.batches)
        .bind(job.target_batches)
        .bind(job.api_calls)
        .bind(job.candle_count)
        .bind(bool_i64(job.history_complete))
        .bind(&job.last_sync_mode)
        .bind(&job.last_sync_time)
        .bind(job.oldest_timestamp)
        .bind(job.newest_timestamp)
        .bind(&job.oldest_time)
        .bind(&job.newest_time)
        .bind(bool_i64(job.reused_existing))
        .bind(bool_i64(job.truncated))
        .bind(bool_i64(job.cancel_requested))
        .execute(&self.db)
        .await?;
        Ok(())
    }
}
