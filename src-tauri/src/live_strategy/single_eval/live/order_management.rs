use super::*;

impl LiveStrategyRuntime {
    pub(super) async fn submit_exchange_cancel_intent(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        private_client: &OkxPrivateClient,
        action_record: &StrategyActionRecord,
        cancel_order: Option<&StrategyCancelOrderIntent>,
    ) -> ExchangeSubmitOutcome {
        let Some(cancel_order) = cancel_order else {
            let reason = "cancel_order 动作缺少可撤订单身份".to_string();
            self.log_execution_stage(
                run_id,
                "cancel",
                "error",
                reason.clone(),
                serde_json::json!({ "symbol": config.symbol }),
            )
            .await;
            return ExchangeSubmitOutcome::terminal(reason);
        };
        if cancel_order.target_kind.allows_algo() {
            let algo_lookup = if cancel_order.scope_explicit {
                query_live_algo_order_identity_context_for_symbol(
                    db,
                    &config.mode,
                    &config.symbol,
                    &cancel_order.order_id,
                    &cancel_order.client_order_id,
                )
                .await
            } else {
                query_live_algo_order_identity_context(
                    db,
                    &config.mode,
                    &cancel_order.order_id,
                    &cancel_order.client_order_id,
                )
                .await
            };
            match algo_lookup {
                Ok(Some(algo_order)) => {
                    if cancel_order.target_kind == StrategyOrderTargetKind::Any {
                        if let Some(outcome) = self
                            .reject_any_target_kind_collision_if_exchange_order_exists(
                                run_id,
                                config,
                                db,
                                "cancel",
                                "cancel_order",
                                cancel_order.scope_explicit,
                                &cancel_order.order_id,
                                &cancel_order.client_order_id,
                            )
                            .await
                        {
                            return outcome;
                        }
                    }
                    let config = config_for_algo_order(config, &algo_order);
                    return self
                        .submit_exchange_cancel_algo_intent(
                            run_id,
                            &config,
                            db,
                            private_client,
                            action_record,
                            cancel_order,
                        )
                        .await;
                }
                Ok(None) if cancel_order.target_kind == StrategyOrderTargetKind::Algo => {
                    if !cancel_order.scope_explicit {
                        let reason = "cancel_order target_order_kind=algo 但本地未找到目标保护单，且 action 未显式提供 symbol，已拒绝撤保护单以避免使用启动交易对误撤"
                            .to_string();
                        self.log_execution_stage(
                            run_id,
                            "cancel",
                            "error",
                            reason.clone(),
                            serde_json::json!({
                                "symbol": config.symbol,
                                "inst_type": config.inst_type,
                                "order_id": cancel_order.order_id,
                                "client_order_id": cancel_order.client_order_id,
                            }),
                        )
                        .await;
                        self.set_error(run_id, reason.clone()).await;
                        return ExchangeSubmitOutcome::terminal(reason);
                    }
                    return self
                        .submit_exchange_cancel_algo_intent(
                            run_id,
                            config,
                            db,
                            private_client,
                            action_record,
                            cancel_order,
                        )
                        .await;
                }
                Ok(None) => {}
                Err(error) => {
                    let reason = format!("查询本地 OKX 保护单失败，已拒绝撤单: {error}");
                    let retryable = retryable_for_app_error(&error);
                    self.log_execution_stage(
                        run_id,
                        "cancel",
                        "error",
                        reason.clone(),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_id": cancel_order.order_id,
                            "client_order_id": cancel_order.client_order_id,
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
            }
        }
        if !cancel_order.target_kind.allows_exchange() {
            let reason =
                "cancel_order target_order_kind=algo 未能解析为可撤保护单，已拒绝改走普通撤单接口"
                    .to_string();
            self.log_execution_stage(
                run_id,
                "cancel",
                "error",
                reason.clone(),
                serde_json::json!({
                    "symbol": config.symbol,
                    "order_id": cancel_order.order_id,
                    "client_order_id": cancel_order.client_order_id,
                }),
            )
            .await;
            self.set_error(run_id, reason.clone()).await;
            return ExchangeSubmitOutcome::terminal(reason);
        }
        let order_lookup = if cancel_order.scope_explicit {
            query_live_order_identity_context_for_symbol(
                db,
                &config.mode,
                &config.symbol,
                &cancel_order.order_id,
                &cancel_order.client_order_id,
            )
            .await
        } else {
            query_live_order_identity_context(
                db,
                &config.mode,
                &cancel_order.order_id,
                &cancel_order.client_order_id,
            )
            .await
        };
        let config = match order_lookup {
            Ok(Some(order)) => config_for_live_order(config, &order),
            Ok(None) if cancel_order.scope_explicit => config.clone(),
            Ok(None) => {
                let reason = "cancel_order 本地未找到目标订单，且 action 未显式提供 symbol，已拒绝撤单以避免使用启动交易对误撤"
                    .to_string();
                self.log_execution_stage(
                    run_id,
                    "cancel",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "inst_type": config.inst_type,
                        "order_id": cancel_order.order_id,
                        "client_order_id": cancel_order.client_order_id,
                    }),
                )
                .await;
                self.set_error(run_id, reason.clone()).await;
                return ExchangeSubmitOutcome::terminal(reason);
            }
            Err(error) => {
                let reason = format!("查询本地 OKX 普通订单失败，已拒绝撤单: {error}");
                let retryable = retryable_for_app_error(&error);
                self.log_execution_stage(
                    run_id,
                    "cancel",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_id": cancel_order.order_id,
                        "client_order_id": cancel_order.client_order_id,
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
        };
        self.log_execution_stage(
            run_id,
            "cancel",
            "info",
            "准备提交 OKX 撤单请求",
            serde_json::json!({
                "symbol": config.symbol,
                "order_id": cancel_order.order_id,
                "client_order_id": cancel_order.client_order_id,
                "target_order_kind": cancel_order.target_kind.as_str(),
                "scope_explicit": cancel_order.scope_explicit,
                "timestamp": action_record.timestamp,
            }),
        )
        .await;

        let result = private_client
            .cancel_order(
                &config.symbol,
                &cancel_order.order_id,
                &cancel_order.client_order_id,
            )
            .await;
        match result {
            Ok(response) => {
                let response_order_id = response_text(&response, "ordId");
                let response_client_order_id = response_text(&response, "clOrdId");
                let order_id = if response_order_id.trim().is_empty() {
                    cancel_order.order_id.clone()
                } else {
                    response_order_id
                };
                let client_order_id = if response_client_order_id.trim().is_empty() {
                    cancel_order.client_order_id.clone()
                } else {
                    response_client_order_id
                };
                let message = "OKX 撤单请求已提交，真实终态以后续订单回报为准".to_string();
                let changed = update_live_exchange_order_state_by_identity_and_symbol(
                    db,
                    &config.mode,
                    &config.symbol,
                    &order_id,
                    &client_order_id,
                    &LiveOrderExchangeState {
                        status: "cancel_requested".to_string(),
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
                                "cancel",
                                "cancel_order",
                                "market",
                                "cancel_requested",
                                &order_id,
                                &client_order_id,
                                &message,
                                true,
                            )
                            .await;
                        }
                        self.log_execution_stage(
                            run_id,
                            "cancel",
                            "success",
                            message.clone(),
                            serde_json::json!({
                                "symbol": config.symbol,
                                "order_id": order_id,
                                "client_order_id": client_order_id,
                                "target_order_kind": cancel_order.target_kind.as_str(),
                                "scope_explicit": cancel_order.scope_explicit,
                                "local_order_records_changed": changed,
                            }),
                        )
                        .await;
                        let mut status = self.inner.status.write().await;
                        if status.run_id == run_id {
                            status.last_action = "cancel_requested".to_string();
                            status.last_action_reason = message;
                            status.error_message.clear();
                        }
                        ExchangeSubmitOutcome::Submitted(SubmittedExchangeOrder {
                            order_id,
                            client_order_id,
                        })
                    }
                    Err(error) => {
                        let reason = format!("OKX 已接受撤单但更新本地订单状态失败: {error}");
                        self.log_execution_stage(
                            run_id,
                            "cancel",
                            "error",
                            reason.clone(),
                            serde_json::json!({
                                "symbol": config.symbol,
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
                    format!("提交 OKX 撤单后响应结果待确认: {error}")
                } else {
                    format!("提交 OKX 撤单失败: {error}")
                };
                self.log_execution_stage(
                    run_id,
                    "cancel",
                    if request_unknown { "warn" } else { "error" },
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_id": cancel_order.order_id,
                        "client_order_id": cancel_order.client_order_id,
                        "target_order_kind": cancel_order.target_kind.as_str(),
                        "scope_explicit": cancel_order.scope_explicit,
                    }),
                )
                .await;
                if request_unknown {
                    self.record_order_management_request_unknown(
                        run_id,
                        &config,
                        db,
                        action_record,
                        "cancel",
                        "cancel_order",
                        "market",
                        "cancel_requested",
                        &cancel_order.order_id,
                        &cancel_order.client_order_id,
                        &reason,
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_id": cancel_order.order_id,
                            "client_order_id": cancel_order.client_order_id,
                        }),
                    )
                    .await;
                    ExchangeSubmitOutcome::SubmittedUnknown(SubmittedExchangeOrder {
                        order_id: cancel_order.order_id.clone(),
                        client_order_id: cancel_order.client_order_id.clone(),
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

    pub(super) async fn submit_exchange_modify_intent(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        client: &OkxPublicClient,
        private_client: &OkxPrivateClient,
        action_record: &StrategyActionRecord,
        modify_order: Option<&StrategyModifyOrderIntent>,
    ) -> ExchangeSubmitOutcome {
        let Some(modify_order) = modify_order else {
            let reason = "modify_order 动作缺少可改单订单身份或改单参数".to_string();
            self.log_execution_stage(
                run_id,
                "modify",
                "error",
                reason.clone(),
                serde_json::json!({ "symbol": config.symbol }),
            )
            .await;
            return ExchangeSubmitOutcome::terminal(reason);
        };
        if modify_order.target_kind.allows_algo() {
            let algo_lookup = if modify_order.scope_explicit {
                query_live_algo_order_identity_context_for_symbol(
                    db,
                    &config.mode,
                    &config.symbol,
                    &modify_order.order_id,
                    &modify_order.client_order_id,
                )
                .await
            } else {
                query_live_algo_order_identity_context(
                    db,
                    &config.mode,
                    &modify_order.order_id,
                    &modify_order.client_order_id,
                )
                .await
            };
            match algo_lookup {
                Ok(Some(algo_order)) => {
                    if modify_order.target_kind == StrategyOrderTargetKind::Any {
                        if let Some(outcome) = self
                            .reject_any_target_kind_collision_if_exchange_order_exists(
                                run_id,
                                config,
                                db,
                                "modify",
                                "modify_order",
                                modify_order.scope_explicit,
                                &modify_order.order_id,
                                &modify_order.client_order_id,
                            )
                            .await
                        {
                            return outcome;
                        }
                    }
                    return self
                        .submit_exchange_modify_algo_intent(
                            run_id,
                            config,
                            db,
                            client,
                            private_client,
                            action_record,
                            modify_order,
                            algo_order,
                        )
                        .await;
                }
                Ok(None) if modify_order.target_kind == StrategyOrderTargetKind::Algo => {
                    if !modify_order.scope_explicit {
                        let reason = "modify_order target_order_kind=algo 但本地未找到目标保护单，且 action 未显式提供 symbol，已拒绝改保护单以避免使用启动交易对误改"
                            .to_string();
                        self.log_execution_stage(
                            run_id,
                            "modify",
                            "error",
                            reason.clone(),
                            serde_json::json!({
                                "symbol": config.symbol,
                                "inst_type": config.inst_type,
                                "order_id": modify_order.order_id,
                                "client_order_id": modify_order.client_order_id,
                            }),
                        )
                        .await;
                        self.set_error(run_id, reason.clone()).await;
                        return ExchangeSubmitOutcome::terminal(reason);
                    }
                    let algo_order = match external_algo_order_context(config, modify_order) {
                        Ok(algo_order) => algo_order,
                        Err(reason) => {
                            self.log_execution_stage(
                                run_id,
                                "modify",
                                "error",
                                reason.clone(),
                                serde_json::json!({
                                    "symbol": config.symbol,
                                    "inst_type": config.inst_type,
                                    "order_id": modify_order.order_id,
                                    "client_order_id": modify_order.client_order_id,
                                }),
                            )
                            .await;
                            self.set_error(run_id, reason.clone()).await;
                            return ExchangeSubmitOutcome::terminal(reason);
                        }
                    };
                    return self
                        .submit_exchange_modify_algo_intent(
                            run_id,
                            config,
                            db,
                            client,
                            private_client,
                            action_record,
                            modify_order,
                            algo_order,
                        )
                        .await;
                }
                Ok(None) => {}
                Err(error) => {
                    let reason = format!("查询本地 OKX 保护单失败，已拒绝改单: {error}");
                    let retryable = retryable_for_app_error(&error);
                    self.log_execution_stage(
                        run_id,
                        "modify",
                        "error",
                        reason.clone(),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_id": modify_order.order_id,
                            "client_order_id": modify_order.client_order_id,
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
            }
        }
        if !modify_order.target_kind.allows_exchange() {
            let reason =
                "modify_order target_order_kind=algo 未能解析为可改保护单，已拒绝改走普通改单接口"
                    .to_string();
            self.log_execution_stage(
                run_id,
                "modify",
                "error",
                reason.clone(),
                serde_json::json!({
                    "symbol": config.symbol,
                    "order_id": modify_order.order_id,
                    "client_order_id": modify_order.client_order_id,
                }),
            )
            .await;
            self.set_error(run_id, reason.clone()).await;
            return ExchangeSubmitOutcome::terminal(reason);
        }
        let order_lookup = if modify_order.scope_explicit {
            query_live_order_identity_context_for_symbol(
                db,
                &config.mode,
                &config.symbol,
                &modify_order.order_id,
                &modify_order.client_order_id,
            )
            .await
        } else {
            query_live_order_identity_context(
                db,
                &config.mode,
                &modify_order.order_id,
                &modify_order.client_order_id,
            )
            .await
        };
        let config = match order_lookup {
            Ok(Some(order)) => config_for_live_order(config, &order),
            Ok(None) if modify_order.scope_explicit => config.clone(),
            Ok(None) => {
                let reason = "modify_order 本地未找到目标订单，且 action 未显式提供 symbol，已拒绝改单以避免使用启动交易对误改"
                    .to_string();
                self.log_execution_stage(
                    run_id,
                    "modify",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "inst_type": config.inst_type,
                        "order_id": modify_order.order_id,
                        "client_order_id": modify_order.client_order_id,
                    }),
                )
                .await;
                self.set_error(run_id, reason.clone()).await;
                return ExchangeSubmitOutcome::terminal(reason);
            }
            Err(error) => {
                let reason = format!("查询本地 OKX 普通订单失败，已拒绝改单: {error}");
                let retryable = retryable_for_app_error(&error);
                self.log_execution_stage(
                    run_id,
                    "modify",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_id": modify_order.order_id,
                        "client_order_id": modify_order.client_order_id,
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
        };
        if let Err(error) = validate_exchange_modify_order(client, &config, modify_order).await {
            let retryable = retryable_for_app_error(&error);
            let reason = error.to_string();
            self.log_execution_stage(
                run_id,
                "modify",
                "error",
                format!("校验 OKX 改单参数失败: {reason}"),
                serde_json::json!({
                    "symbol": config.symbol,
                    "order_id": modify_order.order_id,
                    "client_order_id": modify_order.client_order_id,
                    "new_size": modify_order.new_size.as_deref(),
                    "new_price": modify_order.new_price.as_deref(),
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
            "准备提交 OKX 改单请求",
            serde_json::json!({
                "symbol": config.symbol,
                "order_id": modify_order.order_id,
                "client_order_id": modify_order.client_order_id,
                "new_size": modify_order.new_size.as_deref(),
                "new_price": modify_order.new_price.as_deref(),
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
            .amend_order(
                &config.symbol,
                &modify_order.order_id,
                &modify_order.client_order_id,
                modify_order.new_size.as_deref().unwrap_or(""),
                modify_order.new_price.as_deref().unwrap_or(""),
                modify_order.cancel_on_fail,
                &modify_order.request_id,
            )
            .await;
        match result {
            Ok(response) => {
                let response_order_id = response_text(&response, "ordId");
                let response_client_order_id = response_text(&response, "clOrdId");
                let order_id = if response_order_id.trim().is_empty() {
                    modify_order.order_id.clone()
                } else {
                    response_order_id
                };
                let client_order_id = if response_client_order_id.trim().is_empty() {
                    modify_order.client_order_id.clone()
                } else {
                    response_client_order_id
                };
                let message = "OKX 改单请求已提交，真实终态以后续订单回报为准".to_string();
                let changed = update_live_exchange_order_state_by_identity_and_symbol(
                    db,
                    &config.mode,
                    &config.symbol,
                    &order_id,
                    &client_order_id,
                    &LiveOrderExchangeState {
                        status: "modify_requested".to_string(),
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
                                "modify_order",
                                "market",
                                "modify_requested",
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
                                "order_id": order_id,
                                "client_order_id": client_order_id,
                                "new_size": modify_order.new_size.as_deref(),
                                "new_price": modify_order.new_price.as_deref(),
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
                            status.last_action = "modify_requested".to_string();
                            status.last_action_reason = message;
                            status.error_message.clear();
                        }
                        ExchangeSubmitOutcome::Submitted(SubmittedExchangeOrder {
                            order_id,
                            client_order_id,
                        })
                    }
                    Err(error) => {
                        let reason = format!("OKX 已接受改单但更新本地订单状态失败: {error}");
                        self.log_execution_stage(
                            run_id,
                            "modify",
                            "error",
                            reason.clone(),
                            serde_json::json!({
                                "symbol": config.symbol,
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
                    format!("提交 OKX 改单后响应结果待确认: {error}")
                } else {
                    format!("提交 OKX 改单失败: {error}")
                };
                self.log_execution_stage(
                    run_id,
                    "modify",
                    if request_unknown { "warn" } else { "error" },
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_id": modify_order.order_id,
                        "client_order_id": modify_order.client_order_id,
                        "new_size": modify_order.new_size.as_deref(),
                        "new_price": modify_order.new_price.as_deref(),
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
                        "modify_order",
                        "market",
                        "modify_requested",
                        &modify_order.order_id,
                        &modify_order.client_order_id,
                        &reason,
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_id": modify_order.order_id,
                            "client_order_id": modify_order.client_order_id,
                            "new_size": modify_order.new_size.as_deref(),
                            "new_price": modify_order.new_price.as_deref(),
                            "cancel_on_fail": modify_order.cancel_on_fail,
                            "request_id": modify_order.request_id,
                            "target_order_kind": modify_order.target_kind.as_str(),
                            "target_order_type": modify_order.target_order_type.as_deref(),
                            "scope_explicit": modify_order.scope_explicit,
                        }),
                    )
                    .await;
                    ExchangeSubmitOutcome::SubmittedUnknown(SubmittedExchangeOrder {
                        order_id: modify_order.order_id.clone(),
                        client_order_id: modify_order.client_order_id.clone(),
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

    #[allow(clippy::too_many_arguments)]
    async fn reject_any_target_kind_collision_if_exchange_order_exists(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        stage: &str,
        action_name: &str,
        scope_explicit: bool,
        order_id: &str,
        client_order_id: &str,
    ) -> Option<ExchangeSubmitOutcome> {
        let exchange_lookup = if scope_explicit {
            query_live_order_identity_context_for_symbol(
                db,
                &config.mode,
                &config.symbol,
                order_id,
                client_order_id,
            )
            .await
        } else {
            query_live_order_identity_context(db, &config.mode, order_id, client_order_id).await
        };
        match exchange_lookup {
            Ok(Some(order)) => {
                let reason = order_management_any_target_kind_collision_reason(action_name);
                self.log_execution_stage(
                    run_id,
                    stage,
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "matched_exchange_symbol": order.symbol,
                        "order_id": order_id,
                        "client_order_id": client_order_id,
                        "target_order_kind": "any",
                    }),
                )
                .await;
                self.set_error(run_id, reason.clone()).await;
                Some(ExchangeSubmitOutcome::terminal(reason))
            }
            Ok(None) => None,
            Err(error) => {
                let reason = format!(
                    "校验 {action_name} target_order_kind=any 是否同时命中普通订单失败，已拒绝执行: {error}"
                );
                let retryable = retryable_for_app_error(&error);
                self.log_execution_stage(
                    run_id,
                    stage,
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_id": order_id,
                        "client_order_id": client_order_id,
                        "target_order_kind": "any",
                    }),
                )
                .await;
                self.set_error(run_id, reason.clone()).await;
                Some(if retryable {
                    ExchangeSubmitOutcome::retryable(reason)
                } else {
                    ExchangeSubmitOutcome::terminal(reason)
                })
            }
        }
    }
}

fn external_algo_order_context(
    config: &LiveStrategyConfig,
    modify_order: &StrategyModifyOrderIntent,
) -> Result<LiveAlgoOrderIdentityContext, String> {
    let Some(order_type) = modify_order
        .target_order_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err("modify_order target_order_kind=algo 且本地未找到目标保护单时必须提供 target_order_type"
            .to_string());
    };
    let order_type = normalize_runtime_order_type_text(order_type);
    if live_algo_target_order_kind_from_order_type(&order_type).is_none() {
        return Err(format!(
            "modify_order target_order_type={} 不受支持，必须使用可映射为 OKX 止盈/止损改单字段的市价保护单类型",
            order_type
        ));
    }
    Ok(LiveAlgoOrderIdentityContext {
        id: 0,
        symbol: config.symbol.clone(),
        inst_type: config.inst_type.clone(),
        order_type: order_type.to_string(),
        order_id: modify_order.order_id.clone(),
        client_order_id: modify_order.client_order_id.clone(),
        status: "external".to_string(),
    })
}
