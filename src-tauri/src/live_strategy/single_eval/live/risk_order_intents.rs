use crate::{
    live_strategy::arrival::ArrivalQuote,
    trading_semantics::{
        close_quantity_limit_decision, select_single_standalone_risk_order,
        standalone_risk_order_type, validate_standalone_risk_order_symbol,
        CloseQuantityLimitDecision, StandaloneRiskOrderSelectionError,
    },
};

use super::*;

impl LiveStrategyRuntime {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn submit_exchange_risk_order_intent(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        client: &OkxPublicClient,
        private_client: &OkxPrivateClient,
        action_record: &StrategyActionRecord,
        close_order_side: Option<&str>,
        exchange_size: Option<&str>,
        risk_orders: &[StrategyRiskOrderIntent],
    ) -> ExchangeSubmitOutcome {
        let preview_quantity = order_quantity(config, action_record);
        let order_type = risk_orders
            .first()
            .map(|risk| standalone_risk_order_type(&risk.order_type))
            .unwrap_or_else(|| standalone_risk_order_type(""));
        let risk = match select_single_standalone_risk_order(risk_orders) {
            Ok(risk) => risk,
            Err(StandaloneRiskOrderSelectionError::Missing) => {
                let reason = "place_risk_order 动作缺少保护单参数".to_string();
                self.log_execution_stage(
                    run_id,
                    "risk_order",
                    "error",
                    reason.clone(),
                    serde_json::json!({ "symbol": config.symbol }),
                )
                .await;
                return self
                    .record_risk_order_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        close_order_side.unwrap_or("flat"),
                        preview_quantity,
                        &order_type,
                        ArrivalQuote::default(),
                        &reason,
                        false,
                    )
                    .await;
            }
            Err(StandaloneRiskOrderSelectionError::Multiple { count }) => {
                let reason =
                    format!("place_risk_order 独立动作一次只支持一个保护单，当前收到 {count} 个");
                self.log_execution_stage(
                    run_id,
                    "risk_order",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "risk_action_count": count,
                    }),
                )
                .await;
                return self
                    .record_risk_order_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        close_order_side.unwrap_or("flat"),
                        preview_quantity,
                        &order_type,
                        ArrivalQuote::default(),
                        &reason,
                        false,
                    )
                    .await;
            }
        };
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
                    "requested_order_side": close_order_side,
                }),
            )
            .await;
            return self
                .record_risk_order_submit_failure(
                    run_id,
                    config,
                    db,
                    action_record,
                    close_order_side.unwrap_or("flat"),
                    preview_quantity,
                    &order_type,
                    ArrivalQuote::default(),
                    &reason,
                    false,
                )
                .await;
        }
        let arrival = fetch_arrival_quote(client, config).await;
        if let Err(error) = validate_standalone_risk_order_symbol(&risk.symbol, &config.symbol) {
            let reason = error.to_string();
            self.log_execution_stage(
                run_id,
                "risk_order",
                "error",
                reason.clone(),
                serde_json::json!({
                    "risk_symbol": risk.symbol,
                    "config_symbol": config.symbol,
                }),
            )
            .await;
            return self
                .record_risk_order_submit_failure(
                    run_id,
                    config,
                    db,
                    action_record,
                    close_order_side.unwrap_or("flat"),
                    preview_quantity,
                    &order_type,
                    arrival,
                    &reason,
                    false,
                )
                .await;
        }

        let requested_close_side = close_order_side
            .map(str::trim)
            .filter(|side| !side.is_empty())
            .or_else(|| {
                let side = risk.side.trim();
                (!side.is_empty()).then_some(side)
            });
        let mut close_order =
            match resolve_risk_close_exchange_order(private_client, config, requested_close_side)
                .await
            {
                Ok(order) => order,
                Err(error) => {
                    let reason = error.to_string();
                    self.log_execution_stage(
                        run_id,
                        "risk_order",
                        "error",
                        format!("解析保护单可平仓数量失败: {reason}"),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "requested_order_side": requested_close_side,
                        }),
                    )
                    .await;
                    return self
                        .record_risk_order_submit_failure(
                            run_id,
                            config,
                            db,
                            action_record,
                            requested_close_side.unwrap_or("flat"),
                            preview_quantity,
                            &order_type,
                            arrival,
                            &reason,
                            retryable_for_app_error(&error),
                        )
                        .await;
                }
            };
        if let Some(exchange_size) = exchange_size
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let explicit_quantity = match resolve_explicit_exchange_order_size(
                client,
                config,
                exchange_size,
                "策略显式保护单 exchange_size",
                "已拒绝保护单以避免静默改量",
            )
            .await
            {
                Ok(quantity) => quantity,
                Err(error) => {
                    let reason = error.to_string();
                    self.log_execution_stage(
                        run_id,
                        "risk_order",
                        "error",
                        reason.clone(),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_side": close_order.order_side,
                            "exchange_size": exchange_size,
                        }),
                    )
                    .await;
                    return self
                        .record_risk_order_submit_failure(
                            run_id,
                            config,
                            db,
                            action_record,
                            &close_order.order_side,
                            close_order.quantity,
                            &order_type,
                            arrival,
                            &reason,
                            retryable_for_app_error(&error),
                        )
                        .await;
                }
            };
            match close_quantity_limit_decision(close_order.quantity, explicit_quantity, true) {
                Ok(CloseQuantityLimitDecision::RejectAboveAvailable) => {
                    let reason = format!(
                        "策略显式保护单 exchange_size={} 大于 OKX 当前可平数量 {}，已拒绝保护单以避免静默改量",
                        order_size_string(explicit_quantity),
                        order_size_string(close_order.quantity)
                    );
                    self.log_execution_stage(
                        run_id,
                        "risk_order",
                        "error",
                        reason.clone(),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_side": close_order.order_side,
                            "available_close_quantity": close_order.quantity,
                            "exchange_size": explicit_quantity,
                        }),
                    )
                    .await;
                    return self
                        .record_risk_order_submit_failure(
                            run_id,
                            config,
                            db,
                            action_record,
                            &close_order.order_side,
                            close_order.quantity,
                            &order_type,
                            arrival,
                            &reason,
                            false,
                        )
                        .await;
                }
                Ok(CloseQuantityLimitDecision::CapTo(quantity)) => {
                    self.log_execution_stage(
                        run_id,
                        "risk_order",
                        "info",
                        format!(
                            "按策略显式 exchange_size 限定保护单数量: {} -> {}",
                            order_size_string(close_order.quantity),
                            order_size_string(quantity)
                        ),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_side": close_order.order_side,
                            "available_close_quantity": close_order.quantity,
                            "exchange_size": explicit_quantity,
                        }),
                    )
                    .await;
                    close_order.quantity = quantity;
                }
                Ok(CloseQuantityLimitDecision::UseAvailable) => {}
                Err(error) => {
                    let reason = error.to_string();
                    self.log_execution_stage(
                        run_id,
                        "risk_order",
                        "error",
                        reason.clone(),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_side": close_order.order_side,
                            "available_close_quantity": close_order.quantity,
                            "exchange_size": explicit_quantity,
                        }),
                    )
                    .await;
                    return self
                        .record_risk_order_submit_failure(
                            run_id,
                            config,
                            db,
                            action_record,
                            &close_order.order_side,
                            close_order.quantity,
                            &order_type,
                            arrival,
                            &reason,
                            false,
                        )
                        .await;
                }
            }
        } else if action_record.position_size.is_some() {
            let requested_quantity = match resolve_entry_exchange_quantity(
                client,
                config,
                preview_quantity,
                action_record.price,
            )
            .await
            {
                Ok(quantity) => quantity.exchange_quantity,
                Err(error) => {
                    let reason = format!("解析策略请求保护单数量失败: {error}");
                    self.log_execution_stage(
                        run_id,
                        "risk_order",
                        "error",
                        reason.clone(),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "requested_base_quantity": preview_quantity,
                            "reference_price": action_record.price,
                            "position_size": action_record.position_size,
                        }),
                    )
                    .await;
                    return self
                        .record_risk_order_submit_failure(
                            run_id,
                            config,
                            db,
                            action_record,
                            &close_order.order_side,
                            close_order.quantity,
                            &order_type,
                            arrival,
                            &reason,
                            retryable_for_app_error(&error),
                        )
                        .await;
                }
            };
            match close_quantity_limit_decision(close_order.quantity, requested_quantity, false) {
                Ok(CloseQuantityLimitDecision::CapTo(quantity)) => {
                    self.log_execution_stage(
                        run_id,
                        "risk_order",
                        "info",
                        format!(
                            "按策略 position_size 限定保护单数量: {} -> {}",
                            order_size_string(close_order.quantity),
                            order_size_string(quantity)
                        ),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_side": close_order.order_side,
                            "available_close_quantity": close_order.quantity,
                            "requested_close_quantity": requested_quantity,
                            "position_size": action_record.position_size,
                        }),
                    )
                    .await;
                    close_order.quantity = quantity;
                }
                Ok(
                    CloseQuantityLimitDecision::UseAvailable
                    | CloseQuantityLimitDecision::RejectAboveAvailable,
                ) => {}
                Err(error) => {
                    let reason = error.to_string();
                    self.log_execution_stage(
                        run_id,
                        "risk_order",
                        "error",
                        reason.clone(),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_side": close_order.order_side,
                            "available_close_quantity": close_order.quantity,
                            "requested_close_quantity": requested_quantity,
                            "position_size": action_record.position_size,
                        }),
                    )
                    .await;
                    return self
                        .record_risk_order_submit_failure(
                            run_id,
                            config,
                            db,
                            action_record,
                            &close_order.order_side,
                            close_order.quantity,
                            &order_type,
                            arrival,
                            &reason,
                            false,
                        )
                        .await;
                }
            }
        }

        let kind = match attached_risk_kind(risk) {
            Ok(kind) => kind,
            Err(error) => {
                let reason = error.to_string();
                self.log_execution_stage(
                    run_id,
                    "risk_order",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_type": order_type,
                    }),
                )
                .await;
                return self
                    .record_risk_order_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        &close_order.order_side,
                        close_order.quantity,
                        &order_type,
                        arrival,
                        &reason,
                        false,
                    )
                    .await;
            }
        };
        let trigger_price = match standalone_risk_trigger_price(kind, &close_order, risk) {
            Ok(price) => price,
            Err(error) => {
                let reason = error.to_string();
                self.log_execution_stage(
                    run_id,
                    "risk_order",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_side": close_order.order_side,
                        "average_entry_price": close_order.average_price,
                    }),
                )
                .await;
                return self
                    .record_risk_order_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        &close_order.order_side,
                        close_order.quantity,
                        &order_type,
                        arrival,
                        &reason,
                        false,
                    )
                    .await;
            }
        };
        let instrument_rules = match fetch_instrument_order_rules(client, config).await {
            Ok(rules) => rules,
            Err(error) => {
                let reason = error.to_string();
                self.log_execution_stage(
                    run_id,
                    "risk_order",
                    "error",
                    format!("获取交易规格失败，已拒绝保护单: {reason}"),
                    serde_json::json!({ "symbol": config.symbol }),
                )
                .await;
                return self
                    .record_risk_order_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        &close_order.order_side,
                        close_order.quantity,
                        &order_type,
                        arrival,
                        &reason,
                        retryable_for_app_error(&error),
                    )
                    .await;
            }
        };
        if let Err(error) = validate_price_matches_tick_size(
            &instrument_rules,
            trigger_price,
            "独立保护单触发价",
            "已拒绝提交独立保护单以避免静默改价",
        ) {
            let reason = error.to_string();
            self.log_execution_stage(
                run_id,
                "risk_order",
                "error",
                reason.clone(),
                serde_json::json!({
                    "symbol": config.symbol,
                    "trigger_price": trigger_price,
                }),
            )
            .await;
            return self
                .record_risk_order_submit_failure(
                    run_id,
                    config,
                    db,
                    action_record,
                    &close_order.order_side,
                    close_order.quantity,
                    &order_type,
                    arrival,
                    &reason,
                    false,
                )
                .await;
        }
        let trigger_price_text = order_size_string(trigger_price);
        let algo = match kind {
            ProtectiveOrderKind::StopLoss => {
                OkxAttachedAlgoOrder::stop_loss_market(trigger_price_text)
            }
            ProtectiveOrderKind::TakeProfit => {
                OkxAttachedAlgoOrder::take_profit_market(trigger_price_text)
            }
        };
        let td_mode = match td_mode_from_config(config) {
            Ok(value) => value,
            Err(error) => {
                let reason = error.to_string();
                self.log_execution_stage(
                    run_id,
                    "risk_order",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "inst_type": config.inst_type,
                        "order_side": close_order.order_side,
                    }),
                )
                .await;
                return self
                    .record_risk_order_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        &close_order.order_side,
                        close_order.quantity,
                        &order_type,
                        arrival,
                        &reason,
                        false,
                    )
                    .await;
            }
        };
        let order_context = match resolve_exchange_order_context(
            private_client,
            config,
            &close_order.order_side,
            true,
        )
        .await
        {
            Ok(context) => context,
            Err(error) => {
                let reason = error.to_string();
                self.log_execution_stage(
                    run_id,
                    "risk_order",
                    "error",
                    format!("解析 OKX 保护单上下文失败: {reason}"),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_side": close_order.order_side,
                    }),
                )
                .await;
                return self
                    .record_risk_order_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        &close_order.order_side,
                        close_order.quantity,
                        &order_type,
                        arrival,
                        &reason,
                        retryable_for_app_error(&error),
                    )
                    .await;
            }
        };
        let client_order_id = live_strategy_client_order_id();
        let size = order_size_string(close_order.quantity);
        if let Err(reason) = self
            .pre_register_exchange_algo_order(
                run_id,
                config,
                db,
                action_record,
                &close_order.order_side,
                &order_type,
                close_order.quantity,
                arrival,
                &client_order_id,
            )
            .await
        {
            return ExchangeSubmitOutcome::retryable(reason);
        }
        self.log_execution_stage(
            run_id,
            "risk_order",
            "info",
            format!(
                "准备提交 OKX 独立保护单: {} {} {}",
                config.symbol, close_order.order_side, size
            ),
            serde_json::json!({
                "symbol": config.symbol,
                "td_mode": td_mode,
                "pos_side": order_context.pos_side,
                "reduce_only": order_context.reduce_only,
                "order_side": close_order.order_side,
                "size": size,
                "trigger_price": trigger_price,
                "order_type": order_type,
                "client_order_id": client_order_id,
            }),
        )
        .await;
        let result = private_client
            .place_algo_order(
                &config.symbol,
                &td_mode,
                &close_order.order_side,
                &size,
                &order_context.pos_side,
                order_context.reduce_only,
                &client_order_id,
                &algo,
            )
            .await;
        match result {
            Ok(response) => {
                let algo_id = response_text(&response, "algoId");
                let response_client_order_id = response_text(&response, "algoClOrdId");
                let recorded_client_order_id = if response_client_order_id.trim().is_empty() {
                    client_order_id.clone()
                } else {
                    response_client_order_id
                };
                let message = "OKX 独立保护单已提交，触发和成交以后续交易所回报为准".to_string();
                match self
                    .record_algo_order_submit_success(
                        config,
                        db,
                        action_record,
                        &close_order.order_side,
                        close_order.quantity,
                        &order_type,
                        &algo_id,
                        &client_order_id,
                        &recorded_client_order_id,
                        &message,
                        run_id,
                        arrival,
                    )
                    .await
                {
                    Ok(()) => {
                        self.log_execution_stage(
                            run_id,
                            "risk_order",
                            "success",
                            message.clone(),
                            serde_json::json!({
                                "symbol": config.symbol,
                                "algo_id": algo_id,
                                "client_order_id": recorded_client_order_id,
                                "order_side": close_order.order_side,
                                "size": close_order.quantity,
                                "trigger_price": trigger_price,
                            }),
                        )
                        .await;
                        let mut status = self.inner.status.write().await;
                        if status.run_id == run_id {
                            status.total_orders += 1;
                            status.successful_orders += 1;
                            status.last_action = "algo_submitted".to_string();
                            status.last_action_reason = message;
                            status.error_message.clear();
                        }
                        ExchangeSubmitOutcome::Submitted(SubmittedExchangeOrder {
                            order_id: algo_id,
                            client_order_id: recorded_client_order_id,
                        })
                    }
                    Err(error) => {
                        let reason = format!("OKX 已接受保护单但保存本地记录失败: {error}");
                        self.log_execution_stage(
                            run_id,
                            "persist",
                            "error",
                            reason.clone(),
                            serde_json::json!({
                                "symbol": config.symbol,
                                "algo_id": algo_id,
                                "client_order_id": recorded_client_order_id,
                            }),
                        )
                        .await;
                        self.set_error(run_id, reason.clone()).await;
                        ExchangeSubmitOutcome::Submitted(SubmittedExchangeOrder {
                            order_id: algo_id,
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
                    "risk_order",
                    if submit_unknown { "warn" } else { "error" },
                    if submit_unknown {
                        format!("提交 OKX 独立保护单后响应结果待确认: {reason}")
                    } else {
                        format!("提交 OKX 独立保护单失败: {reason}")
                    },
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_side": close_order.order_side,
                        "size": close_order.quantity,
                    }),
                )
                .await;
                if submit_unknown {
                    self.record_algo_order_submit_unknown(
                        run_id,
                        config,
                        db,
                        action_record,
                        &close_order.order_side,
                        close_order.quantity,
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
                        &close_order.order_side,
                        close_order.quantity,
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

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn record_risk_order_submit_failure(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        action_record: &StrategyActionRecord,
        order_side: &str,
        quantity: f64,
        order_type: &str,
        arrival: ArrivalQuote,
        reason: &str,
        retryable: bool,
    ) -> ExchangeSubmitOutcome {
        let client_order_id = live_strategy_client_order_id();
        self.record_exchange_submit_failure(
            run_id,
            config,
            db,
            action_record,
            order_side,
            quantity,
            order_type,
            &client_order_id,
            arrival,
            reason,
        )
        .await;
        if retryable {
            ExchangeSubmitOutcome::retryable(reason.to_string())
        } else {
            ExchangeSubmitOutcome::terminal(reason.to_string())
        }
    }
}
