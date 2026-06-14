mod estimates;
mod mode;
mod plans;
mod requests;
mod timeframes;

pub(super) use self::plans::{
    base_sync_plan_for_plans, guardian_settings, target_timeframes_from_plans,
    watch_sync_plans_for_record, WatchSyncPlan,
};
pub(super) use self::requests::{sync_request, watched_record_sync_requests};
pub(in crate::commands::local_api) use self::timeframes::normalize_timeframe_name;
pub(super) use self::timeframes::{normalize_target_timeframes, sort_timeframes};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::local_api::market_ops::BASE_CANDLE_TIMEFRAME;

    use super::plans::{base_sync_plan_for_plans, WatchSyncPlan};

    #[test]
    fn sync_request_normalizes_display_and_targets_with_base_source() {
        let request = sync_request(
            "btc-usdt-swap",
            "swap",
            "1h",
            "window",
            3,
            vec!["1d".to_string(), "1h".to_string(), "1H".to_string()],
        )
        .expect("valid sync request");

        assert_eq!(request.timeframe, "1H");
        assert_eq!(request.source_timeframe, BASE_CANDLE_TIMEFRAME);
        assert_eq!(
            request.target_timeframes,
            vec!["1H".to_string(), "1D".to_string()]
        );
    }

    #[test]
    fn sync_request_rejects_invalid_timeframes() {
        assert!(sync_request("btc-usdt-swap", "swap", "bad", "window", 3, Vec::new(),).is_err());
        assert!(sync_request(
            "btc-usdt-swap",
            "swap",
            "1h",
            "window",
            3,
            vec!["bad".to_string()],
        )
        .is_err());
    }

    #[test]
    fn derived_targets_always_include_base_timeframe() {
        let plans = vec![WatchSyncPlan {
            timeframe: "1H".to_string(),
            bootstrap_days: 90,
            archive_mode: "rolling".to_string(),
        }];

        assert_eq!(
            target_timeframes_from_plans(&plans),
            vec![BASE_CANDLE_TIMEFRAME.to_string(), "1H".to_string()]
        );
    }

    #[test]
    fn base_plan_uses_max_selected_days_for_derived_sync() {
        let plans = vec![
            WatchSyncPlan {
                timeframe: "15m".to_string(),
                bootstrap_days: 30,
                archive_mode: "rolling".to_string(),
            },
            WatchSyncPlan {
                timeframe: "1H".to_string(),
                bootstrap_days: 90,
                archive_mode: "rolling".to_string(),
            },
        ];

        let base_plan = base_sync_plan_for_plans(&plans, false).expect("base plan");

        assert_eq!(base_plan.timeframe, BASE_CANDLE_TIMEFRAME);
        assert_eq!(base_plan.bootstrap_days, 90);
        assert_eq!(base_plan.archive_mode, "rolling");
    }
}
