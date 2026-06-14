use anyhow::Result;
use serde_json::Value;
use std::time::Instant;

use super::{
    manager::SyncJobManager,
    payload::{
        optional_i64, optional_string, required_bool, required_i64, required_string,
        required_string_array,
    },
    timeframes::{normalize_required_timeframe, normalize_target_timeframes},
    types::{SyncJob, SyncJobRequest, SyncJobRunningUpdate},
    utils::now_text,
};

struct SyncJobCompletionPayload {
    message: String,
    fetched_count: i64,
    target_fetch_count: i64,
    saved_count: i64,
    target_save_count: i64,
    inserted_count: i64,
    derived_count: i64,
    target_derive_count: i64,
    batches: i64,
    target_batches: i64,
    api_calls: i64,
    candle_count: i64,
    source_timeframe: String,
    target_timeframes: Vec<String>,
    history_complete: bool,
    last_sync_mode: String,
    last_sync_time: Option<String>,
    oldest_timestamp: Option<i64>,
    newest_timestamp: Option<i64>,
    oldest_time: Option<String>,
    newest_time: Option<String>,
    truncated: bool,
}

impl SyncJobCompletionPayload {
    fn parse(payload: &Value, job_timeframe: &str) -> Result<Self> {
        let source_timeframe = normalize_required_timeframe(
            &required_string(payload, "source_timeframe")?,
            "source_timeframe",
        )?;
        let target_timeframes = normalize_target_timeframes(
            required_string_array(payload, "target_timeframes")?,
            job_timeframe,
        )?;
        Ok(Self {
            message: required_string(payload, "message")?,
            fetched_count: required_i64(payload, "fetched_count")?,
            target_fetch_count: required_i64(payload, "target_fetch_count")?,
            saved_count: required_i64(payload, "saved_count")?,
            target_save_count: required_i64(payload, "target_save_count")?,
            inserted_count: required_i64(payload, "inserted_count")?,
            derived_count: required_i64(payload, "derived_count")?,
            target_derive_count: required_i64(payload, "target_derive_count")?,
            batches: required_i64(payload, "batches")?,
            target_batches: required_i64(payload, "target_batches")?,
            api_calls: required_i64(payload, "api_calls")?,
            candle_count: required_i64(payload, "candle_count")?,
            source_timeframe,
            target_timeframes,
            history_complete: required_bool(payload, "history_complete")?,
            last_sync_mode: required_string(payload, "last_sync_mode")?,
            last_sync_time: optional_string(payload, "last_sync_time")?,
            oldest_timestamp: optional_i64(payload, "oldest_timestamp")?,
            newest_timestamp: optional_i64(payload, "newest_timestamp")?,
            oldest_time: optional_string(payload, "oldest_time")?,
            newest_time: optional_string(payload, "newest_time")?,
            truncated: required_bool(payload, "truncated")?,
        })
    }
}

impl SyncJobManager {
    pub async fn recover_interrupted_jobs(&self) -> Result<()> {
        let now = now_text();
        sqlx::query(
            r#"
            UPDATE sync_jobs
            SET status = 'failed',
                progress = 100,
                message = '应用重启，后台同步任务已中断，可重新发起同步',
                error = '应用重启导致任务中断',
                updated_at = ?,
                finished_at = ?,
                cancel_requested = 0
            WHERE status IN ('queued', 'running')
            "#,
        )
        .bind(&now)
        .bind(&now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn create_or_reuse(&self, req: SyncJobRequest) -> Result<(SyncJob, bool)> {
        let key = req.key()?;
        let coverage_key = req.coverage_key()?;
        let mut inner = self.inner.lock().await;

        if let Some(existing_task_id) = inner.active_keys.get(&key).cloned() {
            if let Some(existing) = inner
                .jobs
                .get(&existing_task_id)
                .filter(|job| job.is_active())
            {
                let mut reused = existing.clone();
                reused.reused_existing = true;
                return Ok((reused, true));
            }
            inner.remove_active_task(&existing_task_id);
        }

        if let Some(existing) = inner.covering_active_job(&req, &coverage_key)? {
            inner.active_keys.insert(key, existing.task_id.clone());
            let mut reused = existing;
            reused.reused_existing = true;
            return Ok((reused, true));
        }

        if let Some(existing) = self.fetch_active_job_for_request(&req).await? {
            inner.track_active_job(&existing)?;
            inner
                .jobs
                .insert(existing.task_id.clone(), existing.clone());
            let mut reused = existing;
            reused.reused_existing = true;
            return Ok((reused, true));
        }

        let now = now_text();
        let task_id = format!("sync_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let timeframe = normalize_required_timeframe(&req.timeframe, "timeframe")?;
        let source_timeframe =
            normalize_required_timeframe(&req.source_timeframe, "source_timeframe")?;
        let target_timeframes = normalize_target_timeframes(req.target_timeframes, &timeframe)?;
        let job = SyncJob {
            task_id: task_id.clone(),
            inst_id: req.inst_id.trim().to_uppercase(),
            inst_type: req.inst_type.trim().to_uppercase(),
            timeframe,
            source_timeframe,
            target_timeframes,
            mode: req.mode,
            days: req.days.max(1),
            start_ts: req.start_ts,
            end_ts: req.end_ts,
            repair_method: req.repair_method,
            status: "queued".to_string(),
            progress: 0,
            message: "等待开始".to_string(),
            created_at: now.clone(),
            started_at: None,
            updated_at: now,
            finished_at: None,
            error: String::new(),
            fetched_count: 0,
            target_fetch_count: req.target_fetch_count.max(0),
            saved_count: 0,
            target_save_count: req.target_save_count.max(0),
            inserted_count: 0,
            derived_count: 0,
            target_derive_count: req.target_derive_count.max(0),
            batches: 0,
            target_batches: req.target_batches.max(0),
            api_calls: 0,
            candle_count: 0,
            history_complete: false,
            last_sync_mode: "window".to_string(),
            last_sync_time: None,
            oldest_timestamp: None,
            newest_timestamp: None,
            oldest_time: None,
            newest_time: None,
            reused_existing: false,
            truncated: false,
            cancel_requested: false,
        };

        self.persist_job(&job).await?;
        inner.active_keys.insert(key, task_id.clone());
        inner
            .active_coverage_keys
            .entry(coverage_key)
            .or_default()
            .insert(task_id.clone());
        inner.jobs.insert(task_id, job.clone());
        Ok((job, false))
    }

    pub async fn mark_running(&self, task_id: &str) -> Result<()> {
        let persist_job = {
            let mut inner = self.inner.lock().await;
            let Some(job) = inner.jobs.get_mut(task_id) else {
                return Ok(());
            };
            if job.status == "cancelled" {
                return Ok(());
            }
            let now = now_text();
            job.status = "running".to_string();
            job.progress = 1;
            job.message = "后台任务已启动".to_string();
            job.started_at = Some(now.clone());
            job.updated_at = now;
            job.clone()
        };
        self.persist_running_job_if_active(&persist_job).await?;
        let mut inner = self.inner.lock().await;
        if inner
            .jobs
            .get(task_id)
            .map(SyncJob::is_active)
            .unwrap_or(false)
        {
            inner
                .running_persisted_at
                .insert(task_id.to_string(), Instant::now());
        }
        Ok(())
    }

    pub async fn update_running(&self, task_id: &str, update: SyncJobRunningUpdate) -> Result<()> {
        let now_instant = Instant::now();
        let persist_job = {
            let mut inner = self.inner.lock().await;
            let should_persist = inner.running_update_persist_due(task_id, now_instant);
            let Some(job) = inner.jobs.get_mut(task_id) else {
                return Ok(());
            };
            if job.status == "cancelled" {
                return Ok(());
            }
            let now = now_text();
            job.status = "running".to_string();
            job.progress = job.progress.max(update.progress.clamp(0, 99));
            job.message = update.message;
            job.updated_at = now;
            job.fetched_count = job.fetched_count.max(update.fetched_count);
            job.target_fetch_count = job.target_fetch_count.max(update.target_fetch_count);
            job.saved_count = job.saved_count.max(update.saved_count);
            job.target_save_count = job.target_save_count.max(update.target_save_count);
            job.inserted_count = job.inserted_count.max(update.inserted_count);
            job.derived_count = job.derived_count.max(update.derived_count);
            job.target_derive_count = job.target_derive_count.max(update.target_derive_count);
            job.batches = job.batches.max(update.batches);
            job.target_batches = job.target_batches.max(update.target_batches);
            job.api_calls = job.api_calls.max(update.api_calls);
            if !should_persist {
                return Ok(());
            }
            let job = job.clone();
            inner
                .running_persisted_at
                .insert(task_id.to_string(), now_instant);
            job
        };
        self.persist_running_job_if_active(&persist_job).await?;
        Ok(())
    }

    pub async fn complete_with_payload(&self, task_id: &str, payload: &Value) -> Result<()> {
        let mut inner = self.inner.lock().await;
        if let Some(job) = inner.jobs.get_mut(task_id) {
            if job.status == "cancelled" {
                return Ok(());
            }
            let completion = SyncJobCompletionPayload::parse(payload, &job.timeframe)?;
            let now = now_text();
            job.status = "completed".to_string();
            job.progress = 100;
            job.message = completion.message;
            job.updated_at = now.clone();
            job.finished_at = Some(now);
            job.error.clear();
            job.fetched_count = completion.fetched_count;
            job.target_fetch_count = completion.target_fetch_count;
            job.saved_count = completion.saved_count;
            job.target_save_count = completion.target_save_count;
            job.inserted_count = completion.inserted_count;
            job.derived_count = completion.derived_count;
            job.target_derive_count = completion.target_derive_count;
            job.batches = completion.batches;
            job.target_batches = completion.target_batches;
            job.api_calls = completion.api_calls;
            job.candle_count = completion.candle_count;
            job.source_timeframe = completion.source_timeframe;
            job.target_timeframes = completion.target_timeframes;
            job.history_complete = completion.history_complete;
            job.last_sync_mode = completion.last_sync_mode;
            job.last_sync_time = completion.last_sync_time;
            job.oldest_timestamp = completion.oldest_timestamp;
            job.newest_timestamp = completion.newest_timestamp;
            job.oldest_time = completion.oldest_time;
            job.newest_time = completion.newest_time;
            job.truncated = completion.truncated;
            job.cancel_requested = false;
            self.persist_job(job).await?;
        }
        inner.remove_active_task(task_id);
        Ok(())
    }

    pub async fn fail(&self, task_id: &str, error: String) -> Result<()> {
        let mut inner = self.inner.lock().await;
        if let Some(job) = inner.jobs.get_mut(task_id) {
            if job.status == "cancelled" {
                return Ok(());
            }
            let now = now_text();
            job.status = "failed".to_string();
            job.progress = job.progress.clamp(0, 99);
            job.message = "同步失败".to_string();
            job.updated_at = now.clone();
            job.finished_at = Some(now);
            job.error = error;
            self.persist_job(job).await?;
        }
        inner.remove_active_task(task_id);
        Ok(())
    }
}
