use std::{
    collections::{BTreeSet, HashMap},
    time::Duration,
};

use anyhow::{anyhow, Result};
use tokio::time::sleep;

use super::{
    db_row::{sync_job_from_row, SYNC_JOB_SELECT},
    manager::SyncJobManager,
    timeframes::{
        normalize_required_timeframe, normalize_target_timeframes, target_timeframes_json,
    },
    types::{SyncJob, SyncJobRequest},
};

const SYNC_JOB_WAIT_POLL_MS: u64 = 250;
const SYNC_JOB_WAIT_TIMEOUT_SECS: u64 = 6 * 60 * 60;

impl SyncJobManager {
    pub async fn list(
        &self,
        active_only: bool,
        limit: usize,
        task_ids: Option<&[String]>,
    ) -> Result<Vec<SyncJob>> {
        let limit = limit.max(1);
        let mut jobs = if let Some(task_ids) = task_ids {
            self.fetch_jobs_by_ids(task_ids).await?
        } else if active_only {
            self.cached_active_jobs().await
        } else {
            self.fetch_recent_jobs(limit as i64).await?
        };

        if active_only {
            jobs.retain(SyncJob::is_active);
        }
        jobs.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| right.created_at.cmp(&left.created_at))
        });
        jobs.truncate(limit);
        Ok(jobs)
    }

    pub async fn get(&self, task_id: &str) -> Result<Option<SyncJob>> {
        self.cached_or_persisted_job(task_id).await
    }

    pub async fn wait_for_terminal(&self, task_id: &str) -> Result<SyncJob> {
        let deadline = std::time::Instant::now() + Duration::from_secs(SYNC_JOB_WAIT_TIMEOUT_SECS);
        loop {
            let Some(job) = self.cached_or_persisted_job(task_id).await? else {
                return Err(anyhow!("sync job {task_id} not found while waiting"));
            };
            if !job.is_active() {
                return Ok(job);
            }
            if std::time::Instant::now() >= deadline {
                return Err(anyhow!(
                    "sync job {task_id} did not finish within {SYNC_JOB_WAIT_TIMEOUT_SECS}s"
                ));
            }
            sleep(Duration::from_millis(SYNC_JOB_WAIT_POLL_MS)).await;
        }
    }

    async fn cached_or_persisted_job(&self, task_id: &str) -> Result<Option<SyncJob>> {
        {
            let inner = self.inner.lock().await;
            if let Some(job) = inner.jobs.get(task_id) {
                return Ok(Some(job.clone()));
            }
        }
        self.fetch_job_by_id(task_id).await
    }

    pub(in crate::sync_jobs) async fn fetch_job_by_id(
        &self,
        task_id: &str,
    ) -> Result<Option<SyncJob>> {
        let sql = format!("{SYNC_JOB_SELECT} WHERE task_id = ?");
        let row = sqlx::query(&sql)
            .bind(task_id)
            .fetch_optional(&self.db)
            .await?;
        row.map(sync_job_from_row).transpose()
    }

    async fn fetch_jobs_by_ids(&self, task_ids: &[String]) -> Result<Vec<SyncJob>> {
        if task_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut by_id = HashMap::<String, SyncJob>::with_capacity(task_ids.len());
        let mut missing_task_ids = Vec::new();
        let mut seen_missing = BTreeSet::new();
        {
            let inner = self.inner.lock().await;
            for task_id in task_ids {
                if let Some(job) = inner.jobs.get(task_id) {
                    by_id.insert(task_id.clone(), job.clone());
                } else if seen_missing.insert(task_id.clone()) {
                    missing_task_ids.push(task_id.clone());
                }
            }
        }

        for chunk in missing_task_ids.chunks(900) {
            let placeholders = vec!["?"; chunk.len()].join(", ");
            let sql = format!("{SYNC_JOB_SELECT} WHERE task_id IN ({placeholders})");
            let mut query = sqlx::query(&sql);
            for task_id in chunk {
                query = query.bind(task_id);
            }
            for row in query.fetch_all(&self.db).await? {
                let job = sync_job_from_row(row)?;
                by_id.insert(job.task_id.clone(), job);
            }
        }

        Ok(task_ids
            .iter()
            .filter_map(|task_id| by_id.get(task_id).cloned())
            .collect())
    }

    pub(in crate::sync_jobs) async fn fetch_active_job_for_request(
        &self,
        req: &SyncJobRequest,
    ) -> Result<Option<SyncJob>> {
        let timeframe = normalize_required_timeframe(&req.timeframe, "timeframe")?;
        let source_timeframe =
            normalize_required_timeframe(&req.source_timeframe, "source_timeframe")?;
        let target_timeframes =
            normalize_target_timeframes(req.target_timeframes.clone(), &timeframe)?;
        let sql = format!(
            "{SYNC_JOB_SELECT} \
             WHERE inst_id = ? AND inst_type = ? \
               AND source_timeframe = ? AND target_timeframes = ? \
               AND mode = ? AND days = ? \
               AND ((start_ts IS NULL AND ? IS NULL) OR start_ts = ?) \
               AND ((end_ts IS NULL AND ? IS NULL) OR end_ts = ?) \
               AND repair_method = ? \
               AND status IN ('queued', 'running') \
             ORDER BY created_at DESC, task_id DESC \
             LIMIT 1"
        );
        let row = sqlx::query(&sql)
            .bind(req.inst_id.trim().to_uppercase())
            .bind(req.inst_type.trim().to_uppercase())
            .bind(source_timeframe)
            .bind(target_timeframes_json(&target_timeframes)?)
            .bind(&req.mode)
            .bind(req.days.max(1))
            .bind(req.start_ts)
            .bind(req.start_ts)
            .bind(req.end_ts)
            .bind(req.end_ts)
            .bind(req.repair_method.trim().to_ascii_lowercase())
            .fetch_optional(&self.db)
            .await?;
        row.map(sync_job_from_row).transpose()
    }

    async fn cached_active_jobs(&self) -> Vec<SyncJob> {
        let inner = self.inner.lock().await;
        let mut seen = BTreeSet::new();
        let mut jobs = Vec::with_capacity(inner.active_keys.len());
        for task_id in inner.active_keys.values() {
            if !seen.insert(task_id.clone()) {
                continue;
            }
            if let Some(job) = inner.jobs.get(task_id).filter(|job| job.is_active()) {
                jobs.push(job.clone());
            }
        }
        jobs
    }

    pub(in crate::sync_jobs) async fn fetch_active_jobs(&self) -> Result<Vec<SyncJob>> {
        let sql = format!(
            "{SYNC_JOB_SELECT} \
             WHERE status IN ('queued', 'running') \
             ORDER BY updated_at DESC, created_at DESC"
        );
        let rows = sqlx::query(&sql).fetch_all(&self.db).await?;
        rows.into_iter().map(sync_job_from_row).collect()
    }

    async fn fetch_recent_jobs(&self, limit: i64) -> Result<Vec<SyncJob>> {
        let sql = format!("{SYNC_JOB_SELECT} ORDER BY updated_at DESC, created_at DESC LIMIT ?");
        let rows = sqlx::query(&sql).bind(limit).fetch_all(&self.db).await?;
        rows.into_iter().map(sync_job_from_row).collect()
    }
}
