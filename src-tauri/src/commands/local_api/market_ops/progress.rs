use crate::sync_jobs::{SyncJobManager, SyncJobRunningUpdate};

use super::*;

#[derive(Clone, Default)]
pub(in crate::commands::local_api::market_ops) struct SyncProgressReporter {
    pub(in crate::commands::local_api::market_ops) manager: Option<SyncJobManager>,
    pub(in crate::commands::local_api::market_ops) task_id: Option<String>,
}

#[derive(Clone, Default)]
pub(in crate::commands::local_api::market_ops) struct SyncProgressUpdate {
    pub(in crate::commands::local_api::market_ops) progress: i64,
    pub(in crate::commands::local_api::market_ops) message: String,
    pub(in crate::commands::local_api::market_ops) fetched_count: i64,
    pub(in crate::commands::local_api::market_ops) target_fetch_count: i64,
    pub(in crate::commands::local_api::market_ops) saved_count: i64,
    pub(in crate::commands::local_api::market_ops) target_save_count: i64,
    pub(in crate::commands::local_api::market_ops) inserted_count: i64,
    pub(in crate::commands::local_api::market_ops) derived_count: i64,
    pub(in crate::commands::local_api::market_ops) target_derive_count: i64,
    pub(in crate::commands::local_api::market_ops) batches: i64,
    pub(in crate::commands::local_api::market_ops) target_batches: i64,
    pub(in crate::commands::local_api::market_ops) api_calls: i64,
}

impl From<SyncProgressUpdate> for SyncJobRunningUpdate {
    fn from(update: SyncProgressUpdate) -> Self {
        Self {
            progress: update.progress,
            message: update.message,
            fetched_count: update.fetched_count,
            target_fetch_count: update.target_fetch_count,
            saved_count: update.saved_count,
            target_save_count: update.target_save_count,
            inserted_count: update.inserted_count,
            derived_count: update.derived_count,
            target_derive_count: update.target_derive_count,
            batches: update.batches,
            target_batches: update.target_batches,
            api_calls: update.api_calls,
        }
    }
}

impl SyncProgressReporter {
    pub(in crate::commands::local_api::market_ops) async fn report(
        &self,
        update: SyncProgressUpdate,
    ) -> AppResult<()> {
        let Some(manager) = self.manager.as_ref() else {
            return Ok(());
        };
        let Some(task_id) = self.task_id.as_ref() else {
            return Ok(());
        };
        tracing::debug!(
            task_id = %task_id,
            progress = update.progress,
            fetched_count = update.fetched_count,
            target_fetch_count = update.target_fetch_count,
            saved_count = update.saved_count,
            target_save_count = update.target_save_count,
            inserted_count = update.inserted_count,
            derived_count = update.derived_count,
            target_derive_count = update.target_derive_count,
            batches = update.batches,
            target_batches = update.target_batches,
            api_calls = update.api_calls,
            message = %update.message,
            "sync job progress"
        );
        manager.update_running(task_id, update.into()).await?;
        Ok(())
    }
}

#[derive(Clone)]
pub(in crate::commands::local_api::market_ops) struct SyncCancelGuard {
    pub(in crate::commands::local_api::market_ops) manager: SyncJobManager,
    pub(in crate::commands::local_api::market_ops) task_id: String,
}

impl SyncCancelGuard {
    async fn check(&self) -> AppResult<()> {
        if self.manager.is_cancel_requested(&self.task_id).await? {
            return Err(AppError::Runtime("同步任务已取消".to_string()));
        }
        Ok(())
    }
}

pub(in crate::commands::local_api::market_ops) async fn check_sync_cancel(
    cancel_guard: Option<&SyncCancelGuard>,
) -> AppResult<()> {
    if let Some(guard) = cancel_guard {
        guard.check().await?;
    }
    Ok(())
}
