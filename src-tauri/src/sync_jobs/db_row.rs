use anyhow::{anyhow, Result};
use sqlx::{sqlite::SqliteRow, Row};

use super::{
    timeframes::{
        normalize_required_timeframe, normalize_target_timeframes, target_timeframes_json,
    },
    types::{sync_job_ts_key, SyncJob},
};

pub(super) const SYNC_JOB_SELECT: &str = r#"
    SELECT task_id, inst_id, inst_type, timeframe, source_timeframe, target_timeframes,
           mode, days, start_ts, end_ts, repair_method, status, progress,
           message, created_at, started_at, updated_at, finished_at, error,
           fetched_count, target_fetch_count,
           saved_count, target_save_count,
           inserted_count, derived_count, target_derive_count,
           batches, target_batches, api_calls, candle_count,
           history_complete, last_sync_mode, last_sync_time,
           oldest_timestamp, newest_timestamp, oldest_time, newest_time,
           reused_existing, truncated, cancel_requested
    FROM sync_jobs
"#;

pub(super) fn sync_job_from_row(row: SqliteRow) -> Result<SyncJob> {
    let raw_timeframe = row.try_get::<String, _>("timeframe")?;
    let timeframe = normalize_required_timeframe(&raw_timeframe, "timeframe")?;
    let raw_source_timeframe = row.try_get::<String, _>("source_timeframe")?;
    let source_timeframe = normalize_required_timeframe(&raw_source_timeframe, "source_timeframe")?;
    let raw_target_timeframes = row.try_get::<String, _>("target_timeframes")?;
    let parsed_target_timeframes = serde_json::from_str::<Vec<String>>(&raw_target_timeframes)
        .map_err(|error| anyhow!("invalid sync target_timeframes json: {error}"))?;
    let target_timeframes = normalize_target_timeframes(parsed_target_timeframes, &timeframe)?;
    Ok(SyncJob {
        task_id: row.try_get("task_id")?,
        inst_id: row.try_get::<String, _>("inst_id")?.trim().to_uppercase(),
        inst_type: row.try_get::<String, _>("inst_type")?.trim().to_uppercase(),
        timeframe,
        source_timeframe,
        target_timeframes,
        mode: row.try_get::<String, _>("mode")?,
        days: row.try_get::<i64, _>("days")?.max(1),
        start_ts: row.try_get::<Option<i64>, _>("start_ts")?,
        end_ts: row.try_get::<Option<i64>, _>("end_ts")?,
        repair_method: row.try_get::<String, _>("repair_method")?,
        status: row.try_get::<String, _>("status")?,
        progress: row.try_get::<i64, _>("progress")?,
        message: row.try_get::<String, _>("message")?,
        created_at: row.try_get::<String, _>("created_at")?,
        started_at: row.try_get::<Option<String>, _>("started_at")?,
        updated_at: row.try_get::<String, _>("updated_at")?,
        finished_at: row.try_get::<Option<String>, _>("finished_at")?,
        error: row.try_get::<String, _>("error")?,
        fetched_count: row.try_get::<i64, _>("fetched_count")?,
        target_fetch_count: row.try_get::<i64, _>("target_fetch_count")?,
        saved_count: row.try_get::<i64, _>("saved_count")?,
        target_save_count: row.try_get::<i64, _>("target_save_count")?,
        inserted_count: row.try_get::<i64, _>("inserted_count")?,
        derived_count: row.try_get::<i64, _>("derived_count")?,
        target_derive_count: row.try_get::<i64, _>("target_derive_count")?,
        batches: row.try_get::<i64, _>("batches")?,
        target_batches: row.try_get::<i64, _>("target_batches")?,
        api_calls: row.try_get::<i64, _>("api_calls")?,
        candle_count: row.try_get::<i64, _>("candle_count")?,
        history_complete: row.try_get::<i64, _>("history_complete")? != 0,
        last_sync_mode: row.try_get::<String, _>("last_sync_mode")?,
        last_sync_time: row.try_get::<Option<String>, _>("last_sync_time")?,
        oldest_timestamp: row.try_get::<Option<i64>, _>("oldest_timestamp")?,
        newest_timestamp: row.try_get::<Option<i64>, _>("newest_timestamp")?,
        oldest_time: row.try_get::<Option<String>, _>("oldest_time")?,
        newest_time: row.try_get::<Option<String>, _>("newest_time")?,
        reused_existing: row.try_get::<i64, _>("reused_existing")? != 0,
        truncated: row.try_get::<i64, _>("truncated")? != 0,
        cancel_requested: row.try_get::<i64, _>("cancel_requested")? != 0,
    })
}

pub(super) fn job_key(job: &SyncJob) -> Result<String> {
    let timeframe = normalize_required_timeframe(&job.timeframe, "timeframe")?;
    let source_timeframe = normalize_required_timeframe(&job.source_timeframe, "source_timeframe")?;
    let target_timeframes = normalize_target_timeframes(job.target_timeframes.clone(), &timeframe)?;
    Ok(format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}",
        job.inst_id.trim().to_uppercase(),
        job.inst_type.trim().to_uppercase(),
        source_timeframe,
        target_timeframes_json(&target_timeframes)?,
        job.mode,
        job.days.max(1),
        sync_job_ts_key(job.start_ts),
        sync_job_ts_key(job.end_ts),
        job.repair_method.trim().to_ascii_lowercase()
    ))
}
