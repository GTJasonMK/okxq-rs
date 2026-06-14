use sqlx::SqlitePool;

use crate::{
    okx::{OkxPrivateClient, OkxPublicClient},
    realtime::RealtimeManager,
    strategy_engine::StrategyConfig,
    strategy_executor::PythonRunnerSession,
};

mod live;
mod status;

use super::{
    decision::{fetch_strategy_candles, latest_strategy_plan, LatestStrategyPlanRequest},
    runtime_helpers::{action_dedupe_identity, required_action_candle_count_for_timeframe},
    types::LiveStrategyConfig,
    LiveStrategyRuntime,
};

#[cfg(test)]
use super::runtime_helpers::order_management_action_identity;

impl LiveStrategyRuntime {
    pub(super) async fn evaluate_once(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        client: &OkxPublicClient,
        private_client: Option<&OkxPrivateClient>,
        realtime: Option<&RealtimeManager>,
        strategy_session: Option<&mut PythonRunnerSession>,
    ) {
        let Some(private_client) = private_client else {
            self.set_error(
                run_id,
                "实时策略启动必须提供 OKX 私有交易客户端".to_string(),
            )
            .await;
            return;
        };

        let required_candles =
            required_action_candle_count_for_timeframe(&config.params, &config.timeframe);
        self.log_execution_stage(
            run_id,
            "candles",
            "info",
            format!(
                "开始获取策略 K 线: {} {}，至少 {} 根",
                config.symbol, config.timeframe, required_candles
            ),
            serde_json::json!({
                "symbol": config.symbol,
                "timeframe": config.timeframe,
                "required_candles": required_candles,
            }),
        )
        .await;
        let candles = match fetch_strategy_candles(db, client, config, required_candles).await {
            Ok(candles) if candles.len() >= 3 => {
                self.log_execution_stage(
                    run_id,
                    "candles",
                    "success",
                    format!("策略 K 线获取完成，共 {} 根", candles.len()),
                    serde_json::json!({
                        "candle_count": candles.len(),
                        "latest_timestamp": candles.last().map(|item| item.timestamp),
                    }),
                )
                .await;
                candles
            }
            Ok(_) => {
                self.log_execution_stage(
                    run_id,
                    "candles",
                    "warn",
                    "实时策略 K 线数量不足，等待下一轮检查",
                    serde_json::json!({ "required_min": 3 }),
                )
                .await;
                self.set_error(run_id, "实时策略 K 线数量不足，等待下一轮检查".to_string())
                    .await;
                return;
            }
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "candles",
                    "error",
                    format!("获取实时策略 K 线失败: {error}"),
                    serde_json::json!({}),
                )
                .await;
                self.set_error(run_id, format!("获取实时策略 K 线失败: {error}"))
                    .await;
                return;
            }
        };

        let strategy_config = StrategyConfig {
            strategy_id: config.strategy_id.clone(),
            strategy_name: config.strategy_name.clone(),
            symbol: config.symbol.clone(),
            inst_type: config.inst_type.clone(),
            timeframe: config.timeframe.clone(),
            initial_capital: config.initial_capital,
            position_size: config.position_size,
            stop_loss: config.stop_loss,
            take_profit: config.take_profit,
            params: config.params.clone(),
        };
        let runtime_status = self.status().await;
        self.log_execution_stage(
            run_id,
            "decision",
            "info",
            "开始执行策略决策",
            serde_json::json!({
                "strategy_id": config.strategy_id,
                "strategy_name": config.strategy_name,
                "candle_count": candles.len(),
            }),
        )
        .await;
        let runtime_for_strategy_events = self.clone();
        let run_id_for_strategy_events = run_id.to_string();
        let mut on_strategy_event = move |event: &serde_json::Value| {
            let runtime = runtime_for_strategy_events.clone();
            let run_id = run_id_for_strategy_events.clone();
            let event = event.clone();
            tauri::async_runtime::spawn(async move {
                runtime.log_strategy_event(&run_id, &event).await;
            });
        };
        let plan = match latest_strategy_plan(
            LatestStrategyPlanRequest {
                db,
                client,
                project_root: &config.project_root,
                live_config: config,
                runtime_status: &runtime_status,
                config: &strategy_config,
                candles: &candles,
                private_client: Some(private_client),
                realtime,
                strategy_session,
            },
            &mut on_strategy_event,
        )
        .await
        {
            Ok(plan) => plan,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "decision",
                    "error",
                    format!("执行实时策略决策失败: {error}"),
                    serde_json::json!({}),
                )
                .await;
                self.set_error(run_id, format!("执行实时策略决策失败: {error}"))
                    .await;
                return;
            }
        };
        for log in &plan.execution_logs {
            self.log_strategy_execution_entry(run_id, log).await;
        }
        if plan.intents.is_empty() {
            let skipped_action_count = plan.skipped_actions.len();
            let idle_action_count = plan.idle_actions.len();
            let latest = candles
                .last()
                .expect("live strategy evaluation requires at least three candles");
            let latest_timestamp = latest.timestamp;
            let latest_price = latest.close;
            let message = if skipped_action_count > 0 {
                format!(
                    "策略本轮有 {skipped_action_count} 个动作未通过执行合约校验，已阻断整批交易；请检查 action/side/order_side 合约"
                )
            } else if idle_action_count > 0 {
                format!("策略返回 {idle_action_count} 个 hold 动作，当前不提交交易")
            } else if !plan.risk_actions.is_empty() {
                "策略仅返回保护单/辅助动作，未形成开仓或平仓意图，继续等待下一轮".to_string()
            } else {
                "策略当前无交易动作，继续等待下一轮".to_string()
            };
            self.log_execution_stage(
                run_id,
                "decision",
                if skipped_action_count > 0 {
                    "warn"
                } else {
                    "info"
                },
                message.clone(),
                serde_json::json!({
                    "risk_action_count": plan.risk_actions.len(),
                    "skipped_action_count": skipped_action_count,
                    "idle_action_count": idle_action_count,
                    "strategy_log_count": plan.execution_logs.len(),
                    "skipped_actions": skipped_actions_preview(&plan.skipped_actions),
                    "idle_actions": skipped_actions_preview(&plan.idle_actions),
                }),
            )
            .await;
            if skipped_action_count > 0 {
                self.record_skipped_action_status(run_id, latest_timestamp, latest_price, &message)
                    .await;
            } else {
                self.record_idle_decision_status(run_id, latest_timestamp, latest_price, &message)
                    .await;
            }
            return;
        }
        self.log_execution_stage(
            run_id,
            "decision",
            "success",
            format!("策略决策完成，生成 {} 个可执行动作", plan.intents.len()),
            serde_json::json!({
                "intent_count": plan.intents.len(),
                "risk_action_count": plan.risk_actions.len(),
                "skipped_action_count": plan.skipped_actions.len(),
                "idle_action_count": plan.idle_actions.len(),
                "strategy_log_count": plan.execution_logs.len(),
            }),
        )
        .await;
        if !plan.risk_actions.is_empty() {
            tracing::debug!(
                strategy_id = %config.strategy_id,
                risk_actions = %plan.risk_actions.len(),
                "策略返回了已挂接到执行意图的保护单动作"
            );
        }
        if !plan.diagnostics.is_null() {
            tracing::trace!(
                strategy_id = %config.strategy_id,
                diagnostics = %plan.diagnostics,
                "运行策略决策诊断"
            );
        }

        for (action_index, intent) in plan.intents.into_iter().enumerate() {
            let execution_config = intent.execution_config(config);
            let action = intent.action;
            let order_type = intent.order_type.clone();
            let order_side = intent.order_side.as_deref();
            let exchange_size = intent.exchange_size.as_deref();
            let stop_loss = intent.stop_loss;
            let take_profit = intent.take_profit;
            let max_slippage = intent.max_slippage;
            let attached_risk_orders = intent.attached_risk_orders.clone();
            let planned_exit = intent.planned_exit.clone();
            let cancel_order = intent.cancel_order.clone();
            let modify_order = intent.modify_order.clone();
            let action_identity = action_dedupe_identity(
                action,
                cancel_order.as_ref(),
                modify_order.as_ref(),
                stop_loss,
                take_profit,
                max_slippage,
                &attached_risk_orders,
            );
            let action_record = intent.action_record;

            let Some(should_record_action) = self
                .record_single_action_status(
                    run_id,
                    &execution_config.symbol,
                    action,
                    &order_type,
                    order_side,
                    exchange_size,
                    planned_exit.as_ref().map(|item| item.timestamp),
                    action_identity.as_deref(),
                    action_index,
                    &action_record,
                )
                .await
            else {
                return;
            };
            self.log_execution_stage(
                run_id,
                "intent",
                if should_record_action { "info" } else { "warn" },
                format!(
                    "处理策略动作: {} {} @ {:.8}",
                    execution_config.symbol, action_record.side, action_record.price
                ),
                serde_json::json!({
                    "symbol": execution_config.symbol,
                    "inst_type": execution_config.inst_type,
                    "timeframe": execution_config.timeframe,
                    "action": action.as_str(),
                    "action_side": action_record.side,
                    "action_price": action_record.price,
                    "timestamp": action_record.timestamp,
                    "order_type": order_type,
                    "order_side": order_side,
                    "exchange_size": exchange_size,
                    "action_identity": action_identity.as_deref(),
                    "should_record_action": should_record_action,
                    "attached_risk_orders": attached_risk_orders.len(),
                    "planned_exit_time": planned_exit.as_ref().map(|item| item.timestamp),
                    "cancel_order_id": cancel_order.as_ref().map(|item| item.order_id.as_str()),
                    "cancel_client_order_id": cancel_order.as_ref().map(|item| item.client_order_id.as_str()),
                    "modify_order_id": modify_order.as_ref().map(|item| item.order_id.as_str()),
                    "modify_client_order_id": modify_order.as_ref().map(|item| item.client_order_id.as_str()),
                    "modify_new_size": modify_order.as_ref().and_then(|item| item.new_size.as_deref()),
                    "modify_new_price": modify_order.as_ref().and_then(|item| item.new_price.as_deref()),
                }),
            )
            .await;

            let execution_outcome = self
                .evaluate_live_action(
                    run_id,
                    &execution_config,
                    db,
                    client,
                    private_client,
                    &action_record,
                    action,
                    &order_type,
                    order_side,
                    exchange_size,
                    &attached_risk_orders,
                    planned_exit.as_ref(),
                    cancel_order.as_ref(),
                    modify_order.as_ref(),
                    should_record_action,
                )
                .await;
            if should_record_action && execution_outcome.should_release_action_dedupe_key() {
                self.forget_single_action_submission(
                    &execution_config.symbol,
                    action,
                    &order_type,
                    order_side,
                    exchange_size,
                    planned_exit.as_ref().map(|item| item.timestamp),
                    action_identity.as_deref(),
                    action_index,
                    &action_record,
                )
                .await;
                self.log_execution_stage(
                    run_id,
                    "intent",
                    "warn",
                    format!(
                        "策略动作 {} 未提交成功，已释放本轮动作去重键以允许下次触发重试",
                        action.as_str()
                    ),
                    serde_json::json!({
                        "symbol": execution_config.symbol,
                        "action": action.as_str(),
                        "timestamp": action_record.timestamp,
                        "order_side": order_side,
                    }),
                )
                .await;
            }
        }
    }
}

fn skipped_actions_preview(actions: &[serde_json::Value]) -> Vec<serde_json::Value> {
    actions.iter().take(10).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::{action_dedupe_identity, order_management_action_identity};
    use crate::live_strategy::decision::{
        StrategyCancelOrderIntent, StrategyIntentAction, StrategyModifyOrderIntent,
        StrategyOrderTargetKind, StrategyRiskOrderIntent,
    };

    #[test]
    fn modify_action_identity_ignores_transport_request_id_and_normalizes_decimals() {
        let first = StrategyModifyOrderIntent {
            order_id: "ord-1".to_string(),
            client_order_id: "cl-1".to_string(),
            new_size: Some("2.0000".to_string()),
            new_price: Some("101.250000".to_string()),
            cancel_on_fail: false,
            request_id: "req-a".to_string(),
            scope_explicit: false,
            target_kind: StrategyOrderTargetKind::Any,
            target_order_type: None,
        };
        let second = StrategyModifyOrderIntent {
            order_id: "ord-1".to_string(),
            client_order_id: "cl-1".to_string(),
            new_size: Some("2".to_string()),
            new_price: Some("101.25".to_string()),
            cancel_on_fail: false,
            request_id: "req-b".to_string(),
            scope_explicit: false,
            target_kind: StrategyOrderTargetKind::Any,
            target_order_type: None,
        };

        assert_eq!(
            order_management_action_identity(None, Some(&first)),
            order_management_action_identity(None, Some(&second))
        );
    }

    #[test]
    fn cancel_action_identity_uses_order_identity() {
        let cancel = StrategyCancelOrderIntent {
            order_id: "ord-1".to_string(),
            client_order_id: "cl-1".to_string(),
            scope_explicit: false,
            target_kind: StrategyOrderTargetKind::Any,
        };

        assert_eq!(
            order_management_action_identity(Some(&cancel), None).as_deref(),
            Some("cancel|kind=any|ord=ord-1|cl=cl-1")
        );
    }

    #[test]
    fn order_management_action_identity_includes_target_kind() {
        let mut exchange_cancel = StrategyCancelOrderIntent {
            order_id: "shared-id".to_string(),
            client_order_id: String::new(),
            scope_explicit: true,
            target_kind: StrategyOrderTargetKind::Exchange,
        };
        let exchange_identity = order_management_action_identity(Some(&exchange_cancel), None);
        exchange_cancel.target_kind = StrategyOrderTargetKind::Algo;

        assert_ne!(
            exchange_identity,
            order_management_action_identity(Some(&exchange_cancel), None)
        );
    }

    #[test]
    fn modify_action_identity_includes_target_order_type() {
        let mut stop_modify = StrategyModifyOrderIntent {
            order_id: String::new(),
            client_order_id: "shared-algo-client-id".to_string(),
            new_size: Some("2.0000".to_string()),
            new_price: Some("94.5000".to_string()),
            cancel_on_fail: true,
            request_id: "req-a".to_string(),
            scope_explicit: true,
            target_kind: StrategyOrderTargetKind::Algo,
            target_order_type: Some("stop_market".to_string()),
        };
        let stop_identity = order_management_action_identity(None, Some(&stop_modify));
        stop_modify.request_id = "req-b".to_string();
        assert_eq!(
            stop_identity,
            order_management_action_identity(None, Some(&stop_modify))
        );

        stop_modify.target_order_type = Some("take_profit_market".to_string());
        assert_ne!(
            stop_identity,
            order_management_action_identity(None, Some(&stop_modify))
        );
    }

    #[test]
    fn open_action_identity_includes_risk_settings() {
        let first = action_dedupe_identity(
            StrategyIntentAction::OpenPosition,
            None,
            None,
            Some(0.045),
            None,
            Some(0.002),
            &[],
        );
        let duplicate = action_dedupe_identity(
            StrategyIntentAction::OpenPosition,
            None,
            None,
            Some(0.0450),
            None,
            Some(0.0020),
            &[],
        );
        let changed_stop = action_dedupe_identity(
            StrategyIntentAction::OpenPosition,
            None,
            None,
            Some(0.05),
            None,
            Some(0.002),
            &[],
        );

        assert_eq!(first, duplicate);
        assert_ne!(first, changed_stop);
    }

    #[test]
    fn risk_action_identity_includes_trigger_price() {
        let base = vec![risk_order(Some(94.0), Some(0.06), None)];
        let duplicate = vec![risk_order(Some(94.0000), Some(0.0600), None)];
        let moved_trigger = vec![risk_order(Some(95.0), Some(0.06), None)];

        assert_eq!(
            action_dedupe_identity(
                StrategyIntentAction::PlaceRiskOrder,
                None,
                None,
                None,
                None,
                None,
                &base
            ),
            action_dedupe_identity(
                StrategyIntentAction::PlaceRiskOrder,
                None,
                None,
                None,
                None,
                None,
                &duplicate
            )
        );
        assert_ne!(
            action_dedupe_identity(
                StrategyIntentAction::PlaceRiskOrder,
                None,
                None,
                None,
                None,
                None,
                &base
            ),
            action_dedupe_identity(
                StrategyIntentAction::PlaceRiskOrder,
                None,
                None,
                None,
                None,
                None,
                &moved_trigger
            )
        );
    }

    #[test]
    fn open_action_identity_includes_attached_risk_orders() {
        let stop_94 = vec![risk_order(Some(94.0), Some(0.06), None)];
        let stop_95 = vec![risk_order(Some(95.0), Some(0.06), None)];

        assert_ne!(
            action_dedupe_identity(
                StrategyIntentAction::OpenPosition,
                None,
                None,
                Some(0.06),
                None,
                None,
                &stop_94
            ),
            action_dedupe_identity(
                StrategyIntentAction::OpenPosition,
                None,
                None,
                Some(0.06),
                None,
                None,
                &stop_95
            )
        );
    }

    fn risk_order(
        trigger_price: Option<f64>,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
    ) -> StrategyRiskOrderIntent {
        StrategyRiskOrderIntent {
            symbol: "BTC-USDT-SWAP".to_string(),
            side: "sell".to_string(),
            order_type: "stop_market".to_string(),
            trigger_price,
            stop_loss,
            take_profit,
            reason: "protective_stop".to_string(),
        }
    }
}
