use super::*;

impl LiveStrategyRuntime {
    pub(super) async fn submit_exchange_cancel_algo_intent(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        private_client: &OkxPrivateClient,
        action_record: &StrategyActionRecord,
        cancel_order: &StrategyCancelOrderIntent,
    ) -> ExchangeSubmitOutcome {
        let algo_id = cancel_order.order_id.trim();
        let algo_client_order_id = cancel_order.client_order_id.trim();
        if algo_id.is_empty() && algo_client_order_id.is_empty() {
            let reason =
                "cancel_order 目标是 OKX 保护单，但缺少 algoId/algoClOrdId，无法撤销策略委托"
                    .to_string();
            self.log_execution_stage(
                run_id,
                "cancel_algo",
                "error",
                reason.clone(),
                serde_json::json!({
                    "symbol": config.symbol,
                    "client_order_id": cancel_order.client_order_id,
                }),
            )
            .await;
            self.set_error(run_id, reason.clone()).await;
            return ExchangeSubmitOutcome::terminal(reason);
        }
        self.log_execution_stage(
            run_id,
            "cancel_algo",
            "info",
            "准备提交 OKX 保护单撤销请求",
            serde_json::json!({
                "symbol": config.symbol,
                "algo_id": algo_id,
                "algo_client_order_id": cancel_order.client_order_id,
                "target_order_kind": cancel_order.target_kind.as_str(),
                "scope_explicit": cancel_order.scope_explicit,
            }),
        )
        .await;

        let result = private_client
            .cancel_algo_order(&config.symbol, algo_id, algo_client_order_id)
            .await;
        match result {
            Ok(response) => {
                let response_algo_id = response_text(&response, "algoId");
                let order_id = if response_algo_id.trim().is_empty() {
                    algo_id.to_string()
                } else {
                    response_algo_id
                };
                let response_client_order_id = response_text(&response, "algoClOrdId");
                let client_order_id = if response_client_order_id.trim().is_empty() {
                    algo_client_order_id.to_string()
                } else {
                    response_client_order_id
                };
                let message =
                    "OKX 保护单撤销请求已提交，真实终态以后续策略委托同步为准".to_string();
                let changed = update_live_algo_order_exchange_state_by_identity_and_symbol(
                    db,
                    &config.mode,
                    &config.symbol,
                    &order_id,
                    &client_order_id,
                    &LiveOrderExchangeState {
                        status: "algo_cancel_requested".to_string(),
                        success: true,
                        error_message: message.clone(),
                        order_id: order_id.clone(),
                        client_order_id: client_order_id.clone(),
                    },
                )
                .await;
                match changed {
                    Ok(changed) => {
                        if changed == 0 {
                            self.record_order_management_sync_candidate(
                                run_id,
                                config,
                                db,
                                action_record,
                                "cancel_algo",
                                "place_risk_order",
                                "conditional",
                                "algo_cancel_requested",
                                &order_id,
                                &client_order_id,
                                &message,
                                true,
                            )
                            .await;
                        }
                        self.log_execution_stage(
                            run_id,
                            "cancel_algo",
                            "success",
                            message.clone(),
                            serde_json::json!({
                                "symbol": config.symbol,
                                "algo_id": order_id,
                                "algo_client_order_id": client_order_id,
                                "target_order_kind": cancel_order.target_kind.as_str(),
                                "scope_explicit": cancel_order.scope_explicit,
                                "local_order_records_changed": changed,
                            }),
                        )
                        .await;
                        let mut status = self.inner.status.write().await;
                        if status.run_id == run_id {
                            status.last_action = "algo_cancel_requested".to_string();
                            status.last_action_reason = message;
                            status.error_message.clear();
                        }
                        ExchangeSubmitOutcome::Submitted(SubmittedExchangeOrder {
                            order_id,
                            client_order_id,
                        })
                    }
                    Err(error) => {
                        let reason = format!("OKX 已接受保护单撤销但更新本地状态失败: {error}");
                        self.log_execution_stage(
                            run_id,
                            "cancel_algo",
                            "error",
                            reason.clone(),
                            serde_json::json!({
                                "symbol": config.symbol,
                                "algo_id": order_id,
                                "algo_client_order_id": client_order_id,
                            }),
                        )
                        .await;
                        self.set_error(run_id, reason.clone()).await;
                        ExchangeSubmitOutcome::Submitted(SubmittedExchangeOrder {
                            order_id,
                            client_order_id,
                        })
                    }
                }
            }
            Err(error) => {
                let request_unknown = is_unknown_okx_trade_request_error(&error);
                let reason = if request_unknown {
                    format!("提交 OKX 保护单撤销后响应结果待确认: {error}")
                } else {
                    format!("提交 OKX 保护单撤销失败: {error}")
                };
                self.log_execution_stage(
                    run_id,
                    "cancel_algo",
                    if request_unknown { "warn" } else { "error" },
                    reason.clone(),
                    serde_json::json!({
                            "symbol": config.symbol,
                        "algo_id": algo_id,
                        "algo_client_order_id": algo_client_order_id,
                        "target_order_kind": cancel_order.target_kind.as_str(),
                        "scope_explicit": cancel_order.scope_explicit,
                    }),
                )
                .await;
                if request_unknown {
                    self.record_order_management_request_unknown(
                        run_id,
                        config,
                        db,
                        action_record,
                        "cancel_algo",
                        "place_risk_order",
                        "conditional",
                        "algo_cancel_requested",
                        algo_id,
                        algo_client_order_id,
                        &reason,
                        serde_json::json!({
                            "symbol": config.symbol,
                            "algo_id": algo_id,
                            "algo_client_order_id": algo_client_order_id,
                        }),
                    )
                    .await;
                    ExchangeSubmitOutcome::SubmittedUnknown(SubmittedExchangeOrder {
                        order_id: algo_id.to_string(),
                        client_order_id: algo_client_order_id.to_string(),
                    })
                } else {
                    self.set_error(run_id, reason.clone()).await;
                    if retryable_for_app_error(&error) {
                        ExchangeSubmitOutcome::retryable(reason)
                    } else {
                        ExchangeSubmitOutcome::terminal(reason)
                    }
                }
            }
        }
    }

    pub(super) async fn submit_exchange_modify_algo_intent(
        &self,
        run_id: &str,
        base_config: &LiveStrategyConfig,
        db: &SqlitePool,
        client: &OkxPublicClient,
        private_client: &OkxPrivateClient,
        action_record: &StrategyActionRecord,
        modify_order: &StrategyModifyOrderIntent,
        algo_order: LiveAlgoOrderIdentityContext,
    ) -> ExchangeSubmitOutcome {
        let config = config_for_algo_order(base_config, &algo_order);
        let kind = match attached_risk_kind_from_order_type(&algo_order.order_type) {
            Some(kind) => kind,
            None => {
                let reason = format!(
                    "modify_order 目标保护单 order_type={} 无法映射为 OKX 止盈/止损改单字段",
                    algo_order.order_type
                );
                self.log_execution_stage(
                    run_id,
                    "modify",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "local_order_id": algo_order.id,
                        "order_id": algo_order.order_id,
                        "client_order_id": algo_order.client_order_id,
                        "status": algo_order.status,
                    }),
                )
                .await;
                self.set_error(run_id, reason.clone()).await;
                return ExchangeSubmitOutcome::terminal(reason);
            }
        };
        if let Err(error) = validate_exchange_algo_modify_order(client, &config, modify_order).await
        {
            let retryable = retryable_for_app_error(&error);
            let reason = error.to_string();
            self.log_execution_stage(
                run_id,
                "modify",
                "error",
                format!("校验 OKX 保护单改单参数失败: {reason}"),
                serde_json::json!({
                    "symbol": config.symbol,
                    "local_order_id": algo_order.id,
                    "order_id": algo_order.order_id,
                    "client_order_id": algo_order.client_order_id,
                    "order_type": algo_order.order_type,
                    "new_size": modify_order.new_size.as_deref(),
                    "new_trigger_price": modify_order.new_price.as_deref(),
                }),
            )
            .await;
            self.set_error(run_id, reason.clone()).await;
            return if retryable {
                ExchangeSubmitOutcome::retryable(reason)
            } else {
                ExchangeSubmitOutcome::terminal(reason)
            };
        }
        self.log_execution_stage(
            run_id,
            "modify",
            "info",
            "准备提交 OKX 保护单改单请求",
            serde_json::json!({
                "symbol": config.symbol,
                "local_order_id": algo_order.id,
                "order_id": algo_order.order_id,
                "client_order_id": algo_order.client_order_id,
                "order_type": algo_order.order_type,
                "status": algo_order.status,
                "new_size": modify_order.new_size.as_deref(),
                "new_trigger_price": modify_order.new_price.as_deref(),
                "cancel_on_fail": modify_order.cancel_on_fail,
                "request_id": modify_order.request_id,
                "target_order_kind": modify_order.target_kind.as_str(),
                "target_order_type": modify_order.target_order_type.as_deref(),
                "scope_explicit": modify_order.scope_explicit,
                "timestamp": action_record.timestamp,
            }),
        )
        .await;

        let result = private_client
            .amend_algo_order(
                &config.symbol,
                &algo_order.order_id,
                &algo_order.client_order_id,
                modify_order.new_size.as_deref().unwrap_or(""),
                modify_order.new_price.as_deref().unwrap_or(""),
                matches!(kind, ProtectiveOrderKind::TakeProfit),
                modify_order.cancel_on_fail,
                &modify_order.request_id,
            )
            .await;
        match result {
            Ok(response) => {
                let response_order_id = response_text(&response, "algoId");
                let response_client_order_id = response_text(&response, "algoClOrdId");
                let order_id = if response_order_id.trim().is_empty() {
                    algo_order.order_id.clone()
                } else {
                    response_order_id
                };
                let client_order_id = if response_client_order_id.trim().is_empty() {
                    algo_order.client_order_id.clone()
                } else {
                    response_client_order_id
                };
                let message =
                    "OKX 保护单改单请求已提交，真实终态以后续策略委托回报为准".to_string();
                let changed = update_live_algo_order_exchange_state_by_identity_and_symbol(
                    db,
                    &config.mode,
                    &config.symbol,
                    &order_id,
                    &client_order_id,
                    &LiveOrderExchangeState {
                        status: "algo_modify_requested".to_string(),
                        success: true,
                        error_message: message.clone(),
                        order_id: order_id.clone(),
                        client_order_id: client_order_id.clone(),
                    },
                )
                .await;
                match changed {
                    Ok(changed) => {
                        if changed == 0 {
                            self.record_order_management_sync_candidate(
                                run_id,
                                &config,
                                db,
                                action_record,
                                "modify",
                                "place_risk_order",
                                &algo_order.order_type,
                                "algo_modify_requested",
                                &order_id,
                                &client_order_id,
                                &message,
                                true,
                            )
                            .await;
                        }
                        self.log_execution_stage(
                            run_id,
                            "modify",
                            "success",
                            message.clone(),
                            serde_json::json!({
                                "symbol": config.symbol,
                                "local_order_id": algo_order.id,
                                "order_id": order_id,
                                "client_order_id": client_order_id,
                                "new_size": modify_order.new_size.as_deref(),
                                "new_trigger_price": modify_order.new_price.as_deref(),
                                "cancel_on_fail": modify_order.cancel_on_fail,
                                "request_id": modify_order.request_id,
                                "target_order_kind": modify_order.target_kind.as_str(),
                                "target_order_type": modify_order.target_order_type.as_deref(),
                                "scope_explicit": modify_order.scope_explicit,
                                "local_order_records_changed": changed,
                            }),
                        )
                        .await;
                        let mut status = self.inner.status.write().await;
                        if status.run_id == run_id {
                            status.last_action = "algo_modify_requested".to_string();
                            status.last_action_reason = message;
                            status.error_message.clear();
                        }
                        ExchangeSubmitOutcome::Submitted(SubmittedExchangeOrder {
                            order_id,
                            client_order_id,
                        })
                    }
                    Err(error) => {
                        let reason = format!("OKX 已接受保护单改单但更新本地订单状态失败: {error}");
                        self.log_execution_stage(
                            run_id,
                            "modify",
                            "error",
                            reason.clone(),
                            serde_json::json!({
                                "symbol": config.symbol,
                                "local_order_id": algo_order.id,
                                "order_id": order_id,
                                "client_order_id": client_order_id,
                            }),
                        )
                        .await;
                        self.set_error(run_id, reason.clone()).await;
                        ExchangeSubmitOutcome::Submitted(SubmittedExchangeOrder {
                            order_id,
                            client_order_id,
                        })
                    }
                }
            }
            Err(error) => {
                let request_unknown = is_unknown_okx_trade_request_error(&error);
                let reason = if request_unknown {
                    format!("提交 OKX 保护单改单后响应结果待确认: {error}")
                } else {
                    format!("提交 OKX 保护单改单失败: {error}")
                };
                self.log_execution_stage(
                    run_id,
                    "modify",
                    if request_unknown { "warn" } else { "error" },
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "local_order_id": algo_order.id,
                        "order_id": algo_order.order_id,
                        "client_order_id": algo_order.client_order_id,
                        "order_type": algo_order.order_type,
                        "new_size": modify_order.new_size.as_deref(),
                        "new_trigger_price": modify_order.new_price.as_deref(),
                        "cancel_on_fail": modify_order.cancel_on_fail,
                        "request_id": modify_order.request_id,
                        "target_order_kind": modify_order.target_kind.as_str(),
                        "target_order_type": modify_order.target_order_type.as_deref(),
                        "scope_explicit": modify_order.scope_explicit,
                    }),
                )
                .await;
                if request_unknown {
                    self.record_order_management_request_unknown(
                        run_id,
                        &config,
                        db,
                        action_record,
                        "modify",
                        "place_risk_order",
                        &algo_order.order_type,
                        "algo_modify_requested",
                        &algo_order.order_id,
                        &algo_order.client_order_id,
                        &reason,
                        serde_json::json!({
                            "symbol": config.symbol,
                            "local_order_id": algo_order.id,
                            "order_id": algo_order.order_id,
                            "client_order_id": algo_order.client_order_id,
                            "order_type": algo_order.order_type,
                            "new_size": modify_order.new_size.as_deref(),
                            "new_trigger_price": modify_order.new_price.as_deref(),
                            "cancel_on_fail": modify_order.cancel_on_fail,
                            "request_id": modify_order.request_id,
                            "target_order_kind": modify_order.target_kind.as_str(),
                            "target_order_type": modify_order.target_order_type.as_deref(),
                            "scope_explicit": modify_order.scope_explicit,
                        }),
                    )
                    .await;
                    ExchangeSubmitOutcome::SubmittedUnknown(SubmittedExchangeOrder {
                        order_id: algo_order.order_id,
                        client_order_id: algo_order.client_order_id,
                    })
                } else {
                    self.set_error(run_id, reason.clone()).await;
                    if retryable_for_app_error(&error) {
                        ExchangeSubmitOutcome::retryable(reason)
                    } else {
                        ExchangeSubmitOutcome::terminal(reason)
                    }
                }
            }
        }
    }
}
