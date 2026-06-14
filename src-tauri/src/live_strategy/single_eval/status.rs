use crate::strategy_engine::StrategyActionRecord;

use super::super::{
    decision::StrategyIntentAction,
    runtime_helpers::{action_submission_key, timestamp_to_text, LiveActionSubmissionKeyInput},
    types::LiveStrategyStatus,
    LiveStrategyRuntime,
};

impl LiveStrategyRuntime {
    pub(super) async fn record_single_action_status(
        &self,
        run_id: &str,
        symbol: &str,
        action: StrategyIntentAction,
        order_type: &str,
        order_side: Option<&str>,
        exchange_size: Option<&str>,
        planned_exit_timestamp: Option<i64>,
        action_identity: Option<&str>,
        action_index: usize,
        action_record: &StrategyActionRecord,
    ) -> Option<bool> {
        let mut status = self.inner.status.write().await;
        if status.run_id != run_id {
            return None;
        }
        status.status = "running".to_string();
        status.error_message.clear();
        status.last_action_time = Some(timestamp_to_text(action_record.timestamp));
        status.last_action = action.as_str().to_string();
        status.last_price = Some(action_record.price);
        status.last_action_strength = Some(action_record.strength);
        status.last_action_reason = action_record.reason.clone();
        let should_record_action = if !matches!(action, StrategyIntentAction::Hold) {
            let key = action_submission_key(LiveActionSubmissionKeyInput {
                symbol,
                action,
                order_type,
                order_side,
                exchange_size,
                planned_exit_timestamp,
                action_identity,
                action_index,
                action_record,
            });
            let mut submitted_action_keys = self.inner.submitted_action_keys.lock().await;
            submitted_action_keys.insert(key)
        } else {
            false
        };
        if should_record_action {
            status.total_actions += 1;
            status.last_order_candle_ts = Some(action_record.timestamp);
        }
        Some(should_record_action)
    }

    pub(super) async fn record_idle_decision_status(
        &self,
        run_id: &str,
        timestamp: i64,
        price: f64,
        message: &str,
    ) {
        self.record_no_executable_decision_status(run_id, timestamp, price, "hold", message, false)
            .await;
    }

    pub(super) async fn record_skipped_action_status(
        &self,
        run_id: &str,
        timestamp: i64,
        price: f64,
        message: &str,
    ) {
        self.record_no_executable_decision_status(
            run_id,
            timestamp,
            price,
            "skipped_action",
            message,
            true,
        )
        .await;
    }

    pub(super) async fn forget_single_action_submission(
        &self,
        symbol: &str,
        action: StrategyIntentAction,
        order_type: &str,
        order_side: Option<&str>,
        exchange_size: Option<&str>,
        planned_exit_timestamp: Option<i64>,
        action_identity: Option<&str>,
        action_index: usize,
        action_record: &StrategyActionRecord,
    ) {
        let key = action_submission_key(LiveActionSubmissionKeyInput {
            symbol,
            action,
            order_type,
            order_side,
            exchange_size,
            planned_exit_timestamp,
            action_identity,
            action_index,
            action_record,
        });
        let mut submitted_action_keys = self.inner.submitted_action_keys.lock().await;
        submitted_action_keys.remove(&key);
    }

    async fn record_no_executable_decision_status(
        &self,
        run_id: &str,
        timestamp: i64,
        price: f64,
        action_name: &str,
        message: &str,
        is_error: bool,
    ) {
        let mut status = self.inner.status.write().await;
        if status.run_id != run_id {
            return;
        }
        status.status = "running".to_string();
        status.last_action_time = (timestamp > 0).then(|| timestamp_to_text(timestamp));
        status.last_action = action_name.to_string();
        status.last_price = (price.is_finite() && price > 0.0).then_some(price);
        status.last_action_strength = None;
        status.last_action_reason = message.to_string();
        if is_error {
            status.error_message = message.to_string();
        } else {
            status.error_message.clear();
        }
    }
}

pub(super) fn apply_blocked_order_status(
    status: &mut LiveStrategyStatus,
    action_record: &StrategyActionRecord,
    blocked_action: &str,
    message: &str,
) {
    status.last_action_time = Some(timestamp_to_text(action_record.timestamp));
    status.last_action = blocked_action.to_string();
    status.last_price = Some(action_record.price);
    status.last_action_strength = Some(action_record.strength.abs());
    status.last_action_reason = message.to_string();
    status.error_message = message.to_string();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_order_status_overrides_entry_action_summary() {
        let mut status = LiveStrategyStatus {
            run_id: "run".to_string(),
            last_action: "open_position".to_string(),
            last_action_reason: "过扩展做空".to_string(),
            ..LiveStrategyStatus::default()
        };
        let action_record = StrategyActionRecord {
            action: "open_position".to_string(),
            side: "sell".to_string(),
            price: 100.0,
            reason: "过扩展做空".to_string(),
            strength: 0.7,
            timestamp: 1_700_000_000_000,
            position_size: None,
        };

        apply_blocked_order_status(
            &mut status,
            &action_record,
            "risk_blocked",
            "单笔订单超过风控上限",
        );

        assert_eq!(status.last_action, "risk_blocked");
        assert_eq!(status.last_action_reason, "单笔订单超过风控上限");
        assert_eq!(status.error_message, "单笔订单超过风控上限");
        assert_eq!(status.last_price, Some(100.0));
        assert_eq!(status.last_action_strength, Some(0.7));
        assert_eq!(
            status.last_action_time,
            Some(timestamp_to_text(1_700_000_000_000))
        );
    }

    #[tokio::test]
    async fn duplicate_same_action_on_same_candle_is_not_recorded_twice() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
        }
        let action_record = StrategyActionRecord {
            action: "open_position".to_string(),
            side: "buy".to_string(),
            price: 100.0,
            reason: "entry".to_string(),
            strength: 1.0,
            timestamp: 1_700_000_000_000,
            position_size: Some(0.1),
        };

        let first = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                None,
                Some(1_700_003_600_000),
                None,
                0,
                &action_record,
            )
            .await;
        let second = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                None,
                Some(1_700_003_600_000),
                None,
                0,
                &action_record,
            )
            .await;

        assert_eq!(first, Some(true));
        assert_eq!(second, Some(false));
        let status = runtime.status().await;
        assert_eq!(status.total_actions, 1);
        assert_eq!(status.last_order_candle_ts, Some(action_record.timestamp));
    }

    #[tokio::test]
    async fn close_and_open_on_same_symbol_candle_are_distinct_actions() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
        }
        let close_signal = StrategyActionRecord {
            action: "close_position".to_string(),
            side: "flat".to_string(),
            price: 100.0,
            reason: "exit".to_string(),
            strength: 1.0,
            timestamp: 1_700_000_000_000,
            position_size: None,
        };
        let open_signal = StrategyActionRecord {
            action: "open_position".to_string(),
            side: "sell".to_string(),
            price: 99.0,
            reason: "reverse_short".to_string(),
            strength: 1.0,
            timestamp: 1_700_000_000_000,
            position_size: Some(0.1),
        };

        let close_recorded = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::ClosePosition,
                "market",
                Some("sell"),
                None,
                None,
                None,
                0,
                &close_signal,
            )
            .await;
        let open_recorded = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                None,
                Some(1_700_003_600_000),
                None,
                1,
                &open_signal,
            )
            .await;

        assert_eq!(close_recorded, Some(true));
        assert_eq!(open_recorded, Some(true));
        let status = runtime.status().await;
        assert_eq!(status.total_actions, 2);
        assert_eq!(status.last_order_candle_ts, Some(open_signal.timestamp));
    }

    #[tokio::test]
    async fn same_symbol_candle_same_action_different_plan_index_is_distinct() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
        }
        let action_record = StrategyActionRecord {
            action: "open_position".to_string(),
            side: "buy".to_string(),
            price: 100.0,
            reason: "scale_in".to_string(),
            strength: 1.0,
            timestamp: 1_700_000_000_000,
            position_size: Some(0.05),
        };

        let first = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                None,
                None,
                None,
                0,
                &action_record,
            )
            .await;
        let second = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                None,
                None,
                None,
                1,
                &action_record,
            )
            .await;

        assert_eq!(first, Some(true));
        assert_eq!(second, Some(true));
        let status = runtime.status().await;
        assert_eq!(status.total_actions, 2);
    }

    #[tokio::test]
    async fn same_candle_same_action_different_order_type_is_distinct() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
        }
        let action_record = StrategyActionRecord {
            action: "open_position".to_string(),
            side: "buy".to_string(),
            price: 100.0,
            reason: "entry_order_type_changed".to_string(),
            strength: 1.0,
            timestamp: 1_700_000_000_000,
            position_size: Some(0.05),
        };

        let market = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                None,
                None,
                None,
                0,
                &action_record,
            )
            .await;
        let duplicate_market = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                None,
                None,
                None,
                0,
                &action_record,
            )
            .await;
        let limit = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "limit",
                None,
                None,
                None,
                None,
                0,
                &action_record,
            )
            .await;

        assert_eq!(market, Some(true));
        assert_eq!(duplicate_market, Some(false));
        assert_eq!(limit, Some(true));
        let status = runtime.status().await;
        assert_eq!(status.total_actions, 2);
    }

    #[tokio::test]
    async fn same_candle_limit_actions_with_different_price_or_size_are_distinct() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
        }
        let base_signal = StrategyActionRecord {
            action: "open_position".to_string(),
            side: "buy".to_string(),
            price: 100.0,
            reason: "limit_entry".to_string(),
            strength: 1.0,
            timestamp: 1_700_000_000_000,
            position_size: Some(0.05),
        };
        let mut repriced_signal = base_signal.clone();
        repriced_signal.price = 101.25;
        let mut resized_signal = base_signal.clone();
        resized_signal.position_size = Some(0.08);

        let first = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "limit",
                None,
                None,
                None,
                None,
                0,
                &base_signal,
            )
            .await;
        let duplicate = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "limit",
                None,
                None,
                None,
                None,
                0,
                &base_signal,
            )
            .await;
        let repriced = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "limit",
                None,
                None,
                None,
                None,
                0,
                &repriced_signal,
            )
            .await;
        let resized = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "limit",
                None,
                None,
                None,
                None,
                0,
                &resized_signal,
            )
            .await;

        assert_eq!(first, Some(true));
        assert_eq!(duplicate, Some(false));
        assert_eq!(repriced, Some(true));
        assert_eq!(resized, Some(true));
        let status = runtime.status().await;
        assert_eq!(status.total_actions, 3);
    }

    #[tokio::test]
    async fn same_candle_market_action_ignores_display_price_for_dedupe() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
        }
        let base_signal = StrategyActionRecord {
            action: "open_position".to_string(),
            side: "buy".to_string(),
            price: 100.0,
            reason: "market_entry".to_string(),
            strength: 1.0,
            timestamp: 1_700_000_000_000,
            position_size: Some(0.05),
        };
        let mut repriced_signal = base_signal.clone();
        repriced_signal.price = 101.25;

        let first = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                None,
                None,
                None,
                0,
                &base_signal,
            )
            .await;
        let repriced = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                None,
                None,
                None,
                0,
                &repriced_signal,
            )
            .await;

        assert_eq!(first, Some(true));
        assert_eq!(repriced, Some(false));
        let status = runtime.status().await;
        assert_eq!(status.total_actions, 1);
    }

    #[tokio::test]
    async fn same_candle_market_actions_with_different_exchange_size_are_distinct() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
        }
        let action_record = StrategyActionRecord {
            action: "open_position".to_string(),
            side: "buy".to_string(),
            price: 100.0,
            reason: "explicit_okx_size".to_string(),
            strength: 1.0,
            timestamp: 1_700_000_000_000,
            position_size: Some(0.05),
        };

        let first = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                Some("2.03"),
                None,
                None,
                0,
                &action_record,
            )
            .await;
        let duplicate = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                Some("2.03"),
                None,
                None,
                0,
                &action_record,
            )
            .await;
        let different_exchange_size = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                Some("2.04"),
                None,
                None,
                0,
                &action_record,
            )
            .await;

        assert_eq!(first, Some(true));
        assert_eq!(duplicate, Some(false));
        assert_eq!(different_exchange_size, Some(true));
        let status = runtime.status().await;
        assert_eq!(status.total_actions, 2);
    }

    #[tokio::test]
    async fn same_candle_order_management_actions_use_target_identity_for_dedupe() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
        }
        let action_record = StrategyActionRecord {
            action: "modify_order".to_string(),
            side: "hold".to_string(),
            price: 0.0,
            reason: "order_management".to_string(),
            strength: 1.0,
            timestamp: 1_700_000_000_000,
            position_size: None,
        };

        let first = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::ModifyOrder,
                "market",
                None,
                None,
                None,
                Some("modify|ord=order-a|new_px=101"),
                0,
                &action_record,
            )
            .await;
        let duplicate = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::ModifyOrder,
                "market",
                None,
                None,
                None,
                Some("modify|ord=order-a|new_px=101"),
                0,
                &action_record,
            )
            .await;
        let different_target = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::ModifyOrder,
                "market",
                None,
                None,
                None,
                Some("modify|ord=order-b|new_px=102"),
                0,
                &action_record,
            )
            .await;

        assert_eq!(first, Some(true));
        assert_eq!(duplicate, Some(false));
        assert_eq!(different_target, Some(true));
        let status = runtime.status().await;
        assert_eq!(status.total_actions, 2);
    }

    #[tokio::test]
    async fn order_management_actions_ignore_plan_index_when_identity_matches() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
        }
        let action_record = StrategyActionRecord {
            action: "cancel_order".to_string(),
            side: "hold".to_string(),
            price: 0.0,
            reason: "cancel_duplicate".to_string(),
            strength: 1.0,
            timestamp: 1_700_000_000_000,
            position_size: None,
        };

        let first = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::CancelOrder,
                "market",
                None,
                None,
                None,
                Some("cancel|ord=order-a|cl=client-a"),
                0,
                &action_record,
            )
            .await;
        let same_target_different_index = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::CancelOrder,
                "market",
                None,
                None,
                None,
                Some("cancel|ord=order-a|cl=client-a"),
                3,
                &action_record,
            )
            .await;
        let different_target_same_index = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::CancelOrder,
                "market",
                None,
                None,
                None,
                Some("cancel|ord=order-b|cl=client-b"),
                3,
                &action_record,
            )
            .await;

        assert_eq!(first, Some(true));
        assert_eq!(same_target_different_index, Some(false));
        assert_eq!(different_target_same_index, Some(true));
        let status = runtime.status().await;
        assert_eq!(status.total_actions, 2);
    }

    #[tokio::test]
    async fn standalone_risk_actions_ignore_plan_index_when_identity_matches() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
        }
        let action_record = StrategyActionRecord {
            action: "place_risk_order".to_string(),
            side: "sell".to_string(),
            price: 0.0,
            reason: "protect_existing_position".to_string(),
            strength: 1.0,
            timestamp: 1_700_000_000_000,
            position_size: None,
        };

        let first = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::PlaceRiskOrder,
                "stop_market",
                Some("sell"),
                None,
                None,
                Some("risk|symbol=btc-usdt-swap|side=sell|trigger=94"),
                0,
                &action_record,
            )
            .await;
        let duplicate_different_index = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::PlaceRiskOrder,
                "stop_market",
                Some("sell"),
                None,
                None,
                Some("risk|symbol=btc-usdt-swap|side=sell|trigger=94"),
                2,
                &action_record,
            )
            .await;
        let moved_trigger = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::PlaceRiskOrder,
                "stop_market",
                Some("sell"),
                None,
                None,
                Some("risk|symbol=btc-usdt-swap|side=sell|trigger=95"),
                2,
                &action_record,
            )
            .await;

        assert_eq!(first, Some(true));
        assert_eq!(duplicate_different_index, Some(false));
        assert_eq!(moved_trigger, Some(true));
        let status = runtime.status().await;
        assert_eq!(status.total_actions, 2);
    }

    #[tokio::test]
    async fn open_actions_keep_plan_index_even_when_risk_identity_matches() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
        }
        let action_record = StrategyActionRecord {
            action: "open_position".to_string(),
            side: "buy".to_string(),
            price: 100.0,
            reason: "two_same_bar_entries".to_string(),
            strength: 1.0,
            timestamp: 1_700_000_000_000,
            position_size: Some(0.05),
        };

        let first = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                None,
                None,
                Some("open_risk|sl=0.06|attached=stop94"),
                0,
                &action_record,
            )
            .await;
        let same_identity_different_index = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::OpenPosition,
                "market",
                None,
                None,
                None,
                Some("open_risk|sl=0.06|attached=stop94"),
                1,
                &action_record,
            )
            .await;

        assert_eq!(first, Some(true));
        assert_eq!(same_identity_different_index, Some(true));
        let status = runtime.status().await;
        assert_eq!(status.total_actions, 2);
    }

    #[tokio::test]
    async fn forgotten_action_submission_can_be_recorded_again() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
        }
        let action_record = StrategyActionRecord {
            action: "close_position".to_string(),
            side: "flat".to_string(),
            price: 100.0,
            reason: "exit_retry".to_string(),
            strength: 1.0,
            timestamp: 1_700_000_000_000,
            position_size: None,
        };

        let first = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::ClosePosition,
                "market",
                Some("sell"),
                Some("2.03"),
                None,
                None,
                0,
                &action_record,
            )
            .await;
        runtime
            .forget_single_action_submission(
                "BTC-USDT-SWAP",
                StrategyIntentAction::ClosePosition,
                "market",
                Some("sell"),
                Some("2.03"),
                None,
                None,
                0,
                &action_record,
            )
            .await;
        let retry = runtime
            .record_single_action_status(
                "run",
                "BTC-USDT-SWAP",
                StrategyIntentAction::ClosePosition,
                "market",
                Some("sell"),
                Some("2.03"),
                None,
                None,
                0,
                &action_record,
            )
            .await;

        assert_eq!(first, Some(true));
        assert_eq!(retry, Some(true));
    }

    #[tokio::test]
    async fn idle_decision_clears_stale_error_without_counting_order_signal() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
            status.error_message = "上一次执行错误".to_string();
            status.total_actions = 3;
            status.total_orders = 2;
        }

        runtime
            .record_idle_decision_status("run", 1_700_000_000_000, 100.0, "策略当前无交易动作")
            .await;

        let status = runtime.status().await;
        assert_eq!(status.last_action, "hold");
        assert_eq!(status.last_action_reason, "策略当前无交易动作");
        assert_eq!(status.last_price, Some(100.0));
        assert_eq!(status.last_action_strength, None);
        assert_eq!(status.error_message, "");
        assert_eq!(status.total_actions, 3);
        assert_eq!(status.total_orders, 2);
    }

    #[tokio::test]
    async fn skipped_action_status_keeps_contract_error_visible() {
        let runtime = LiveStrategyRuntime::new();
        {
            let mut status = runtime.inner.status.write().await;
            status.run_id = "run".to_string();
            status.status = "running".to_string();
        }

        runtime
            .record_skipped_action_status(
                "run",
                1_700_000_000_000,
                100.0,
                "策略返回动作但执行层无法解析",
            )
            .await;

        let status = runtime.status().await;
        assert_eq!(status.last_action, "skipped_action");
        assert_eq!(status.last_action_reason, "策略返回动作但执行层无法解析");
        assert_eq!(status.error_message, "策略返回动作但执行层无法解析");
    }
}
