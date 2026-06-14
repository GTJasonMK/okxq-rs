use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::timeframes::{
    normalize_required_timeframe, normalize_target_timeframes, target_timeframes_json,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncJobRequest {
    pub inst_id: String,
    pub inst_type: String,
    pub timeframe: String,
    pub source_timeframe: String,
    pub target_timeframes: Vec<String>,
    pub mode: String,
    pub days: i64,
    pub start_ts: Option<i64>,
    pub end_ts: Option<i64>,
    pub repair_method: String,
    pub target_fetch_count: i64,
    pub target_save_count: i64,
    pub target_derive_count: i64,
    pub target_batches: i64,
}

#[derive(Clone, Debug, Default)]
pub struct SyncJobRunningUpdate {
    pub progress: i64,
    pub message: String,
    pub fetched_count: i64,
    pub target_fetch_count: i64,
    pub saved_count: i64,
    pub target_save_count: i64,
    pub inserted_count: i64,
    pub derived_count: i64,
    pub target_derive_count: i64,
    pub batches: i64,
    pub target_batches: i64,
    pub api_calls: i64,
}

impl SyncJobRequest {
    pub fn key(&self) -> Result<String> {
        let timeframe = normalize_required_timeframe(&self.timeframe, "timeframe")?;
        let source_timeframe =
            normalize_required_timeframe(&self.source_timeframe, "source_timeframe")?;
        let target_timeframes =
            normalize_target_timeframes(self.target_timeframes.clone(), &timeframe)?;
        Ok(format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}",
            self.inst_id.trim().to_uppercase(),
            self.inst_type.trim().to_uppercase(),
            source_timeframe,
            target_timeframes_json(&target_timeframes)?,
            self.mode,
            self.days.max(1),
            sync_job_ts_key(self.start_ts),
            sync_job_ts_key(self.end_ts),
            self.repair_method.trim().to_ascii_lowercase()
        ))
    }

    pub(in crate::sync_jobs) fn coverage_key(&self) -> Result<String> {
        sync_job_coverage_key(
            &self.inst_id,
            &self.inst_type,
            &self.source_timeframe,
            &self.mode,
            self.start_ts,
            self.end_ts,
            &self.repair_method,
        )
    }

    pub fn is_covered_by(&self, job: &SyncJob) -> Result<bool> {
        if self.inst_id.trim().to_uppercase() != job.inst_id.trim().to_uppercase()
            || self.inst_type.trim().to_uppercase() != job.inst_type.trim().to_uppercase()
            || !job.is_active()
        {
            return Ok(false);
        }

        let request_timeframe = normalize_required_timeframe(&self.timeframe, "timeframe")?;
        let request_source =
            normalize_required_timeframe(&self.source_timeframe, "source_timeframe")?;
        let request_targets =
            normalize_target_timeframes(self.target_timeframes.clone(), &request_timeframe)?;
        let job_timeframe = normalize_required_timeframe(&job.timeframe, "timeframe")?;
        let job_source = normalize_required_timeframe(&job.source_timeframe, "source_timeframe")?;
        let job_targets =
            normalize_target_timeframes(job.target_timeframes.clone(), &job_timeframe)?;

        Ok(request_source == job_source
            && self.mode == job.mode
            && self.start_ts == job.start_ts
            && self.end_ts == job.end_ts
            && self
                .repair_method
                .trim()
                .eq_ignore_ascii_case(&job.repair_method)
            && job.days.max(1) >= self.days.max(1)
            && request_targets
                .iter()
                .all(|timeframe| job_targets.contains(timeframe)))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncJob {
    pub task_id: String,
    pub inst_id: String,
    pub inst_type: String,
    pub timeframe: String,
    pub source_timeframe: String,
    pub target_timeframes: Vec<String>,
    pub mode: String,
    pub days: i64,
    pub start_ts: Option<i64>,
    pub end_ts: Option<i64>,
    pub repair_method: String,
    pub status: String,
    pub progress: i64,
    pub message: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub updated_at: String,
    pub finished_at: Option<String>,
    pub error: String,
    pub fetched_count: i64,
    pub target_fetch_count: i64,
    pub saved_count: i64,
    pub target_save_count: i64,
    pub inserted_count: i64,
    pub derived_count: i64,
    pub target_derive_count: i64,
    pub batches: i64,
    pub target_batches: i64,
    pub api_calls: i64,
    pub candle_count: i64,
    pub history_complete: bool,
    pub last_sync_mode: String,
    pub last_sync_time: Option<String>,
    pub oldest_timestamp: Option<i64>,
    pub newest_timestamp: Option<i64>,
    pub oldest_time: Option<String>,
    pub newest_time: Option<String>,
    pub reused_existing: bool,
    pub truncated: bool,
    pub cancel_requested: bool,
}

impl SyncJob {
    pub fn to_value(&self) -> Result<Value> {
        serde_json::to_value(self).map_err(Into::into)
    }

    pub fn is_active(&self) -> bool {
        matches!(self.status.as_str(), "queued" | "running")
    }

    pub(in crate::sync_jobs) fn coverage_key(&self) -> Result<String> {
        sync_job_coverage_key(
            &self.inst_id,
            &self.inst_type,
            &self.source_timeframe,
            &self.mode,
            self.start_ts,
            self.end_ts,
            &self.repair_method,
        )
    }
}

fn sync_job_coverage_key(
    inst_id: &str,
    inst_type: &str,
    source_timeframe: &str,
    mode: &str,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    repair_method: &str,
) -> Result<String> {
    let source_timeframe = normalize_required_timeframe(source_timeframe, "source_timeframe")?;
    Ok(format!(
        "{}:{}:{}:{}:{}:{}:{}",
        inst_id.trim().to_uppercase(),
        inst_type.trim().to_uppercase(),
        source_timeframe,
        mode,
        sync_job_ts_key(start_ts),
        sync_job_ts_key(end_ts),
        repair_method.trim().to_ascii_lowercase()
    ))
}

pub(in crate::sync_jobs) fn sync_job_ts_key(value: Option<i64>) -> String {
    value.map_or_else(|| "null".to_string(), |timestamp| timestamp.to_string())
}
