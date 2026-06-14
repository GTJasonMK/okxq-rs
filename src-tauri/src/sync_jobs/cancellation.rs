use std::collections::BTreeMap;

use anyhow::{anyhow, Result};

use super::{manager::SyncJobManager, types::SyncJob, utils::now_text};

impl SyncJobManager {
    pub async fn cancel_job(&self, task_id: &str, reason: &str) -> Result<Option<SyncJob>> {
        let task_ids = [task_id.to_string()];
        let mut jobs = self
            .cancel_jobs(None, Some(task_ids.as_slice()), reason)
            .await?;
        if let Some(job) = jobs.pop() {
            return Ok(Some(job));
        }
        self.get(task_id).await
    }

    pub async fn cancel_jobs(
        &self,
        inst_ids: Option<&[String]>,
        task_ids: Option<&[String]>,
        reason: &str,
    ) -> Result<Vec<SyncJob>> {
        let active_jobs = self.fetch_active_jobs().await?;
        let mut active_by_id = active_jobs
            .into_iter()
            .map(|job| (job.task_id.clone(), job))
            .collect::<BTreeMap<_, _>>();
        let mut inner = self.inner.lock().await;
        for job in inner.jobs.values().filter(|job| job.is_active()) {
            active_by_id.insert(job.task_id.clone(), job.clone());
        }

        let inst_id_filter = inst_ids
            .unwrap_or(&[])
            .iter()
            .filter(|item| !item.trim().is_empty())
            .map(|item| item.trim().to_uppercase())
            .collect::<Vec<_>>();
        let task_id_filter = task_ids
            .unwrap_or(&[])
            .iter()
            .filter(|item| !item.trim().is_empty())
            .cloned()
            .collect::<Vec<_>>();

        let mut cancelled_jobs = Vec::new();
        for mut job in active_by_id.into_values() {
            if !task_id_filter.is_empty() && !task_id_filter.iter().any(|item| item == &job.task_id)
            {
                continue;
            }
            if !inst_id_filter.is_empty()
                && !inst_id_filter
                    .iter()
                    .any(|item| item == &job.inst_id.to_uppercase())
            {
                continue;
            }
            let now = now_text();
            job.cancel_requested = true;
            job.status = "cancelled".to_string();
            job.progress = 100;
            job.message = reason.to_string();
            job.updated_at = now.clone();
            job.finished_at = job.finished_at.or(Some(now));
            job.error.clear();
            inner.remove_active_task(&job.task_id);
            inner.jobs.insert(job.task_id.clone(), job.clone());
            self.persist_job(&job).await?;
            cancelled_jobs.push(job);
        }

        cancelled_jobs.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.task_id.cmp(&left.task_id))
        });
        Ok(cancelled_jobs)
    }

    pub async fn is_cancel_requested(&self, task_id: &str) -> Result<bool> {
        {
            let inner = self.inner.lock().await;
            if let Some(job) = inner.jobs.get(task_id) {
                return Ok(job.cancel_requested || job.status == "cancelled");
            }
        }
        let Some(job) = self.fetch_job_by_id(task_id).await? else {
            return Err(anyhow!(
                "sync job {task_id} not found while checking cancellation state"
            ));
        };
        Ok(job.cancel_requested || job.status == "cancelled")
    }
}
