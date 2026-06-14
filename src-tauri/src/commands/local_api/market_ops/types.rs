use serde_json::{json, Value};

use crate::{
    error::{AppError, AppResult},
    sync_jobs::SyncJob,
};

#[derive(Default)]
pub(in crate::commands::local_api::market_ops) struct DeriveResult {
    pub(in crate::commands::local_api::market_ops) saved_count: i64,
    pub(in crate::commands::local_api::market_ops) target_count: i64,
    pub(in crate::commands::local_api::market_ops) derived_timeframes: Vec<String>,
}

#[derive(Debug)]
pub(in crate::commands::local_api::market_ops) struct SyncFetchResult {
    pub(in crate::commands::local_api::market_ops) fetched_count: i64,
    pub(in crate::commands::local_api::market_ops) target_fetch_count: i64,
    pub(in crate::commands::local_api::market_ops) saved_count: i64,
    pub(in crate::commands::local_api::market_ops) target_save_count: i64,
    pub(in crate::commands::local_api::market_ops) batches: i64,
    pub(in crate::commands::local_api::market_ops) target_batches: i64,
    pub(in crate::commands::local_api::market_ops) api_calls: i64,
    pub(in crate::commands::local_api::market_ops) history_complete: bool,
    pub(in crate::commands::local_api::market_ops) truncated: bool,
    pub(in crate::commands::local_api::market_ops) message: String,
}

#[derive(Clone, Debug)]
pub(in crate::commands::local_api::market_ops) struct SyncRecordStats {
    pub(in crate::commands::local_api::market_ops) last_sync_time: Option<String>,
    pub(in crate::commands::local_api::market_ops) oldest_timestamp: Option<i64>,
    pub(in crate::commands::local_api::market_ops) newest_timestamp: Option<i64>,
    pub(in crate::commands::local_api::market_ops) candle_count: i64,
    pub(in crate::commands::local_api::market_ops) history_complete: bool,
    pub(in crate::commands::local_api::market_ops) last_sync_mode: String,
}

pub(in crate::commands::local_api::market_ops) fn submitted_sync_job_value(
    job: &SyncJob,
    reused_existing: bool,
) -> Value {
    json!({
        "task_id": job.task_id,
        "inst_id": job.inst_id,
        "inst_type": job.inst_type,
        "timeframe": job.timeframe,
        "source_timeframe": job.source_timeframe,
        "target_timeframes": job.target_timeframes,
        "mode": job.mode,
        "days": job.days,
        "start_ts": job.start_ts,
        "end_ts": job.end_ts,
        "repair_method": job.repair_method,
        "status": job.status,
        "progress": job.progress,
        "message": job.message,
        "created_at": job.created_at,
        "updated_at": job.updated_at,
        "finished_at": job.finished_at,
        "error": job.error,
        "fetched_count": job.fetched_count,
        "target_fetch_count": job.target_fetch_count,
        "saved_count": job.saved_count,
        "target_save_count": job.target_save_count,
        "derived_count": job.derived_count,
        "target_derive_count": job.target_derive_count,
        "batches": job.batches,
        "target_batches": job.target_batches,
        "api_calls": job.api_calls,
        "reused_existing": reused_existing || job.reused_existing,
    })
}

pub(in crate::commands::local_api::market_ops) fn terminal_sync_job_payload(
    mut job: SyncJob,
    reused_existing: bool,
) -> AppResult<Value> {
    match job.status.as_str() {
        "completed" => {
            job.reused_existing = reused_existing || job.reused_existing;
            Ok(job.to_value()?)
        }
        "cancelled" => Err(AppError::Runtime(format!(
            "同步任务 {} 已取消: {}",
            job.task_id, job.message
        ))),
        "failed" => {
            let detail = if job.error.trim().is_empty() {
                job.message
            } else {
                job.error
            };
            Err(AppError::Runtime(format!(
                "同步任务 {} 失败: {detail}",
                job.task_id
            )))
        }
        status => Err(AppError::Runtime(format!(
            "同步任务 {} 未完成，当前状态: {status}",
            job.task_id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submitted_sync_job_payload_preserves_observe_fields() {
        let job = sync_job(0);

        let value = submitted_sync_job_value(&job, true);

        assert_eq!(value["task_id"], job.task_id);
        assert_eq!(value["inst_id"], job.inst_id);
        assert_eq!(value["target_timeframes"], json!(job.target_timeframes));
        assert_eq!(value["status"], job.status);
        assert_eq!(value["progress"], job.progress);
        assert_eq!(value["target_fetch_count"], job.target_fetch_count);
        assert_eq!(value["reused_existing"], true);
        assert!(value.get("oldest_timestamp").is_none());
        assert!(value.get("newest_timestamp").is_none());
        assert!(value.get("last_sync_time").is_none());
        assert!(value.get("truncated").is_none());
    }

    #[test]
    fn submitted_sync_job_payload_omits_persistent_inventory_fields() {
        let job = sync_job(1);

        let value = submitted_sync_job_value(&job, job.reused_existing);

        for field in [
            "started_at",
            "inserted_count",
            "candle_count",
            "history_complete",
            "last_sync_mode",
            "last_sync_time",
            "oldest_timestamp",
            "newest_timestamp",
            "oldest_time",
            "newest_time",
            "truncated",
            "cancel_requested",
        ] {
            assert!(
                value.get(field).is_none(),
                "{field} should stay out of submitted payload"
            );
        }
        assert_eq!(value["target_save_count"], job.target_save_count);
        assert_eq!(value["target_derive_count"], job.target_derive_count);
    }

    #[test]
    fn terminal_sync_job_payload_marks_reused_completed_job() {
        let mut job = sync_job(1);
        job.status = "completed".to_string();

        let value = terminal_sync_job_payload(job, true).expect("completed job payload");

        assert_eq!(value["status"], "completed");
        assert_eq!(value["reused_existing"], true);
    }

    #[test]
    fn terminal_sync_job_payload_reports_failed_job_detail() {
        let mut job = sync_job(2);
        job.status = "failed".to_string();
        job.message = "fallback message".to_string();
        job.error = String::new();

        let error = terminal_sync_job_payload(job, false)
            .expect_err("failed job should return error")
            .to_string();

        assert!(error.contains("sync_job_00002"));
        assert!(error.contains("fallback message"));
    }

    fn sync_job(index: usize) -> SyncJob {
        SyncJob {
            task_id: format!("sync_job_{index:05}"),
            inst_id: format!("PERF-{index:05}-USDT-SWAP"),
            inst_type: "SWAP".to_string(),
            timeframe: "1H".to_string(),
            source_timeframe: "1m".to_string(),
            target_timeframes: vec!["1H".to_string(), "4H".to_string()],
            mode: "gap_repair".to_string(),
            days: 30,
            start_ts: Some(1_770_000_000_000),
            end_ts: Some(1_780_000_000_000),
            repair_method: "auto".to_string(),
            status: "queued".to_string(),
            progress: 0,
            message: "等待执行".to_string(),
            created_at: "2026-05-01T00:00:00.000000000+00:00".to_string(),
            started_at: None,
            updated_at: "2026-05-01T00:00:00.000000000+00:00".to_string(),
            finished_at: None,
            error: String::new(),
            fetched_count: 0,
            target_fetch_count: 1_000,
            saved_count: 0,
            target_save_count: 1_000,
            inserted_count: 0,
            derived_count: 0,
            target_derive_count: 0,
            batches: 0,
            target_batches: 10,
            api_calls: 0,
            candle_count: 0,
            history_complete: false,
            last_sync_mode: "window".to_string(),
            last_sync_time: None,
            oldest_timestamp: None,
            newest_timestamp: None,
            oldest_time: None,
            newest_time: None,
            reused_existing: index % 3 == 0,
            truncated: false,
            cancel_requested: false,
        }
    }
}
