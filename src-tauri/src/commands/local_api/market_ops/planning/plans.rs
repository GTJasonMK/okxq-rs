use crate::{
    config::WatchedSymbolRecord,
    guardian::{GuardianSettings, GUARDIAN_SETTINGS_KEY},
};

use super::{
    super::*,
    timeframes::{normalize_timeframe_name, sort_timeframes},
};

#[derive(Clone, Debug)]
pub(in crate::commands::local_api::market_ops) struct WatchSyncPlan {
    pub(in crate::commands::local_api::market_ops) timeframe: String,
    pub(in crate::commands::local_api::market_ops) bootstrap_days: i64,
    pub(in crate::commands::local_api::market_ops) archive_mode: String,
}

pub(in crate::commands::local_api::market_ops) async fn guardian_settings(
    state: &AppState,
) -> AppResult<GuardianSettings> {
    let value = state.preferences.get(GUARDIAN_SETTINGS_KEY).await?;
    Ok(value
        .and_then(|item| serde_json::from_value::<GuardianSettings>(item).ok())
        .unwrap_or_default()
        .normalized())
}

fn watch_sync_plans(settings: &GuardianSettings) -> Vec<WatchSyncPlan> {
    settings
        .plans
        .iter()
        .filter(|plan| plan.enabled)
        .map(|plan| WatchSyncPlan {
            timeframe: plan.timeframe.clone(),
            bootstrap_days: plan.bootstrap_days.clamp(1, 3650),
            archive_mode: if plan.archive_mode.eq_ignore_ascii_case("full") {
                "full".to_string()
            } else {
                "rolling".to_string()
            },
        })
        .collect()
}

pub(in crate::commands::local_api::market_ops) fn watch_sync_plans_for_record(
    record: &WatchedSymbolRecord,
    settings: &GuardianSettings,
) -> Vec<WatchSyncPlan> {
    if record.sync_plans.is_empty() {
        return watch_sync_plans(settings);
    }
    record
        .sync_plans
        .iter()
        .filter(|plan| plan.enabled)
        .map(|plan| WatchSyncPlan {
            timeframe: plan.timeframe.clone(),
            bootstrap_days: plan.bootstrap_days.clamp(1, 3650),
            archive_mode: if plan.archive_mode.eq_ignore_ascii_case("full") {
                "full".to_string()
            } else {
                "rolling".to_string()
            },
        })
        .collect()
}

pub(in crate::commands::local_api::market_ops) fn base_sync_plan_for_plans(
    plans: &[WatchSyncPlan],
    archive_all_history: bool,
) -> Option<WatchSyncPlan> {
    if plans.is_empty() {
        return None;
    }
    let bootstrap_days = plans
        .iter()
        .map(|plan| plan.bootstrap_days)
        .max()
        .unwrap_or(30)
        .clamp(1, 3650);
    let archive_mode =
        if archive_all_history || plans.iter().any(|plan| plan.archive_mode == "full") {
            "full"
        } else {
            "rolling"
        };
    Some(WatchSyncPlan {
        timeframe: BASE_CANDLE_TIMEFRAME.to_string(),
        bootstrap_days,
        archive_mode: archive_mode.to_string(),
    })
}

pub(in crate::commands::local_api::market_ops) fn target_timeframes_from_plans(
    plans: &[WatchSyncPlan],
) -> Vec<String> {
    let mut timeframes = plans
        .iter()
        .map(|plan| normalize_timeframe_name(&plan.timeframe))
        .filter(|timeframe| !timeframe.is_empty())
        .collect::<Vec<_>>();
    if !timeframes
        .iter()
        .any(|timeframe| timeframe == BASE_CANDLE_TIMEFRAME)
    {
        timeframes.push(BASE_CANDLE_TIMEFRAME.to_string());
    }
    sort_timeframes(&mut timeframes);
    timeframes
}
