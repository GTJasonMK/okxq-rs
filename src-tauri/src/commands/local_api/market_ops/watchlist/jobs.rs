use std::collections::BTreeMap;

use crate::{config::WatchedSymbolRecord, guardian::GuardianSettings};

use super::super::*;

#[derive(Default)]
pub(super) struct WatchedRepairJobs {
    pub(super) sync_jobs: Vec<Value>,
    pub(super) reused_count: usize,
    pub(super) exact_gap_jobs: usize,
    pub(super) rule_jobs: usize,
}

pub(super) async fn enqueue_watched_record_sync_jobs(
    state: &AppState,
    record: &WatchedSymbolRecord,
    sync_spot: bool,
    sync_swap: bool,
) -> AppResult<(Vec<Value>, usize)> {
    if !sync_spot && !sync_swap {
        return Ok((Vec::new(), 0));
    }

    let settings = guardian_settings(state).await?;
    let requests =
        watched_record_sync_requests(state, record, &settings, sync_spot, sync_swap, true).await?;

    let mut jobs = Vec::new();
    let mut reused_count = 0usize;
    for request in requests {
        let (job, reused) = enqueue_sync_job(state, request).await?;
        if reused {
            reused_count += 1;
        }
        jobs.push(submitted_sync_job_value(&job, reused));
    }

    Ok((jobs, reused_count))
}

pub(super) async fn enqueue_watched_record_repair_jobs(
    state: &AppState,
    record: &WatchedSymbolRecord,
    sync_spot: bool,
    sync_swap: bool,
) -> AppResult<WatchedRepairJobs> {
    if !sync_spot && !sync_swap {
        return Ok(WatchedRepairJobs::default());
    }

    let settings = guardian_settings(state).await?;
    let (mut sync_jobs, mut reused_count, needs_rule_sync) =
        enqueue_watched_record_gap_repair_jobs(state, record, &settings, sync_spot, sync_swap)
            .await?;
    let exact_gap_jobs = sync_jobs.len();

    if needs_rule_sync {
        let (mut rule_jobs, rule_reused_count) =
            enqueue_watched_record_sync_jobs(state, record, sync_spot, sync_swap).await?;
        reused_count += rule_reused_count;
        let rule_jobs_count = rule_jobs.len();
        sync_jobs.append(&mut rule_jobs);
        return Ok(WatchedRepairJobs {
            sync_jobs,
            reused_count,
            exact_gap_jobs,
            rule_jobs: rule_jobs_count,
        });
    }

    Ok(WatchedRepairJobs {
        sync_jobs,
        reused_count,
        exact_gap_jobs,
        rule_jobs: 0,
    })
}

pub(in crate::commands::local_api::market_ops) async fn enqueue_watched_record_gap_repair_jobs(
    state: &AppState,
    record: &WatchedSymbolRecord,
    settings: &GuardianSettings,
    sync_spot: bool,
    sync_swap: bool,
) -> AppResult<(Vec<Value>, usize, bool)> {
    let plans = gap_repair_plans_for_record(record, settings);
    if plans.is_empty() {
        return Ok((Vec::new(), 0, false));
    }

    let mut jobs = Vec::new();
    let mut reused_count = 0usize;
    let mut needs_rule_sync = false;
    let now_ms = chrono::Utc::now().timestamp_millis();
    for (enabled, inst_id, inst_type) in [
        (sync_spot, record.spot_inst_id.as_str(), "SPOT"),
        (sync_swap, record.swap_inst_id.as_str(), "SWAP"),
    ] {
        if !enabled {
            continue;
        }
        for plan in &plans {
            let timeframe = normalize_timeframe_name(&plan.timeframe);
            if timeframe.is_empty() {
                continue;
            }
            if record.archive_all_history || plan.archive_mode == "full" {
                needs_rule_sync = true;
            }
            let timeframe_ms = timeframe_to_ms(&timeframe).max(1);
            let end_ts = latest_complete_candle_ts(now_ms, timeframe_ms);
            let start_ts = end_ts.saturating_sub(plan.bootstrap_days.max(1) * DAY_MS);
            for (range_start, range_end) in
                repair_ranges_for_plan(state, inst_id, inst_type, &timeframe, start_ts, end_ts)
                    .await?
            {
                if let Some((job, reused)) = enqueue_gap_repair_job_for_range(
                    state,
                    inst_id,
                    inst_type,
                    &timeframe,
                    range_start,
                    range_end,
                    "auto",
                )
                .await?
                {
                    if reused {
                        reused_count += 1;
                    }
                    jobs.push(submitted_sync_job_value(&job, reused));
                }
            }
        }
    }

    Ok((jobs, reused_count, needs_rule_sync))
}

async fn repair_ranges_for_plan(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    policy_start_ts: i64,
    policy_end_ts: i64,
) -> AppResult<Vec<(i64, i64)>> {
    let mut ranges = vec![(policy_start_ts, policy_end_ts)];
    if let Some((oldest, newest)) =
        local_candle_bounds(&state.db, inst_id, inst_type, timeframe).await?
    {
        ranges.push((oldest, newest));
    }
    Ok(merge_repair_ranges(ranges))
}

fn merge_repair_ranges(mut ranges: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    ranges.retain(|(start, end)| end >= start);
    ranges.sort_by_key(|(start, end)| (*start, *end));
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (start, end) in ranges {
        let Some(last) = merged.last_mut() else {
            merged.push((start, end));
            continue;
        };
        if start <= last.1 {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    merged
}

fn gap_repair_plans_for_record(
    record: &WatchedSymbolRecord,
    settings: &GuardianSettings,
) -> Vec<WatchSyncPlan> {
    let plans = watch_sync_plans_for_record(record, settings);
    let mut by_timeframe = BTreeMap::<String, WatchSyncPlan>::new();
    for plan in &plans {
        upsert_gap_repair_plan(&mut by_timeframe, plan.clone());
    }
    if let Some(base_plan) = base_sync_plan_for_plans(&plans, record.archive_all_history) {
        upsert_gap_repair_plan(&mut by_timeframe, base_plan);
    }
    let mut plans = by_timeframe.into_values().collect::<Vec<_>>();
    plans.sort_by_key(|plan| timeframe_to_ms(&plan.timeframe));
    plans
}

fn upsert_gap_repair_plan(
    by_timeframe: &mut BTreeMap<String, WatchSyncPlan>,
    mut plan: WatchSyncPlan,
) {
    plan.timeframe = normalize_timeframe_name(&plan.timeframe);
    if plan.timeframe.is_empty() {
        return;
    }
    by_timeframe
        .entry(plan.timeframe.clone())
        .and_modify(|existing| {
            existing.bootstrap_days = existing.bootstrap_days.max(plan.bootstrap_days);
            if plan.archive_mode == "full" {
                existing.archive_mode = "full".to_string();
            }
        })
        .or_insert(plan);
}

pub(super) async fn cancel_jobs_for_disabled_watch_markets(
    state: &AppState,
    previous: Option<&WatchedSymbolRecord>,
    current: &WatchedSymbolRecord,
) -> AppResult<Vec<Value>> {
    let Some(previous) = previous else {
        return Ok(Vec::new());
    };

    let mut disabled_inst_ids = Vec::new();
    if previous.sync_spot && !current.sync_spot {
        disabled_inst_ids.push(current.spot_inst_id.clone());
    }
    if previous.sync_swap && !current.sync_swap {
        disabled_inst_ids.push(current.swap_inst_id.clone());
    }
    if disabled_inst_ids.is_empty() {
        return Ok(Vec::new());
    }

    let cancelled = state
        .sync_jobs
        .cancel_jobs(
            Some(disabled_inst_ids.as_slice()),
            None,
            "关注规则已关闭该市场，后台同步任务已取消",
        )
        .await?;
    cancelled
        .into_iter()
        .map(|job| job.to_value().map_err(Into::into))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{config::WatchedSymbolSyncPlan, guardian::GuardianSettings};

    use super::*;

    #[test]
    fn derived_repair_plans_include_base_timeframe() {
        let record = watched_record(vec![sync_plan("1H", 365, "rolling")]);
        let plans = gap_repair_plans_for_record(&record, &GuardianSettings::default());

        let labels = plans
            .iter()
            .map(|plan| (plan.timeframe.as_str(), plan.bootstrap_days))
            .collect::<Vec<_>>();
        assert_eq!(labels, vec![("1m", 365), ("1H", 365)]);
    }

    #[test]
    fn custom_repair_plans_still_include_base_timeframe() {
        let record = watched_record(vec![
            sync_plan("1H", 90, "rolling"),
            sync_plan("5m", 30, "rolling"),
        ]);
        let plans = gap_repair_plans_for_record(&record, &GuardianSettings::default());

        assert_eq!(
            plans
                .iter()
                .map(|plan| plan.timeframe.as_str())
                .collect::<Vec<_>>(),
            vec!["1m", "5m", "1H"]
        );
    }

    #[test]
    fn repair_ranges_merge_local_inventory_range_with_policy_window() {
        let ranges = merge_repair_ranges(vec![
            (1_654_055_820_000, 1_780_578_660_000),
            (1_748_208_660_000, 1_780_578_660_000),
        ]);

        assert_eq!(ranges, vec![(1_654_055_820_000, 1_780_578_660_000)]);
    }

    #[test]
    fn repair_ranges_keep_disjoint_local_and_policy_ranges() {
        let ranges = merge_repair_ranges(vec![(1_000, 2_000), (10_000, 11_000)]);

        assert_eq!(ranges, vec![(1_000, 2_000), (10_000, 11_000)]);
    }

    fn watched_record(sync_plans: Vec<WatchedSymbolSyncPlan>) -> WatchedSymbolRecord {
        WatchedSymbolRecord {
            symbol: "BTC-USDT".to_string(),
            base_ccy: "BTC".to_string(),
            spot_inst_id: "BTC-USDT".to_string(),
            swap_inst_id: "BTC-USDT-SWAP".to_string(),
            sync_spot: false,
            sync_swap: true,
            archive_all_history: false,
            sync_days: 90,
            sync_plans,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn sync_plan(
        timeframe: &str,
        bootstrap_days: i64,
        archive_mode: &str,
    ) -> WatchedSymbolSyncPlan {
        WatchedSymbolSyncPlan {
            timeframe: timeframe.to_string(),
            enabled: true,
            bootstrap_days,
            archive_mode: archive_mode.to_string(),
        }
    }
}
