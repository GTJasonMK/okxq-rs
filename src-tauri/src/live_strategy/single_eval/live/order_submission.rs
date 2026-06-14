use crate::live_strategy::arrival::ArrivalQuote;

use super::*;

impl LiveStrategyRuntime {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn submit_exchange_order(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        private_client: &OkxPrivateClient,
        action_record: &StrategyActionRecord,
        order_side: &str,
        requested_order_type: &str,
        quantity: f64,
        reduce_only: bool,
        arrival: ArrivalQuote,
        attached_risk_orders: &[StrategyRiskOrderIntent],
        client: &OkxPublicClient,
        client_order_id_override: Option<&str>,
        planned_exit_plan_id: Option<i64>,
    ) -> ExchangeSubmitOutcome {
        let client_order_id = client_order_id_override
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(live_strategy_client_order_id);
        let requested_order_type = normalized_requested_order_type(requested_order_type);
        let order_type = match normalized_exchange_order_type(&requested_order_type) {
            Ok(value) => value.to_string(),
            Err(error) => {
                let reason = error.to_string();
                self.log_execution_stage(
                    run_id,
                    "submit",
                    "error",
                    format!("策略动作包含不支持的 OKX 订单类型，已拒绝提交: {reason}"),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "requested_order_type": requested_order_type,
                        "order_side": order_side,
                        "reduce_only": reduce_only,
                    }),
                )
                .await;
                self.record_exchange_submit_failure(
                    run_id,
                    config,
                    db,
                    action_record,
                    order_side,
                    quantity,
                    &requested_order_type,
                    &client_order_id,
                    arrival,
                    &reason,
                )
                .await;
                return ExchangeSubmitOutcome::terminal(reason);
            }
        };
        let requires_price = match exchange_order_type_requires_price(&order_type) {
            Ok(value) => value,
            Err(error) => {
                let reason = error.to_string();
                self.log_execution_stage(
                    run_id,
                    "submit",
                    "error",
                    format!("解析 OKX 订单类型价格规则失败，已拒绝提交: {reason}"),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_type": order_type,
                        "order_side": order_side,
                        "reduce_only": reduce_only,
                    }),
                )
                .await;
                self.record_exchange_submit_failure(
                    run_id,
                    config,
                    db,
                    action_record,
                    order_side,
                    quantity,
                    &order_type,
                    &client_order_id,
                    arrival,
                    &reason,
                )
                .await;
                return ExchangeSubmitOutcome::terminal(reason);
            }
        };
        if requires_price && (!action_record.price.is_finite() || action_record.price <= 0.0) {
            let reason = format!("OKX {} 订单需要有效价格，当前策略价格无效", order_type);
            self.log_execution_stage(
                run_id,
                "submit",
                "error",
                reason.clone(),
                serde_json::json!({
                    "symbol": config.symbol,
                    "order_type": order_type,
                    "order_side": order_side,
                    "price": action_record.price,
                    "reduce_only": reduce_only,
                }),
            )
            .await;
            self.record_exchange_submit_failure(
                run_id,
                config,
                db,
                action_record,
                order_side,
                quantity,
                &order_type,
                &client_order_id,
                arrival,
                &reason,
            )
            .await;
            return ExchangeSubmitOutcome::terminal(reason);
        }
        let order_price = order_price_string_for_order_type(&order_type, action_record.price);
        let size = order_size_string(quantity);
        let td_mode = match td_mode_from_config(config) {
            Ok(value) => value,
            Err(error) => {
                let reason = error.to_string();
                self.log_execution_stage(
                    run_id,
                    "submit",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "inst_type": config.inst_type,
                        "order_side": order_side,
                        "reduce_only": reduce_only,
                    }),
                )
                .await;
                self.record_exchange_submit_failure(
                    run_id,
                    config,
                    db,
                    action_record,
                    order_side,
                    quantity,
                    &order_type,
                    &client_order_id,
                    arrival,
                    &reason,
                )
                .await;
                return ExchangeSubmitOutcome::terminal(reason);
            }
        };
        let mut attached_algos = if reduce_only {
            Vec::new()
        } else {
            let instrument_rules = if attached_risk_orders.is_empty() {
                None
            } else {
                match fetch_instrument_order_rules(client, config).await {
                    Ok(rules) => Some(rules),
                    Err(error) => {
                        let retryable = retryable_for_app_error(&error);
                        let reason = error.to_string();
                        self.record_exchange_submit_failure(
                            run_id,
                            config,
                            db,
                            action_record,
                            order_side,
                            quantity,
                            &order_type,
                            &client_order_id,
                            arrival,
                            &reason,
                        )
                        .await;
                        return if retryable {
                            ExchangeSubmitOutcome::retryable(reason)
                        } else {
                            ExchangeSubmitOutcome::terminal(reason)
                        };
                    }
                }
            };
            match attached_algo_orders_from_risk_orders(
                attached_risk_orders,
                &config.symbol,
                order_side,
                action_record.price,
                instrument_rules.as_ref(),
            ) {
                Ok(algos) => algos,
                Err(error) => {
                    let reason = error.to_string();
                    self.record_exchange_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        order_side,
                        quantity,
                        &order_type,
                        &client_order_id,
                        arrival,
                        &reason,
                    )
                    .await;
                    return ExchangeSubmitOutcome::terminal(reason);
                }
            }
        };
        let attached_algo_client_order_ids = attached_algos
            .iter_mut()
            .map(|algo| {
                let client_order_id = live_strategy_client_order_id();
                algo.attach_algo_client_order_id = Some(client_order_id.clone());
                client_order_id
            })
            .collect::<Vec<_>>();
        self.log_execution_stage(
            run_id,
            "submit",
            "info",
            format!(
                "准备提交 OKX {}单: {} {} {}",
                if reduce_only { "平仓" } else { "开仓" },
                config.symbol,
                order_side,
                size
            ),
            serde_json::json!({
                "symbol": config.symbol,
                "td_mode": td_mode,
                "order_side": order_side,
                "order_type": order_type,
                "order_price": order_price,
                "size": size,
                "reduce_only": reduce_only,
                "attached_risk_orders": attached_algos.len(),
            }),
        )
        .await;
        let order_context =
            match resolve_exchange_order_context(private_client, config, order_side, reduce_only)
                .await
            {
                Ok(context) => context,
                Err(error) => {
                    let retryable = retryable_for_app_error(&error);
                    let reason = error.to_string();
                    self.log_execution_stage(
                        run_id,
                        "submit",
                        "error",
                        format!("解析 OKX 下单上下文失败: {reason}"),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_side": order_side,
                            "reduce_only": reduce_only,
                        }),
                    )
                    .await;
                    self.record_exchange_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        order_side,
                        quantity,
                        &order_type,
                        &client_order_id,
                        arrival,
                        &reason,
                    )
                    .await;
                    return if retryable {
                        ExchangeSubmitOutcome::retryable(reason)
                    } else {
                        ExchangeSubmitOutcome::terminal(reason)
                    };
                }
            };
        if !reduce_only {
            let confirmed_leverage = match self
                .ensure_exchange_leverage(config, private_client, &td_mode, &order_context.pos_side)
                .await
            {
                Ok(value) => value,
                Err(error) => {
                    let retryable = retryable_for_app_error(&error);
                    let reason = error.to_string();
                    self.log_execution_stage(
                        run_id,
                        "leverage",
                        "error",
                        reason.clone(),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "td_mode": td_mode,
                            "pos_side": order_context.pos_side,
                        }),
                    )
                    .await;
                    self.record_exchange_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        order_side,
                        quantity,
                        &order_type,
                        &client_order_id,
                        arrival,
                        &reason,
                    )
                    .await;
                    return if retryable {
                        ExchangeSubmitOutcome::retryable(reason)
                    } else {
                        ExchangeSubmitOutcome::terminal(reason)
                    };
                }
            };
            if let Some(leverage) = confirmed_leverage {
                self.log_execution_stage(
                    run_id,
                    "leverage",
                    "success",
                    format!("OKX 杠杆已确认: {}x", order_size_string(leverage)),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "td_mode": td_mode,
                        "pos_side": order_context.pos_side,
                        "leverage": leverage,
                    }),
                )
                .await;
            }
        }
        if let Some(plan_id) = planned_exit_plan_id {
            match mark_live_planned_exit_submitting(db, plan_id, run_id, &client_order_id).await {
                Ok(true) => {}
                Ok(false) => {
                    let reason = format!("计划退出状态已变化，拒绝提交平仓单: plan_id={plan_id}");
                    self.log_execution_stage(
                        run_id,
                        "planned_exit",
                        "warn",
                        reason.clone(),
                        serde_json::json!({
                            "plan_id": plan_id,
                            "client_order_id": client_order_id,
                        }),
                    )
                    .await;
                    return ExchangeSubmitOutcome::terminal(reason);
                }
                Err(error) => {
                    let reason = format!("预登记计划退出平仓单失败，已拒绝提交 OKX: {error}");
                    self.log_execution_stage(
                        run_id,
                        "planned_exit",
                        "error",
                        reason.clone(),
                        serde_json::json!({
                            "plan_id": plan_id,
                            "client_order_id": client_order_id,
                        }),
                    )
                    .await;
                    self.set_error(run_id, reason.clone()).await;
                    return ExchangeSubmitOutcome::retryable(reason);
                }
            }
        }
        if let Err(reason) = self
            .pre_register_exchange_order(
                run_id,
                config,
                db,
                action_record,
                order_side,
                &order_type,
                quantity,
                arrival,
                &client_order_id,
            )
            .await
        {
            return ExchangeSubmitOutcome::retryable(reason);
        }
        self.log_execution_stage(
            run_id,
            "submit",
            "info",
            "已通过下单前检查，正在提交 OKX 订单",
            serde_json::json!({
                "symbol": config.symbol,
                "td_mode": td_mode,
                "pos_side": order_context.pos_side,
                "reduce_only": order_context.reduce_only,
                "order_type": order_type,
                "order_price": order_price,
                "client_order_id": client_order_id,
            }),
        )
        .await;
        let result = private_client
            .place_order_with_attached_algos(
                &config.symbol,
                &td_mode,
                order_side,
                &order_type,
                &size,
                &order_price,
                &order_context.pos_side,
                order_context.reduce_only,
                &client_order_id,
                &attached_algos,
            )
            .await;
        match result {
            Ok(response) => {
                if let Some(error_message) = order_submit_error(&response) {
                    self.log_execution_stage(
                        run_id,
                        "submit",
                        "error",
                        error_message.clone(),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "client_order_id": client_order_id,
                        }),
                    )
                    .await;
                    self.record_exchange_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        order_side,
                        quantity,
                        &order_type,
                        &client_order_id,
                        arrival,
                        &error_message,
                    )
                    .await;
                    return ExchangeSubmitOutcome::terminal(error_message);
                }
                let order_id = response_text(&response, "ordId");
                let response_client_order_id = response_text(&response, "clOrdId");
                let recorded_client_order_id = if response_client_order_id.trim().is_empty() {
                    client_order_id
                } else {
                    response_client_order_id
                };
                let submit_message = if attached_algos.is_empty() {
                    "OKX 订单已提交，成交和费用以后续交易所回报为准".to_string()
                } else {
                    format!(
                        "OKX 订单已提交，已随单附加 {} 个保护单，成交和费用以后续交易所回报为准",
                        attached_algos.len()
                    )
                };
                match self
                    .record_exchange_submit_success(
                        run_id,
                        config,
                        db,
                        action_record,
                        order_side,
                        quantity,
                        &order_type,
                        &order_id,
                        &recorded_client_order_id,
                        &submit_message,
                        arrival,
                    )
                    .await
                {
                    Ok(_) => {
                        self.record_attached_algo_orders_after_parent_submit(
                            run_id,
                            config,
                            db,
                            action_record,
                            quantity,
                            arrival,
                            attached_risk_orders,
                            &attached_algos,
                            &attached_algo_client_order_ids,
                            &order_id,
                            &recorded_client_order_id,
                        )
                        .await;
                        self.log_execution_stage(
                            run_id,
                            "submit",
                            "success",
                            submit_message.clone(),
                            serde_json::json!({
                                "symbol": config.symbol,
                                "order_id": order_id,
                                "client_order_id": recorded_client_order_id,
                                "order_side": order_side,
                                "size": quantity,
                                "attached_risk_orders": attached_algos.len(),
                            }),
                        )
                        .await;
                        let mut status = self.inner.status.write().await;
                        if status.run_id == run_id {
                            status.total_orders += 1;
                            status.successful_orders += 1;
                            status.last_action = "submitted".to_string();
                            status.last_action_reason = format!(
                                "OKX order submitted; side={order_side}; size={size}; attached_risk_orders={}; clOrdId={recorded_client_order_id}",
                                attached_algos.len()
                            );
                            status.error_message.clear();
                        }
                        ExchangeSubmitOutcome::Submitted(SubmittedExchangeOrder {
                            order_id,
                            client_order_id: recorded_client_order_id,
                        })
                    }
                    Err(reason) => {
                        self.log_execution_stage(
                            run_id,
                            "persist",
                            "error",
                            format!("保存实时策略交易所订单记录失败: {reason}"),
                            serde_json::json!({
                                "symbol": config.symbol,
                                "order_side": order_side,
                            }),
                        )
                        .await;
                        self.set_error(run_id, format!("保存实时策略交易所订单记录失败: {reason}"))
                            .await;
                        ExchangeSubmitOutcome::Submitted(SubmittedExchangeOrder {
                            order_id,
                            client_order_id: recorded_client_order_id,
                        })
                    }
                }
            }
            Err(error) => {
                let reason = error.to_string();
                let submit_unknown = is_unknown_okx_trade_request_error(&error);
                self.log_execution_stage(
                    run_id,
                    "submit",
                    if submit_unknown { "warn" } else { "error" },
                    if submit_unknown {
                        format!("提交 OKX 订单后响应结果待确认: {reason}")
                    } else {
                        format!("提交 OKX 订单失败: {reason}")
                    },
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_side": order_side,
                        "size": quantity,
                    }),
                )
                .await;
                if submit_unknown {
                    self.record_exchange_submit_unknown(
                        run_id,
                        config,
                        db,
                        action_record,
                        order_side,
                        quantity,
                        &order_type,
                        &client_order_id,
                        arrival,
                        &reason,
                    )
                    .await;
                    ExchangeSubmitOutcome::SubmittedUnknown(SubmittedExchangeOrder {
                        order_id: String::new(),
                        client_order_id,
                    })
                } else {
                    let retryable = retryable_for_app_error(&error);
                    self.record_exchange_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        order_side,
                        quantity,
                        &order_type,
                        &client_order_id,
                        arrival,
                        &reason,
                    )
                    .await;
                    if retryable {
                        ExchangeSubmitOutcome::retryable(reason)
                    } else {
                        ExchangeSubmitOutcome::terminal(reason)
                    }
                }
            }
        }
    }
}
