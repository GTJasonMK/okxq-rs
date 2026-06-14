use serde_json::{json, Value};

use crate::sync_jobs::SyncJobRequest;

use super::super::super::*;
use super::plan::SyncRequestPlan;

pub(super) struct SyncCompletionTimeline<'a> {
    pub(super) created_at: &'a str,
    pub(super) started_at: &'a str,
    pub(super) finished_at: &'a str,
}

pub(super) struct SyncCompletionPayloadInput<'a> {
    pub(super) task_id: &'a str,
    pub(super) request: &'a SyncJobRequest,
    pub(super) plan: &'a SyncRequestPlan,
    pub(super) fetch_result: &'a SyncFetchResult,
    pub(super) derive_result: &'a DeriveResult,
    pub(super) display_record: &'a SyncRecordStats,
    pub(super) timeline: SyncCompletionTimeline<'a>,
}

pub(super) fn sync_completion_payload(input: SyncCompletionPayloadInput<'_>) -> Value {
    let task_id = input.task_id;
    let request = input.request;
    let plan = input.plan;
    let fetch_result = input.fetch_result;
    let derive_result = input.derive_result;
    let display_record = input.display_record;
    let timeline = input.timeline;
    let saved_count = fetch_result.saved_count;
    json!({
        "task_id": task_id,
        "inst_id": request.inst_id.as_str(),
        "inst_type": request.inst_type.as_str(),
        "timeframe": plan.display_timeframe.as_str(),
        "source_timeframe": plan.source_timeframe.as_str(),
        "target_timeframes": &plan.target_timeframes,
        "derived_timeframes": &derive_result.derived_timeframes,
        "mode": request.mode.as_str(),
        "days": request.days,
        "status": "completed",
        "progress": 100,
        "message": sync_completion_message(fetch_result, derive_result, &plan.source_timeframe),
        "created_at": timeline.created_at,
        "started_at": timeline.started_at,
        "updated_at": timeline.finished_at,
        "finished_at": timeline.finished_at,
        "error": "",
        "fetched_count": fetch_result.fetched_count,
        "target_fetch_count": fetch_result.target_fetch_count,
        "saved_count": saved_count,
        "target_save_count": fetch_result.target_save_count,
        "inserted_count": saved_count + derive_result.saved_count,
        "derived_count": derive_result.saved_count,
        "target_derive_count": derive_result.target_count,
        "batches": fetch_result.batches,
        "target_batches": fetch_result.target_batches,
        "api_calls": fetch_result.api_calls,
        "candle_count": display_record.candle_count,
        "history_complete": display_record.history_complete,
        "last_sync_mode": display_record.last_sync_mode,
        "last_sync_time": display_record.last_sync_time,
        "oldest_timestamp": display_record.oldest_timestamp,
        "newest_timestamp": display_record.newest_timestamp,
        "oldest_time": ts_to_iso(display_record.oldest_timestamp),
        "newest_time": ts_to_iso(display_record.newest_timestamp),
        "reused_existing": false,
        "truncated": fetch_result.truncated
    })
}

fn sync_completion_message(
    fetch_result: &SyncFetchResult,
    derive_result: &DeriveResult,
    source_timeframe: &str,
) -> String {
    let message = if fetch_result.truncated {
        format!(
            "{}；已达到单次同步批次数上限 {}，可再次发起继续回补",
            fetch_result.message,
            max_sync_batches()
        )
    } else {
        fetch_result.message.clone()
    };
    if derive_result.derived_timeframes.is_empty() {
        message
    } else {
        format!(
            "{}；已从 {} 对齐 {}",
            message,
            source_timeframe,
            derive_result.derived_timeframes.join("/")
        )
    }
}
