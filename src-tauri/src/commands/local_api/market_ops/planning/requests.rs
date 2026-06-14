use crate::{config::WatchedSymbolRecord, guardian::GuardianSettings, sync_jobs::SyncJobRequest};

use super::{
    super::*,
    estimates::planned_sync_targets,
    mode::select_watch_sync_mode,
    plans::{
        base_sync_plan_for_plans, target_timeframes_from_plans, watch_sync_plans_for_record,
        WatchSyncPlan,
    },
    timeframes::sort_timeframes,
};

pub(in crate::commands::local_api::market_ops) async fn watched_record_sync_requests(
    state: &AppState,
    record: &WatchedSymbolRecord,
    settings: &GuardianSettings,
    sync_spot: bool,
    sync_swap: bool,
    eager_full_on_missing: bool,
) -> AppResult<Vec<SyncJobRequest>> {
    if !sync_spot && !sync_swap {
        return Ok(Vec::new());
    }

    let plans = watch_sync_plans_for_record(record, settings);
    if plans.is_empty() {
        return Ok(Vec::new());
    }

    let mut requests = Vec::new();
    for (enabled, inst_id, inst_type) in [
        (sync_spot, record.spot_inst_id.as_str(), "SPOT"),
        (sync_swap, record.swap_inst_id.as_str(), "SWAP"),
    ] {
        if !enabled {
            continue;
        }
        append_derived_sync_request(
            state,
            &mut requests,
            inst_id,
            inst_type,
            &plans,
            record.archive_all_history,
            eager_full_on_missing,
        )
        .await?;
    }

    Ok(requests)
}

async fn append_derived_sync_request(
    state: &AppState,
    requests: &mut Vec<SyncJobRequest>,
    inst_id: &str,
    inst_type: &str,
    plans: &[WatchSyncPlan],
    archive_all_history: bool,
    eager_full_on_missing: bool,
) -> AppResult<()> {
    let Some(base_plan) = base_sync_plan_for_plans(plans, archive_all_history) else {
        return Ok(());
    };
    let mode = select_watch_sync_mode(
        &state.db,
        inst_id,
        inst_type,
        &base_plan,
        archive_all_history,
        eager_full_on_missing,
    )
    .await?;
    requests.push(sync_request(
        inst_id,
        inst_type,
        BASE_CANDLE_TIMEFRAME,
        mode,
        base_plan.bootstrap_days,
        target_timeframes_from_plans(plans),
    )?);
    Ok(())
}

pub(in crate::commands::local_api::market_ops) fn sync_request(
    inst_id: impl Into<String>,
    inst_type: impl Into<String>,
    display_timeframe: impl Into<String>,
    mode: impl Into<String>,
    days: i64,
    target_timeframes: Vec<String>,
) -> AppResult<SyncJobRequest> {
    let display_timeframe = required_timeframe_name(&display_timeframe.into(), "timeframe")?;
    let mode = mode.into();
    let days = days.max(1);
    let mut target_timeframes = required_target_timeframes(target_timeframes)?;
    if target_timeframes.is_empty() {
        target_timeframes.push(display_timeframe.clone());
    }
    sort_timeframes(&mut target_timeframes);
    let (target_fetch_count, target_save_count, target_derive_count, target_batches) =
        planned_sync_targets(&mode, days, BASE_CANDLE_TIMEFRAME, &target_timeframes);
    Ok(SyncJobRequest {
        inst_id: inst_id.into(),
        inst_type: inst_type.into(),
        timeframe: display_timeframe,
        source_timeframe: BASE_CANDLE_TIMEFRAME.to_string(),
        target_timeframes,
        mode,
        days,
        start_ts: None,
        end_ts: None,
        repair_method: String::new(),
        target_fetch_count,
        target_save_count,
        target_derive_count,
        target_batches,
    })
}
