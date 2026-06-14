use crate::live_strategy::arrival::ArrivalQuote;

use super::*;

impl LiveStrategyRuntime {
    pub(in crate::live_strategy::single_eval) async fn evaluate_live_action(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        client: &OkxPublicClient,
        private_client: &OkxPrivateClient,
        action_record: &StrategyActionRecord,
        action: StrategyIntentAction,
        order_type: &str,
        close_order_side: Option<&str>,
        exchange_size: Option<&str>,
        attached_risk_orders: &[StrategyRiskOrderIntent],
        planned_exit: Option<&StrategyPlannedExitIntent>,
        cancel_order: Option<&StrategyCancelOrderIntent>,
        modify_order: Option<&StrategyModifyOrderIntent>,
        should_record_action: bool,
    ) -> LiveActionExecutionOutcome {
        if !should_record_action || matches!(action, StrategyIntentAction::Hold) {
            return LiveActionExecutionOutcome::Skipped;
        }
        if matches!(action, StrategyIntentAction::ModifyOrder) {
            return live_action_execution_outcome(
                self.submit_exchange_modify_intent(
                    run_id,
                    config,
                    db,
                    client,
                    private_client,
                    action_record,
                    modify_order,
                )
                .await,
            );
        }
        if matches!(action, StrategyIntentAction::CancelOrder) {
            return live_action_execution_outcome(
                self.submit_exchange_cancel_intent(
                    run_id,
                    config,
                    db,
                    private_client,
                    action_record,
                    cancel_order,
                )
                .await,
            );
        }
        if matches!(action, StrategyIntentAction::PlaceRiskOrder) {
            return live_action_execution_outcome(
                self.submit_exchange_risk_order_intent(
                    run_id,
                    config,
                    db,
                    client,
                    private_client,
                    action_record,
                    close_order_side,
                    exchange_size,
                    attached_risk_orders,
                )
                .await,
            );
        }
        if action.closes_position() {
            return live_action_execution_outcome(
                self.submit_exchange_close_intent(
                    run_id,
                    config,
                    db,
                    client,
                    private_client,
                    action_record,
                    order_type,
                    close_order_side,
                    exchange_size,
                    None,
                    None,
                    None,
                )
                .await,
            );
        }
        let Some((pos_side, _)) = action_position_side(&action_record.side) else {
            return LiveActionExecutionOutcome::terminal(format!(
                "无法从策略 side={} 推导开仓方向",
                action_record.side
            ));
        };
        if pos_side == "short" && !is_contract_inst_type(&config.inst_type) {
            return LiveActionExecutionOutcome::terminal(
                "现货实时策略暂不支持用 open_position 做空".to_string(),
            );
        }
        if pos_side == "short" && !risk_controls::allow_short(&config.params, &config.inst_type) {
            return LiveActionExecutionOutcome::terminal("策略配置不允许做空".to_string());
        }
        if let Err(error) = explicit_configured_leverage(config) {
            let reason = error.to_string();
            self.log_execution_stage(
                run_id,
                "leverage",
                "error",
                reason.clone(),
                serde_json::json!({
                    "symbol": config.symbol,
                    "inst_type": config.inst_type,
                }),
            )
            .await;
            let client_order_id = live_strategy_client_order_id();
            self.record_exchange_submit_failure(
                run_id,
                config,
                db,
                action_record,
                entry_order_side(&action_record.side),
                pre_quantity_validation_failure_size(config, action_record),
                order_type,
                &client_order_id,
                ArrivalQuote::default(),
                &reason,
            )
            .await;
            return LiveActionExecutionOutcome::terminal(reason);
        }
        if let Err(error) = td_mode_from_config(config) {
            let reason = error.to_string();
            self.log_execution_stage(
                run_id,
                "td_mode",
                "error",
                reason.clone(),
                serde_json::json!({
                    "symbol": config.symbol,
                    "inst_type": config.inst_type,
                }),
            )
            .await;
            let client_order_id = live_strategy_client_order_id();
            self.record_exchange_submit_failure(
                run_id,
                config,
                db,
                action_record,
                entry_order_side(&action_record.side),
                pre_quantity_validation_failure_size(config, action_record),
                order_type,
                &client_order_id,
                ArrivalQuote::default(),
                &reason,
            )
            .await;
            return LiveActionExecutionOutcome::terminal(reason);
        }

        let entry_quantity = match resolve_entry_order_quantities(
            client,
            config,
            action_record,
            exchange_size,
        )
        .await
        {
            Ok(quantity) => quantity,
            Err(error) => {
                let retryable = retryable_for_app_error(&error);
                let reason = error.to_string();
                self.log_execution_stage(
                    run_id,
                    "quantity",
                    "error",
                    format!("解析交易所下单数量失败: {reason}"),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "price": action_record.price,
                        "exchange_size": exchange_size,
                    }),
                )
                .await;
                let arrival = fetch_arrival_quote(client, config).await;
                let client_order_id = live_strategy_client_order_id();
                self.record_exchange_submit_failure(
                    run_id,
                    config,
                    db,
                    action_record,
                    entry_order_side(&action_record.side),
                    order_quantity(config, action_record),
                    order_type,
                    &client_order_id,
                    arrival,
                    &reason,
                )
                .await;
                return if retryable {
                    LiveActionExecutionOutcome::retryable(reason)
                } else {
                    LiveActionExecutionOutcome::terminal(reason)
                };
            }
        };
        let quantity = entry_quantity.base_quantity;
        let order_side = entry_order_side(&action_record.side);
        let (risk_passed, risk_reason, risk_retryable) =
            match Self::exchange_runtime_risk_state(config, private_client).await {
                Ok(risk_state) => {
                    let (passed, reason) = Self::check_risk_controls_with_state(
                        config,
                        &action_record.side,
                        action_record.price,
                        quantity,
                        &risk_state,
                    );
                    (passed, reason, false)
                }
                Err(error) => (false, format!("获取 OKX 风控状态失败: {error}"), true),
            };
        if !risk_passed {
            let order_status = if risk_retryable {
                "risk_check_failed"
            } else {
                "risk_blocked"
            };
            self.log_execution_stage(
                run_id,
                "risk",
                if risk_retryable { "error" } else { "warn" },
                if risk_retryable {
                    format!("风控状态读取失败，暂不提交下单: {risk_reason}")
                } else {
                    format!("风控拦截下单: {risk_reason}")
                },
                serde_json::json!({
                    "symbol": config.symbol,
                    "side": action_record.side,
                    "price": action_record.price,
                    "quantity": quantity,
                    "reason": risk_reason,
                    "retryable": risk_retryable,
                }),
            )
            .await;
            let arrival = fetch_arrival_quote(client, config).await;
            match insert_live_order(
                db,
                config,
                order_side,
                quantity,
                action_record.price,
                &action_record.action,
                order_status,
                false,
                &risk_reason,
                run_id,
                action_record.timestamp,
                arrival,
            )
            .await
            {
                Ok(_) => {
                    let mut status = self.inner.status.write().await;
                    if status.run_id == run_id {
                        status.total_orders += 1;
                        status.failed_orders += 1;
                        apply_blocked_order_status(
                            &mut status,
                            action_record,
                            order_status,
                            &risk_reason,
                        );
                    }
                }
                Err(error) => {
                    self.set_error(run_id, format!("保存实时策略风控拦截记录失败: {error}"))
                        .await;
                }
            }
            return if risk_retryable {
                LiveActionExecutionOutcome::retryable(risk_reason)
            } else {
                LiveActionExecutionOutcome::terminal(risk_reason)
            };
        }
        self.log_execution_stage(
            run_id,
            "risk",
            "success",
            "交易所风控状态检查通过",
            serde_json::json!({
                "symbol": config.symbol,
                "side": action_record.side,
                "price": action_record.price,
                "quantity": quantity,
            }),
        )
        .await;

        let arrival = fetch_arrival_quote(client, config).await;
        let (slippage_passed, slippage_reason) =
            check_slippage_control(config, order_side, action_record.price, arrival);
        if !slippage_passed {
            self.log_execution_stage(
                run_id,
                "slippage",
                "warn",
                format!("滑点控制拦截下单: {slippage_reason}"),
                serde_json::json!({
                    "symbol": config.symbol,
                    "order_side": order_side,
                    "reference_price": action_record.price,
                    "arrival_mid_px": arrival.mid_px,
                    "arrival_bid_px": arrival.bid_px,
                    "arrival_ask_px": arrival.ask_px,
                    "reason": slippage_reason,
                }),
            )
            .await;
            match insert_live_order(
                db,
                config,
                order_side,
                quantity,
                action_record.price,
                &action_record.action,
                "slippage_blocked",
                false,
                &slippage_reason,
                run_id,
                action_record.timestamp,
                arrival,
            )
            .await
            {
                Ok(_) => {
                    let mut status = self.inner.status.write().await;
                    if status.run_id == run_id {
                        status.total_orders += 1;
                        status.failed_orders += 1;
                        apply_blocked_order_status(
                            &mut status,
                            action_record,
                            "slippage_blocked",
                            &slippage_reason,
                        );
                    }
                }
                Err(error) => {
                    self.set_error(run_id, format!("保存实时策略滑点拦截记录失败: {error}"))
                        .await;
                }
            }
            return LiveActionExecutionOutcome::terminal(slippage_reason);
        }
        self.log_execution_stage(
            run_id,
            "slippage",
            "success",
            "滑点控制检查通过",
            serde_json::json!({
                "symbol": config.symbol,
                "order_side": order_side,
                "reference_price": action_record.price,
                "arrival_mid_px": arrival.mid_px,
                "arrival_bid_px": arrival.bid_px,
                "arrival_ask_px": arrival.ask_px,
            }),
        )
        .await;

        let exchange_quantity = entry_quantity.exchange_quantity;
        if let Err(error) = validate_exchange_order_price(
            client,
            config,
            order_type,
            order_side,
            action_record.price,
        )
        .await
        {
            let retryable = retryable_for_app_error(&error);
            let reason = error.to_string();
            self.log_execution_stage(
                run_id,
                "price",
                "error",
                format!("校验交易所下单价格失败: {reason}"),
                serde_json::json!({
                    "symbol": config.symbol,
                    "order_side": order_side,
                    "order_type": order_type,
                    "price": action_record.price,
                }),
            )
            .await;
            let client_order_id = live_strategy_client_order_id();
            self.record_exchange_submit_failure(
                run_id,
                config,
                db,
                action_record,
                order_side,
                exchange_quantity,
                order_type,
                &client_order_id,
                arrival,
                &reason,
            )
            .await;
            return if retryable {
                LiveActionExecutionOutcome::retryable(reason)
            } else {
                LiveActionExecutionOutcome::terminal(reason)
            };
        }
        self.log_execution_stage(
            run_id,
            "quantity",
            "success",
            if entry_quantity.explicit_exchange_size {
                format!("使用策略显式 OKX 下单数量 {:.8}", exchange_quantity)
            } else {
                format!("交易所下单数量已换算为 {:.8}", exchange_quantity)
            },
            serde_json::json!({
                "symbol": config.symbol,
                "base_quantity": quantity,
                "exchange_quantity": exchange_quantity,
                "explicit_exchange_size": entry_quantity.explicit_exchange_size,
            }),
        )
        .await;

        let outcome = self
            .submit_exchange_order(
                run_id,
                config,
                db,
                private_client,
                action_record,
                order_side,
                order_type,
                exchange_quantity,
                false,
                arrival,
                attached_risk_orders,
                client,
                None,
                None,
            )
            .await;
        match outcome {
            ExchangeSubmitOutcome::Submitted(entry_order)
            | ExchangeSubmitOutcome::SubmittedUnknown(entry_order) => {
                if let Some(planned_exit) = planned_exit {
                    self.persist_open_planned_exit(
                        run_id,
                        config,
                        db,
                        action_record,
                        order_side,
                        planned_exit,
                        &entry_order,
                    )
                    .await;
                }
                LiveActionExecutionOutcome::Submitted
            }
            ExchangeSubmitOutcome::NotSubmitted { reason, retryable } => {
                LiveActionExecutionOutcome::NotSubmitted { reason, retryable }
            }
        }
    }
}

fn pre_quantity_validation_failure_size(
    config: &LiveStrategyConfig,
    action_record: &StrategyActionRecord,
) -> f64 {
    action_record
        .position_size
        .filter(|value| value.is_finite() && *value > 0.0)
        .or_else(|| {
            config
                .position_size
                .is_finite()
                .then_some(config.position_size)
                .filter(|value| *value > 0.0)
        })
        .unwrap_or(1.0)
}
