use serde::{Deserialize, Serialize};

use crate::timeframes::okx_timeframe_order;

pub const GUARDIAN_SETTINGS_KEY: &str = "data_guardian_settings";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuardianPlan {
    pub timeframe: String,
    pub enabled: bool,
    pub bootstrap_days: i64,
    pub archive_mode: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuardianSettings {
    pub enabled: bool,
    pub scan_interval_seconds: i64,
    pub max_full_backfill_jobs_per_cycle: i64,
    pub plans: Vec<GuardianPlan>,
}

impl Default for GuardianSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            scan_interval_seconds: 300,
            max_full_backfill_jobs_per_cycle: 1,
            plans: vec![
                GuardianPlan::new("1m", true, 90, "rolling"),
                GuardianPlan::new("5m", true, 90, "rolling"),
                GuardianPlan::new("15m", true, 90, "rolling"),
                GuardianPlan::new("1H", true, 90, "rolling"),
                GuardianPlan::new("4H", true, 90, "rolling"),
                GuardianPlan::new("1D", true, 365, "full"),
            ],
        }
    }
}

impl GuardianSettings {
    pub fn normalized(mut self) -> Self {
        self.scan_interval_seconds = self.scan_interval_seconds.clamp(60, 3600);
        self.max_full_backfill_jobs_per_cycle = self.max_full_backfill_jobs_per_cycle.clamp(1, 20);
        self.plans = self
            .plans
            .into_iter()
            .filter_map(|plan| {
                let timeframe = plan.timeframe.trim().to_string();
                if timeframe.is_empty() {
                    return None;
                }
                let archive_mode = if plan.archive_mode.trim().eq_ignore_ascii_case("full") {
                    "full"
                } else {
                    "rolling"
                };
                Some(GuardianPlan::new(
                    &timeframe,
                    plan.enabled,
                    plan.bootstrap_days.clamp(1, 3650),
                    archive_mode,
                ))
            })
            .collect();
        if self.plans.is_empty() {
            self.plans = Self::default().plans;
        }
        self.plans
            .sort_by_key(|plan| okx_timeframe_order(&plan.timeframe));
        self
    }
}

impl GuardianPlan {
    fn new(timeframe: &str, enabled: bool, bootstrap_days: i64, archive_mode: &str) -> Self {
        Self {
            timeframe: timeframe.to_string(),
            enabled,
            bootstrap_days,
            archive_mode: archive_mode.to_string(),
        }
    }
}
