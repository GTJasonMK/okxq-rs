use crate::{error::AppResult, sync_jobs::SyncJobRequest};

use super::super::super::*;

pub(super) struct SyncRequestPlan {
    pub(super) source_timeframe: String,
    pub(super) display_timeframe: String,
    pub(super) target_timeframes: Vec<String>,
    pub(super) planned_fetch_count: i64,
    pub(super) planned_save_count: i64,
    pub(super) planned_derive_count: i64,
    pub(super) planned_batches: i64,
}

pub(super) fn resolve_sync_request_plan(request: &SyncJobRequest) -> AppResult<SyncRequestPlan> {
    let source_timeframe = required_timeframe_name(&request.source_timeframe, "source_timeframe")?;
    let display_timeframe = required_timeframe_name(&request.timeframe, "timeframe")?;
    let mut target_timeframes = required_target_timeframes(request.target_timeframes.clone())?;
    if target_timeframes.is_empty() {
        target_timeframes.push(display_timeframe.clone());
    }
    if !target_timeframes
        .iter()
        .any(|timeframe| timeframe == &display_timeframe)
    {
        target_timeframes.push(display_timeframe.clone());
    }
    sort_timeframes(&mut target_timeframes);

    let planned_fetch_count =
        request
            .target_fetch_count
            .max(estimated_target_fetch_count_for_timeframe(
                &request.mode,
                request.days,
                &source_timeframe,
            ));
    let planned_save_count = request.target_save_count.max(planned_fetch_count);
    let planned_derive_count = request.target_derive_count.max(0);
    let planned_batches = request
        .target_batches
        .max(estimated_batches_for_count(planned_fetch_count));

    Ok(SyncRequestPlan {
        source_timeframe,
        display_timeframe,
        target_timeframes,
        planned_fetch_count,
        planned_save_count,
        planned_derive_count,
        planned_batches,
    })
}
