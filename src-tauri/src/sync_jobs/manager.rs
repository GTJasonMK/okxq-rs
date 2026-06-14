use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use sqlx::SqlitePool;
use tokio::sync::Mutex;

use super::{
    db_row::job_key,
    types::{SyncJob, SyncJobRequest},
};

#[derive(Clone)]
pub struct SyncJobManager {
    pub(in crate::sync_jobs) db: SqlitePool,
    pub(in crate::sync_jobs) inner: Arc<Mutex<SyncJobStore>>,
}

#[derive(Default)]
pub(in crate::sync_jobs) struct SyncJobStore {
    pub(in crate::sync_jobs) jobs: BTreeMap<String, SyncJob>,
    pub(in crate::sync_jobs) active_keys: BTreeMap<String, String>,
    pub(in crate::sync_jobs) active_coverage_keys: BTreeMap<String, BTreeSet<String>>,
    pub(in crate::sync_jobs) running_persisted_at: BTreeMap<String, Instant>,
}

pub(in crate::sync_jobs) const RUNNING_PERSIST_MIN_INTERVAL: Duration = Duration::from_millis(750);

impl SyncJobManager {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            db,
            inner: Arc::new(Mutex::new(SyncJobStore::default())),
        }
    }
}

impl SyncJobStore {
    pub(in crate::sync_jobs) fn track_active_job(&mut self, job: &SyncJob) -> Result<()> {
        if !job.is_active() {
            self.remove_active_task(&job.task_id);
            return Ok(());
        }
        self.active_keys.insert(job_key(job)?, job.task_id.clone());
        self.active_coverage_keys
            .entry(job.coverage_key()?)
            .or_default()
            .insert(job.task_id.clone());
        Ok(())
    }

    pub(in crate::sync_jobs) fn covering_active_job(
        &mut self,
        req: &SyncJobRequest,
        coverage_key: &str,
    ) -> Result<Option<SyncJob>> {
        let Some(candidate_ids) = self.active_coverage_keys.get(coverage_key) else {
            return Ok(None);
        };
        let candidate_ids = candidate_ids.iter().cloned().collect::<Vec<_>>();
        let mut stale_task_ids = Vec::new();
        let mut covered_job = None;
        for task_id in candidate_ids {
            let Some(job) = self.jobs.get(&task_id) else {
                stale_task_ids.push(task_id);
                continue;
            };
            if !job.is_active() {
                stale_task_ids.push(task_id);
                continue;
            }
            if req.is_covered_by(job)? {
                covered_job = Some(job.clone());
                break;
            }
        }
        for task_id in stale_task_ids {
            self.remove_active_task(&task_id);
        }
        Ok(covered_job)
    }

    pub(in crate::sync_jobs) fn running_update_persist_due(
        &self,
        task_id: &str,
        now: Instant,
    ) -> bool {
        self.running_persisted_at
            .get(task_id)
            .map(|last| now.duration_since(*last) >= RUNNING_PERSIST_MIN_INTERVAL)
            .unwrap_or(true)
    }

    pub(in crate::sync_jobs) fn remove_active_task(&mut self, task_id: &str) {
        self.active_keys
            .retain(|_, active_task_id| active_task_id != task_id);
        self.active_coverage_keys.retain(|_, active_task_ids| {
            active_task_ids.remove(task_id);
            !active_task_ids.is_empty()
        });
        self.running_persisted_at.remove(task_id);
    }
}
