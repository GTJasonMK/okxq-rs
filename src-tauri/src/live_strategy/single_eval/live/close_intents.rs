use crate::{
    live_strategy::arrival::ArrivalQuote,
    trading_semantics::{close_quantity_limit_decision, CloseQuantityLimitDecision},
};

use super::*;

impl LiveStrategyRuntime {
    pub(super) async fn submit_exchange_close_intent(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        client: &OkxPublicClient,
        private_client: &OkxPrivateClient,
        action_record: &StrategyActionRecord,
        order_type: &str,
        close_order_side: Option<&str>,
        exchange_size: Option<&str>,
        client_order_id_override: Option<&str>,
        planned_exit_plan_id: Option<i64>,
        max_exchange_quantity_override: Option<f64>,
    ) -> ExchangeSubmitOutcome {
        let preview_quantity = order_quantity(config, action_record);
        if let Err(error) = td_mode_from_config(config) {
            let reason = error.to_string();
            let record_side = close_order_side
                .map(str::trim)
                .filter(|side| !side.is_empty())
                .unwrap_or("flat");
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
            let client_order_id = live_strategy_client_order_id();
            self.record_exchange_submit_failure(
                run_id,
                config,
                db,
                action_record,
                record_side,
                preview_quantity,
                order_type,
                &client_order_id,
                ArrivalQuote::default(),
                &reason,
            )
            .await;
            return ExchangeSubmitOutcome::terminal(reason);
        }
        let arrival = fetch_arrival_quote(client, config).await;
        let mut close_order =
            match resolve_close_exchange_order(private_client, config, close_order_side).await {
                Ok(quantity) => quantity,
                Err(error) => {
                    let reason = error.to_string();
                    let record_side = close_order_side
                        .map(str::trim)
                        .filter(|side| !side.is_empty())
                        .unwrap_or("flat");
                    self.log_execution_stage(
                        run_id,
                        "close",
                        "error",
                        format!("解析交易所可平仓数量失败: {reason}"),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "requested_order_side": close_order_side,
                        }),
                    )
                    .await;
                    let client_order_id = live_strategy_client_order_id();
                    self.record_exchange_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        record_side,
                        preview_quantity,
                        order_type,
                        &client_order_id,
                        arrival,
                        &reason,
                    )
                    .await;
                    return if retryable_for_close_resolution_error(&error, &reason) {
                        ExchangeSubmitOutcome::retryable(reason)
                    } else {
                        ExchangeSubmitOutcome::terminal(reason)
                    };
                }
            };
        if let Some(max_quantity) = max_exchange_quantity_override {
            if !max_quantity.is_finite() || max_quantity <= 0.0 {
                let reason = format!(
                    "计划退出剩余可平数量无效: {}",
                    order_size_string(max_quantity)
                );
                self.log_execution_stage(
                    run_id,
                    "close",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_side": close_order.order_side,
                        "available_close_quantity": close_order.quantity,
                        "max_exchange_quantity": max_quantity,
                        "planned_exit_plan_id": planned_exit_plan_id,
                    }),
                )
                .await;
                let client_order_id = live_strategy_client_order_id();
                self.record_exchange_submit_failure(
                    run_id,
                    config,
                    db,
                    action_record,
                    &close_order.order_side,
                    preview_quantity,
                    order_type,
                    &client_order_id,
                    arrival,
                    &reason,
                )
                .await;
                return ExchangeSubmitOutcome::terminal(reason);
            }
            match close_quantity_limit_decision(close_order.quantity, max_quantity, false) {
                Ok(CloseQuantityLimitDecision::CapTo(quantity)) => {
                    self.log_execution_stage(
                        run_id,
                        "close",
                        "info",
                        format!(
                            "按计划退出入口剩余成交量限定平仓数量: {} -> {}",
                            order_size_string(close_order.quantity),
                            order_size_string(quantity)
                        ),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_side": close_order.order_side,
                            "available_close_quantity": close_order.quantity,
                            "max_exchange_quantity": max_quantity,
                            "planned_exit_plan_id": planned_exit_plan_id,
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
                        "close",
                        "error",
                        reason.clone(),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_side": close_order.order_side,
                            "available_close_quantity": close_order.quantity,
                            "max_exchange_quantity": max_quantity,
                            "planned_exit_plan_id": planned_exit_plan_id,
                        }),
                    )
                    .await;
                    let client_order_id = live_strategy_client_order_id();
                    self.record_exchange_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        &close_order.order_side,
                        preview_quantity,
                        order_type,
                        &client_order_id,
                        arrival,
                        &reason,
                    )
                    .await;
                    return ExchangeSubmitOutcome::terminal(reason);
                }
            }
        } else if let Some(exchange_size) = exchange_size
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let explicit_quantity = match resolve_explicit_exchange_order_size(
                client,
                config,
                exchange_size,
                "策略显式平仓 exchange_size",
                "已拒绝平仓以避免静默改量",
            )
            .await
            {
                Ok(quantity) => quantity,
                Err(error) => {
                    let reason = error.to_string();
                    self.log_execution_stage(
                        run_id,
                        "close",
                        "error",
                        reason.clone(),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "order_side": close_order.order_side,
                            "exchange_size": exchange_size,
                        }),
                    )
                    .await;
                    let client_order_id = live_strategy_client_order_id();
                    self.record_exchange_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        &close_order.order_side,
                        preview_quantity,
                        order_type,
                        &client_order_id,
                        arrival,
                        &reason,
                    )
                    .await;
                    return if retryable_for_app_error(&error) {
                        ExchangeSubmitOutcome::retryable(reason)
                    } else {
                        ExchangeSubmitOutcome::terminal(reason)
                    };
                }
            };
            match close_quantity_limit_decision(close_order.quantity, explicit_quantity, true) {
                Ok(CloseQuantityLimitDecision::RejectAboveAvailable) => {
                    let reason = format!(
                        "策略显式平仓 exchange_size={} 大于 OKX 当前可平数量 {}，已拒绝平仓以避免静默改量",
                        order_size_string(explicit_quantity),
                        order_size_string(close_order.quantity)
                    );
                    self.log_execution_stage(
                        run_id,
                        "close",
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
                    let client_order_id = live_strategy_client_order_id();
                    self.record_exchange_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        &close_order.order_side,
                        preview_quantity,
                        order_type,
                        &client_order_id,
                        arrival,
                        &reason,
                    )
                    .await;
                    return ExchangeSubmitOutcome::terminal(reason);
                }
                Ok(CloseQuantityLimitDecision::CapTo(quantity)) => {
                    self.log_execution_stage(
                        run_id,
                        "close",
                        "info",
                        format!(
                            "按策略显式 exchange_size 限定平仓数量: {} -> {}",
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
                        "close",
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
                    let client_order_id = live_strategy_client_order_id();
                    self.record_exchange_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        &close_order.order_side,
                        preview_quantity,
                        order_type,
                        &client_order_id,
                        arrival,
                        &reason,
                    )
                    .await;
                    return ExchangeSubmitOutcome::terminal(reason);
                }
            }
        } else if planned_exit_plan_id.is_none() && action_record.position_size.is_some() {
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
                    let reason = format!("解析策略请求平仓数量失败: {error}");
                    self.log_execution_stage(
                        run_id,
                        "close",
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
                    let client_order_id = live_strategy_client_order_id();
                    self.record_exchange_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        &close_order.order_side,
                        preview_quantity,
                        order_type,
                        &client_order_id,
                        arrival,
                        &reason,
                    )
                    .await;
                    return if retryable_for_app_error(&error) {
                        ExchangeSubmitOutcome::retryable(reason)
                    } else {
                        ExchangeSubmitOutcome::terminal(reason)
                    };
                }
            };
            match close_quantity_limit_decision(close_order.quantity, requested_quantity, false) {
                Ok(CloseQuantityLimitDecision::CapTo(quantity)) => {
                    self.log_execution_stage(
                        run_id,
                        "close",
                        "info",
                        format!(
                            "按策略 position_size 限定平仓数量: {} -> {}",
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
                        "close",
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
                    let client_order_id = live_strategy_client_order_id();
                    self.record_exchange_submit_failure(
                        run_id,
                        config,
                        db,
                        action_record,
                        &close_order.order_side,
                        preview_quantity,
                        order_type,
                        &client_order_id,
                        arrival,
                        &reason,
                    )
                    .await;
                    return ExchangeSubmitOutcome::terminal(reason);
                }
            }
        }
        let order_side = close_order.order_side.as_str();
        if close_order_side
            .map(str::trim)
            .filter(|side| !side.is_empty())
            .is_none()
        {
            self.log_execution_stage(
                run_id,
                "close",
                "info",
                format!(
                    "已根据 OKX 当前持仓推断平仓方向: {} {} {}",
                    config.symbol, order_side, close_order.quantity
                ),
                serde_json::json!({
                    "symbol": config.symbol,
                    "order_side": order_side,
                    "quantity": close_order.quantity,
                }),
            )
            .await;
        }
        if let Err(error) = validate_exchange_order_price(
            client,
            config,
            order_type,
            order_side,
            action_record.price,
        )
        .await
        {
            let reason = error.to_string();
            self.log_execution_stage(
                run_id,
                "price",
                "error",
                format!("校验交易所平仓价格失败: {reason}"),
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
                close_order.quantity,
                order_type,
                &client_order_id,
                arrival,
                &reason,
            )
            .await;
            return if retryable_for_app_error(&error) {
                ExchangeSubmitOutcome::retryable(reason)
            } else {
                ExchangeSubmitOutcome::terminal(reason)
            };
        }
        self.submit_exchange_order(
            run_id,
            config,
            db,
            private_client,
            action_record,
            order_side,
            order_type,
            close_order.quantity,
            true,
            arrival,
            &[],
            client,
            client_order_id_override,
            planned_exit_plan_id,
        )
        .await
    }
}
