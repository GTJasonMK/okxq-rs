use serde_json::Value;

use super::{
    timeframes::{normalize_timeframe, timeframe_order},
    types::{WatchedSymbolSyncPlan, DEFAULT_SYNC_DAYS},
};

pub(in crate::config::preferences) fn normalize_sync_days(value: i64) -> i64 {
    value.clamp(1, 3650)
}

pub(in crate::config::preferences) fn infer_sync_days_from_plans(
    plans: &[WatchedSymbolSyncPlan],
) -> i64 {
    plans
        .iter()
        .filter(|plan| plan.enabled)
        .map(|plan| plan.bootstrap_days)
        .max()
        .map(normalize_sync_days)
        .unwrap_or(DEFAULT_SYNC_DAYS)
}

pub(in crate::config::preferences) fn apply_sync_days_to_plans(
    plans: Vec<WatchedSymbolSyncPlan>,
    sync_days: i64,
) -> Vec<WatchedSymbolSyncPlan> {
    let sync_days = normalize_sync_days(sync_days);
    plans
        .into_iter()
        .map(|mut plan| {
            plan.bootstrap_days = sync_days;
            plan
        })
        .collect()
}

pub(super) fn normalize_sync_plans_from_value(value: Option<&Value>) -> Vec<WatchedSymbolSyncPlan> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    let plans = items
        .iter()
        .filter_map(|item| serde_json::from_value::<WatchedSymbolSyncPlan>(item.clone()).ok())
        .collect::<Vec<_>>();
    normalize_sync_plans_from_slice(&plans)
}

pub(in crate::config::preferences) fn normalize_sync_plans_from_slice(
    plans: &[WatchedSymbolSyncPlan],
) -> Vec<WatchedSymbolSyncPlan> {
    let mut seen = std::collections::BTreeSet::new();
    let mut normalized = plans
        .iter()
        .filter_map(normalize_sync_plan)
        .filter(|plan| seen.insert(plan.timeframe.clone()))
        .collect::<Vec<_>>();
    normalized.sort_by_key(|plan| timeframe_order(&plan.timeframe));
    normalized
}

pub(in crate::config::preferences) fn has_enabled_sync_plan(
    plans: &[WatchedSymbolSyncPlan],
) -> bool {
    plans.iter().any(|plan| plan.enabled)
}

fn normalize_sync_plan(plan: &WatchedSymbolSyncPlan) -> Option<WatchedSymbolSyncPlan> {
    let timeframe = normalize_timeframe(&plan.timeframe)?;
    let archive_mode = if plan.archive_mode.trim().eq_ignore_ascii_case("full") {
        "full".to_string()
    } else {
        "rolling".to_string()
    };
    Some(WatchedSymbolSyncPlan {
        timeframe,
        enabled: plan.enabled,
        bootstrap_days: plan.bootstrap_days.clamp(1, 3650),
        archive_mode,
    })
}
