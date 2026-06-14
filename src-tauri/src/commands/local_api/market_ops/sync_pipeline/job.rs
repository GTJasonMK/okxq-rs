use std::{fmt::Display, future::Future};

use serde_json::Value;
use sqlx::SqlitePool;

use crate::{
    app_state::AppState,
    error::AppResult,
    okx::OkxPublicClient,
    sync_jobs::{SyncJob, SyncJobRequest},
};

use super::super::*;
use super::{client::okx_public_client, request::run_sync_request};

#[derive(Clone, Copy)]
pub(in crate::commands::local_api::market_ops) enum BackgroundSyncJobKind {
    Sync,
    GapRepair,
}

pub(in crate::commands::local_api::market_ops) async fn enqueue_sync_job(
    state: &AppState,
    request: SyncJobRequest,
) -> AppResult<(SyncJob, bool)> {
    let run_request = request.clone();
    enqueue_background_sync_job(
        state,
        request,
        BackgroundSyncJobKind::Sync,
        move |pool, client, task_id, cancel_guard, progress| async move {
            run_sync_request(
                pool,
                client,
                run_request,
                Some(task_id),
                Some(cancel_guard),
                progress,
            )
            .await
        },
    )
    .await
}

pub(in crate::commands::local_api::market_ops) async fn enqueue_background_sync_job<F, Fut>(
    state: &AppState,
    request: SyncJobRequest,
    kind: BackgroundSyncJobKind,
    run: F,
) -> AppResult<(SyncJob, bool)>
where
    F: FnOnce(SqlitePool, OkxPublicClient, String, SyncCancelGuard, SyncProgressReporter) -> Fut
        + Send
        + 'static,
    Fut: Future<Output = AppResult<Value>> + Send + 'static,
{
    let (job, reused) = state.sync_jobs.create_or_reuse(request.clone()).await?;
    if reused {
        return Ok((job, true));
    }

    let manager = state.sync_jobs.clone();
    let pool = state.db.clone();
    let task_id = job.task_id.clone();
    let request_meta = request.clone();
    let client = match okx_public_client(state).await {
        Ok(client) => client,
        Err(error) => {
            kind.log_client_error(&task_id, &request_meta, &error);
            if let Err(persist_error) = manager.fail(&task_id, error.to_string()).await {
                kind.log_persist_fail_error(&task_id, &persist_error);
            }
            return Err(error);
        }
    };
    tauri::async_runtime::spawn(async move {
        let limit = sync_runtime_settings().sync_job_concurrency;
        let permit = sync_job_limiter().acquire(limit).await;
        if let Err(error) = manager.mark_running(&task_id).await {
            kind.log_mark_running_error(&task_id, &error);
        }
        kind.log_started(&task_id, &request_meta);
        let cancel_guard = SyncCancelGuard {
            manager: manager.clone(),
            task_id: task_id.clone(),
        };
        let progress = SyncProgressReporter {
            manager: Some(manager.clone()),
            task_id: Some(task_id.clone()),
        };
        match run(pool, client, task_id.clone(), cancel_guard, progress).await {
            Ok(payload) => {
                kind.log_completed(&task_id, &request_meta);
                if let Err(error) = manager.complete_with_payload(&task_id, &payload).await {
                    kind.log_complete_error(&task_id, &error);
                }
            }
            Err(error) => match manager.is_cancel_requested(&task_id).await {
                Ok(true) => {
                    kind.log_cancelled(&task_id, &request_meta);
                }
                Ok(false) => {
                    kind.log_failed(&task_id, &request_meta, &error);
                    if let Err(persist_error) = manager.fail(&task_id, error.to_string()).await {
                        kind.log_persist_fail_error(&task_id, &persist_error);
                    }
                }
                Err(cancel_error) => {
                    let detail = format!("{error}; 取消状态检查失败: {cancel_error}");
                    kind.log_cancel_check_failed(&task_id, &request_meta, &error, &cancel_error);
                    if let Err(persist_error) = manager.fail(&task_id, detail).await {
                        kind.log_persist_fail_after_cancel_check_error(&task_id, &persist_error);
                    }
                }
            },
        }
        drop(permit);
    });

    Ok((job, false))
}

impl BackgroundSyncJobKind {
    fn log_client_error(&self, task_id: &str, request: &SyncJobRequest, error: &dyn Display) {
        match self {
            Self::Sync => {
                tracing::error!(
                    task_id = %task_id,
                    inst_id = %request.inst_id,
                    inst_type = %request.inst_type,
                    timeframe = %request.timeframe,
                    mode = %request.mode,
                    error = %error,
                    "failed to create OKX public client for sync job"
                );
            }
            Self::GapRepair => {}
        }
    }

    fn log_mark_running_error(&self, task_id: &str, error: &dyn Display) {
        match self {
            Self::Sync => {
                tracing::warn!(task_id = %task_id, error = %error, "failed to mark sync job running");
            }
            Self::GapRepair => {
                tracing::warn!(task_id = %task_id, error = %error, "failed to mark gap repair job running");
            }
        }
    }

    fn log_started(&self, task_id: &str, request: &SyncJobRequest) {
        match self {
            Self::Sync => {
                tracing::info!(
                    task_id = %task_id,
                    inst_id = %request.inst_id,
                    inst_type = %request.inst_type,
                    timeframe = %request.timeframe,
                    mode = %request.mode,
                    days = request.days,
                    "sync job started"
                );
            }
            Self::GapRepair => {}
        }
    }

    fn log_completed(&self, task_id: &str, request: &SyncJobRequest) {
        match self {
            Self::Sync => {
                tracing::info!(
                    task_id = %task_id,
                    inst_id = %request.inst_id,
                    inst_type = %request.inst_type,
                    timeframe = %request.timeframe,
                    mode = %request.mode,
                    "sync job completed"
                );
            }
            Self::GapRepair => {
                tracing::info!(
                    task_id = %task_id,
                    inst_id = %request.inst_id,
                    inst_type = %request.inst_type,
                    timeframe = %request.timeframe,
                    "gap repair job completed"
                );
            }
        }
    }

    fn log_complete_error(&self, task_id: &str, error: &dyn Display) {
        match self {
            Self::Sync => {
                tracing::warn!(task_id = %task_id, error = %error, "failed to complete sync job");
            }
            Self::GapRepair => {
                tracing::warn!(task_id = %task_id, error = %error, "failed to complete gap repair job");
            }
        }
    }

    fn log_cancelled(&self, task_id: &str, request: &SyncJobRequest) {
        match self {
            Self::Sync => {
                tracing::info!(
                    task_id = %task_id,
                    inst_id = %request.inst_id,
                    inst_type = %request.inst_type,
                    timeframe = %request.timeframe,
                    mode = %request.mode,
                    "sync job cancelled"
                );
            }
            Self::GapRepair => {
                tracing::info!(task_id = %task_id, "gap repair job cancelled");
            }
        }
    }

    fn log_failed(&self, task_id: &str, request: &SyncJobRequest, error: &dyn Display) {
        match self {
            Self::Sync => {
                tracing::error!(
                    task_id = %task_id,
                    inst_id = %request.inst_id,
                    inst_type = %request.inst_type,
                    timeframe = %request.timeframe,
                    mode = %request.mode,
                    days = request.days,
                    error = %error,
                    "sync job failed"
                );
            }
            Self::GapRepair => {
                tracing::error!(
                    task_id = %task_id,
                    inst_id = %request.inst_id,
                    inst_type = %request.inst_type,
                    timeframe = %request.timeframe,
                    error = %error,
                    "gap repair job failed"
                );
            }
        }
    }

    fn log_cancel_check_failed(
        &self,
        task_id: &str,
        request: &SyncJobRequest,
        error: &dyn Display,
        cancel_error: &dyn Display,
    ) {
        match self {
            Self::Sync => {
                tracing::error!(
                    task_id = %task_id,
                    inst_id = %request.inst_id,
                    inst_type = %request.inst_type,
                    timeframe = %request.timeframe,
                    mode = %request.mode,
                    days = request.days,
                    error = %error,
                    cancel_error = %cancel_error,
                    "sync job failed and cancel check failed"
                );
            }
            Self::GapRepair => {
                tracing::error!(
                    task_id = %task_id,
                    inst_id = %request.inst_id,
                    inst_type = %request.inst_type,
                    timeframe = %request.timeframe,
                    error = %error,
                    cancel_error = %cancel_error,
                    "gap repair job failed and cancel check failed"
                );
            }
        }
    }

    fn log_persist_fail_error(&self, task_id: &str, error: &dyn Display) {
        match self {
            Self::Sync => {
                tracing::warn!(task_id = %task_id, error = %error, "failed to mark sync job failed");
            }
            Self::GapRepair => {
                tracing::warn!(task_id = %task_id, error = %error, "failed to mark gap repair job failed");
            }
        }
    }

    fn log_persist_fail_after_cancel_check_error(&self, task_id: &str, error: &dyn Display) {
        match self {
            Self::Sync => {
                tracing::warn!(task_id = %task_id, error = %error, "failed to mark sync job failed after cancel check error");
            }
            Self::GapRepair => {
                tracing::warn!(task_id = %task_id, error = %error, "failed to mark gap repair job failed after cancel check error");
            }
        }
    }
}

pub(in crate::commands::local_api::market_ops) async fn run_sync_request_guarded(
    state: &AppState,
    request: SyncJobRequest,
) -> AppResult<Value> {
    let (job, reused) = state.sync_jobs.create_or_reuse(request.clone()).await?;
    if reused {
        tracing::info!(
            task_id = %job.task_id,
            inst_id = %job.inst_id,
            inst_type = %job.inst_type,
            timeframe = %job.timeframe,
            mode = %job.mode,
            days = job.days,
            "guarded sync request reused active sync job"
        );
        let completed = state.sync_jobs.wait_for_terminal(&job.task_id).await?;
        return terminal_sync_job_payload(completed, true);
    }

    let manager = state.sync_jobs.clone();
    let pool = state.db.clone();
    let task_id = job.task_id.clone();
    let request_meta = request.clone();
    let client = match okx_public_client(state).await {
        Ok(client) => client,
        Err(error) => {
            if let Err(persist_error) = manager.fail(&task_id, error.to_string()).await {
                tracing::warn!(task_id = %task_id, error = %persist_error, "failed to mark guarded sync job failed");
            }
            return Err(error);
        }
    };

    let limit = sync_runtime_settings().sync_job_concurrency;
    let _permit = sync_job_limiter().acquire(limit).await;
    if let Err(error) = manager.mark_running(&task_id).await {
        if let Err(persist_error) = manager.fail(&task_id, error.to_string()).await {
            tracing::warn!(task_id = %task_id, error = %persist_error, "failed to mark guarded sync job failed after running mark error");
        }
        return Err(error.into());
    }
    tracing::info!(
        task_id = %task_id,
        inst_id = %request_meta.inst_id,
        inst_type = %request_meta.inst_type,
        timeframe = %request_meta.timeframe,
        mode = %request_meta.mode,
        days = request_meta.days,
        "guarded sync request started under sync job guard"
    );
    let cancel_guard = SyncCancelGuard {
        manager: manager.clone(),
        task_id: task_id.clone(),
    };
    match run_sync_request(
        pool,
        client,
        request,
        Some(task_id.clone()),
        Some(cancel_guard),
        SyncProgressReporter {
            manager: Some(manager.clone()),
            task_id: Some(task_id.clone()),
        },
    )
    .await
    {
        Ok(payload) => {
            if let Err(error) = manager.complete_with_payload(&task_id, &payload).await {
                tracing::warn!(task_id = %task_id, error = %error, "failed to complete guarded sync job");
            }
            Ok(payload)
        }
        Err(error) => {
            match manager.is_cancel_requested(&task_id).await {
                Ok(true) => {}
                Ok(false) => {
                    if let Err(persist_error) = manager.fail(&task_id, error.to_string()).await {
                        tracing::warn!(task_id = %task_id, error = %persist_error, "failed to mark guarded sync job failed");
                    }
                }
                Err(cancel_error) => {
                    let detail = format!("{error}; 取消状态检查失败: {cancel_error}");
                    tracing::warn!(
                        task_id = %task_id,
                        error = %error,
                        cancel_error = %cancel_error,
                        "guarded sync job failed and cancel check failed"
                    );
                    if let Err(persist_error) = manager.fail(&task_id, detail).await {
                        tracing::warn!(task_id = %task_id, error = %persist_error, "failed to mark guarded sync job failed after cancel check error");
                    }
                }
            }
            Err(error)
        }
    }
}
