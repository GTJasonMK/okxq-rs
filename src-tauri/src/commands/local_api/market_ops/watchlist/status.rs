use super::super::*;

pub(in crate::commands::local_api) async fn guardian_status_payload(
    state: &AppState,
) -> AppResult<Value> {
    let settings = guardian_settings(state).await?;
    let watched = state.preferences.watched_symbols().await?;
    let active_jobs = state.sync_jobs.list(true, 200, None).await?;
    let runtime = state.guardian.snapshot().await;
    let watched_symbols = watched
        .iter()
        .map(|item| Value::String(item.symbol.clone()))
        .collect::<Vec<_>>();
    let mut watched_instruments = Vec::new();
    for item in &watched {
        if item.sync_spot {
            watched_instruments.push(Value::String(item.spot_inst_id.clone()));
        }
        if item.sync_swap {
            watched_instruments.push(Value::String(item.swap_inst_id.clone()));
        }
    }
    let timeframes = settings
        .plans
        .iter()
        .filter(|plan| plan.enabled)
        .map(|plan| Value::String(plan.timeframe.clone()))
        .collect::<Vec<_>>();
    let full_backfill_timeframes = settings
        .plans
        .iter()
        .filter(|plan| plan.enabled && plan.archive_mode == "full")
        .map(|plan| Value::String(plan.timeframe.clone()))
        .collect::<Vec<_>>();
    let rolling_window_timeframes = settings
        .plans
        .iter()
        .filter(|plan| plan.enabled && plan.archive_mode != "full")
        .map(|plan| Value::String(plan.timeframe.clone()))
        .collect::<Vec<_>>();
    let queue_preview = active_jobs
        .iter()
        .take(20)
        .map(|job| job.to_value())
        .collect::<Result<Vec<_>, _>>()?;
    let rest_base_url = okx_rest_base_url(state).await;

    Ok(json!({
        "enabled": settings.enabled,
        "active": runtime.active,
        "exchange_available": !rest_base_url.trim().is_empty(),
        "scan_interval_seconds": settings.scan_interval_seconds,
        "max_full_backfill_jobs_per_cycle": settings.max_full_backfill_jobs_per_cycle,
        "policy_summary": format!("{} 个周期，{} 个关注币种", timeframes.len(), watched.len()),
        "timeframes": timeframes,
        "full_backfill_timeframes": full_backfill_timeframes,
        "rolling_window_timeframes": rolling_window_timeframes,
        "watched_symbols": watched_symbols,
        "watched_instruments": watched_instruments,
        "watched_count": watched.len(),
        "inst_type": "MIXED",
        "current_inst_id": runtime.current_inst_id,
        "current_timeframe": runtime.current_timeframe,
        "current_mode": runtime.current_mode,
        "current_phase": runtime.current_phase,
        "cycle_completed_units": runtime.cycle_completed_units,
        "cycle_total_units": runtime.cycle_total_units,
        "backfill_queue_size": active_jobs.len(),
        "backfill_queue_preview": queue_preview,
        "last_run_started_at": runtime.last_run_started_at,
        "last_run_finished_at": runtime.last_run_finished_at,
        "last_successful_run_at": runtime.last_successful_run_at,
        "last_triggered_at": runtime.last_triggered_at,
        "last_run_summary": runtime.last_run_summary,
        "last_sync_results": runtime.last_sync_results,
        "last_errors": runtime.last_errors
    }))
}
