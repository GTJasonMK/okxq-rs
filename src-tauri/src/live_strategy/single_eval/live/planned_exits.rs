use super::*;

impl LiveStrategyRuntime {
    pub(super) async fn persist_open_planned_exit(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        action_record: &StrategyActionRecord,
        entry_side: &str,
        planned_exit: &StrategyPlannedExitIntent,
        entry_order: &SubmittedExchangeOrder,
    ) {
        let Some(close_side) = expected_protective_close_side(entry_side) else {
            self.log_execution_stage(
                run_id,
                "planned_exit",
                "error",
                format!("无法根据开仓方向 {entry_side} 推导计划退出平仓方向"),
                serde_json::json!({
                    "symbol": config.symbol,
                    "entry_side": entry_side,
                    "planned_exit_time": planned_exit.timestamp,
                }),
            )
            .await;
            return;
        };
        match insert_live_planned_exit_plan(
            db,
            config,
            run_id,
            action_record,
            entry_side,
            close_side,
            planned_exit,
            &entry_order.order_id,
            &entry_order.client_order_id,
        )
        .await
        {
            Ok(outcome) => {
                self.inner.planned_exit_notify.notify_one();
                self.log_execution_stage(
                    run_id,
                    "planned_exit",
                    if outcome.inserted { "success" } else { "info" },
                    if outcome.inserted {
                        format!(
                            "已登记计划退出: {} 将在 {} 平仓",
                            config.symbol, planned_exit.timestamp
                        )
                    } else {
                        format!("计划退出已存在，跳过重复登记: {}", config.symbol)
                    },
                    serde_json::json!({
                        "plan_id": outcome.id,
                        "inserted": outcome.inserted,
                        "symbol": config.symbol,
                        "entry_side": entry_side,
                        "close_side": close_side,
                        "planned_exit_time": planned_exit.timestamp,
                        "planned_exit_reason": planned_exit.reason,
                        "planned_exit_contract": planned_exit.contract,
                        "entry_order_id": entry_order.order_id,
                        "entry_client_order_id": entry_order.client_order_id,
                    }),
                )
                .await;
            }
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "planned_exit",
                    "error",
                    format!("保存计划退出失败: {error}"),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "planned_exit_time": planned_exit.timestamp,
                    }),
                )
                .await;
                self.set_error(run_id, format!("保存计划退出失败: {error}"))
                    .await;
            }
        }
    }

    pub(in crate::live_strategy) async fn process_due_planned_exits(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        client: &OkxPublicClient,
        private_client: &OkxPrivateClient,
    ) {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let plans =
            match query_due_live_planned_exits(db, &config.mode, &config.strategy_id, now_ms, 20)
                .await
            {
                Ok(plans) => plans,
                Err(error) => {
                    self.log_execution_stage(
                        run_id,
                        "planned_exit",
                        "error",
                        format!("读取到期计划退出失败: {error}"),
                        serde_json::json!({
                            "strategy_id": config.strategy_id,
                            "mode": config.mode,
                        }),
                    )
                    .await;
                    self.set_error(run_id, format!("读取到期计划退出失败: {error}"))
                        .await;
                    return;
                }
            };
        if plans.is_empty() {
            return;
        }
        self.log_execution_stage(
            run_id,
            "planned_exit",
            "info",
            format!("发现 {} 个到期计划退出，开始处理", plans.len()),
            serde_json::json!({
                "due_count": plans.len(),
                "now_ms": now_ms,
            }),
        )
        .await;
        for plan in plans {
            match claim_due_live_planned_exit(db, plan.id, now_ms).await {
                Ok(true) => {}
                Ok(false) => {
                    self.log_execution_stage(
                        run_id,
                        "planned_exit",
                        "info",
                        format!("计划退出已被其他调度路径处理，跳过: plan_id={}", plan.id),
                        serde_json::json!({
                            "plan_id": plan.id,
                            "symbol": plan.symbol,
                        }),
                    )
                    .await;
                    continue;
                }
                Err(error) => {
                    self.log_execution_stage(
                        run_id,
                        "planned_exit",
                        "error",
                        format!("抢占计划退出失败，已跳过本轮: {error}"),
                        serde_json::json!({
                            "plan_id": plan.id,
                            "symbol": plan.symbol,
                        }),
                    )
                    .await;
                    self.set_error(run_id, format!("抢占计划退出失败: {error}"))
                        .await;
                    continue;
                }
            }
            self.execute_due_planned_exit(run_id, config, db, client, private_client, plan, now_ms)
                .await;
        }
    }

    pub(super) async fn execute_due_planned_exit(
        &self,
        run_id: &str,
        base_config: &LiveStrategyConfig,
        db: &SqlitePool,
        client: &OkxPublicClient,
        private_client: &OkxPrivateClient,
        plan: LivePlannedExitPlan,
        now_ms: i64,
    ) {
        if !matches!(
            plan.close_side.trim().to_ascii_lowercase().as_str(),
            "buy" | "sell"
        ) {
            let reason = format!("计划退出缺少有效平仓方向: {}", plan.close_side);
            self.mark_planned_exit_skipped(db, run_id, plan.id, "skipped_invalid_plan", &reason)
                .await;
            return;
        }
        let exit_config = config_for_planned_exit(base_config, &plan);
        let fill_scope = match planned_exit_fill_scope(db, &plan).await {
            Ok(scope) => scope,
            Err(error) => {
                let reason = format!("计算计划退出入口剩余成交量失败: {error}");
                self.log_execution_stage(
                    run_id,
                    "planned_exit",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "plan_id": plan.id,
                        "symbol": exit_config.symbol,
                        "entry_order_id": plan.entry_order_id,
                        "entry_client_order_id": plan.entry_client_order_id,
                        "exit_order_id": plan.exit_order_id,
                        "exit_client_order_id": plan.exit_client_order_id,
                    }),
                )
                .await;
                self.set_error(run_id, reason.clone()).await;
                self.mark_planned_exit_retry(
                    run_id,
                    db,
                    plan.id,
                    base_config,
                    plan.attempt_count,
                    &reason,
                )
                .await;
                return;
            }
        };
        if fill_scope.entry_filled <= f64::EPSILON {
            self.mark_planned_exit_retry(
                run_id,
                db,
                plan.id,
                base_config,
                plan.attempt_count,
                &format!(
                    "计划退出到期但尚未同步到入口订单成交，继续等待成交同步: entry_order_id={}, entry_client_order_id={}",
                    plan.entry_order_id, plan.entry_client_order_id
                ),
            )
            .await;
            return;
        }
        if fill_scope.residual <= f64::EPSILON {
            let reason = format!(
                "计划退出入口成交已由退出成交覆盖，无剩余可平数量: entry_filled={}, exit_filled={}",
                order_size_string(fill_scope.entry_filled),
                order_size_string(fill_scope.exit_filled)
            );
            self.mark_planned_exit_skipped(db, run_id, plan.id, "skipped_no_position", &reason)
                .await;
            return;
        }
        if let Some(reason) = self
            .cancel_planned_exit_entry_remainder(
                run_id,
                base_config,
                db,
                private_client,
                &plan,
                now_ms,
            )
            .await
        {
            self.mark_planned_exit_retry(
                run_id,
                db,
                plan.id,
                base_config,
                plan.attempt_count,
                &reason,
            )
            .await;
            return;
        }
        let reference_price = self
            .planned_exit_reference_price(client, &exit_config, &plan)
            .await;
        if !reference_price.is_finite() || reference_price <= 0.0 {
            let reason = "计划退出缺少有效当前价格和 entry_price，暂缓重试".to_string();
            self.mark_planned_exit_retry(
                run_id,
                db,
                plan.id,
                base_config,
                plan.attempt_count,
                &reason,
            )
            .await;
            return;
        }
        self.log_execution_stage(
            run_id,
            "planned_exit",
            "info",
            format!(
                "计划退出到期，准备平仓: {} {}",
                exit_config.symbol, plan.close_side
            ),
            serde_json::json!({
                "plan_id": plan.id,
                "symbol": exit_config.symbol,
                "close_side": plan.close_side,
                "planned_exit_time": plan.planned_exit_time,
                "planned_exit_reason": plan.planned_exit_reason,
                "planned_exit_contract": plan.planned_exit_contract,
                "entry_filled": fill_scope.entry_filled,
                "exit_filled": fill_scope.exit_filled,
                "residual": fill_scope.residual,
                "attempt_count": plan.attempt_count,
                "now_ms": now_ms,
            }),
        )
        .await;
        let action_record = StrategyActionRecord {
            action: "close_position".to_string(),
            side: "flat".to_string(),
            price: reference_price,
            reason: if plan.planned_exit_reason.trim().is_empty() {
                "planned_exit".to_string()
            } else {
                plan.planned_exit_reason.clone()
            },
            strength: 1.0,
            timestamp: plan.planned_exit_time,
            position_size: None,
        };
        let exit_client_order_id = live_strategy_client_order_id();
        match self
            .submit_exchange_close_intent(
                run_id,
                &exit_config,
                db,
                client,
                private_client,
                &action_record,
                "market",
                Some(&plan.close_side),
                None,
                Some(&exit_client_order_id),
                Some(plan.id),
                Some(fill_scope.residual),
            )
            .await
        {
            ExchangeSubmitOutcome::Submitted(order)
            | ExchangeSubmitOutcome::SubmittedUnknown(order) => {
                match mark_live_planned_exit_submitted(
                    db,
                    plan.id,
                    run_id,
                    &order.order_id,
                    &order.client_order_id,
                )
                .await
                {
                    Ok(()) => {
                        self.log_execution_stage(
                            run_id,
                            "planned_exit",
                            "success",
                            format!("计划退出平仓单已提交: {}", exit_config.symbol),
                            serde_json::json!({
                                "plan_id": plan.id,
                                "symbol": exit_config.symbol,
                                "order_id": order.order_id,
                                "client_order_id": order.client_order_id,
                                "entry_filled": fill_scope.entry_filled,
                                "exit_filled": fill_scope.exit_filled,
                                "residual": fill_scope.residual,
                            }),
                        )
                        .await;
                    }
                    Err(error) => {
                        self.log_execution_stage(
                            run_id,
                            "planned_exit",
                            "error",
                            format!("更新计划退出提交状态失败: {error}"),
                            serde_json::json!({
                                "plan_id": plan.id,
                                "symbol": exit_config.symbol,
                            }),
                        )
                        .await;
                    }
                }
            }
            ExchangeSubmitOutcome::NotSubmitted { reason, .. }
                if is_no_close_position_reason(&reason) =>
            {
                self.mark_planned_exit_retry(
                    run_id,
                    db,
                    plan.id,
                    base_config,
                    plan.attempt_count,
                    &format!(
                        "计划退出到期但暂未发现可平仓位，继续等待交易所持仓/成交同步: {reason}"
                    ),
                )
                .await;
            }
            ExchangeSubmitOutcome::NotSubmitted { reason, .. } => {
                self.mark_planned_exit_retry(
                    run_id,
                    db,
                    plan.id,
                    base_config,
                    plan.attempt_count,
                    &reason,
                )
                .await;
            }
        }
    }

    async fn cancel_planned_exit_entry_remainder(
        &self,
        run_id: &str,
        base_config: &LiveStrategyConfig,
        db: &SqlitePool,
        private_client: &OkxPrivateClient,
        plan: &LivePlannedExitPlan,
        now_ms: i64,
    ) -> Option<String> {
        if plan.entry_order_id.trim().is_empty() && plan.entry_client_order_id.trim().is_empty() {
            return None;
        }
        let entry_order = match query_live_order_identity_context_for_symbol(
            db,
            &plan.mode,
            &plan.symbol,
            &plan.entry_order_id,
            &plan.entry_client_order_id,
        )
        .await
        {
            Ok(Some(order)) => order,
            Ok(None) => {
                self.log_execution_stage(
                    run_id,
                    "planned_exit",
                    "info",
                    "计划退出未找到可撤入口订单，继续按已成交残量平仓",
                    serde_json::json!({
                        "plan_id": plan.id,
                        "symbol": plan.symbol,
                        "entry_order_id": plan.entry_order_id,
                        "entry_client_order_id": plan.entry_client_order_id,
                    }),
                )
                .await;
                return None;
            }
            Err(error) => {
                return Some(format!("查询计划退出入口订单状态失败，暂缓平仓: {error}"));
            }
        };
        if !planned_exit_entry_remainder_cancel_needed(&entry_order.status, &entry_order.order_type)
        {
            return None;
        }
        let cancel_config = config_for_live_order(base_config, &entry_order);
        let cancel_action = StrategyActionRecord {
            action: "cancel_order".to_string(),
            side: "hold".to_string(),
            price: plan.entry_price,
            reason: "planned_exit_cancel_entry_remainder".to_string(),
            strength: 1.0,
            timestamp: now_ms,
            position_size: None,
        };
        let cancel_order = StrategyCancelOrderIntent {
            order_id: entry_order.order_id.clone(),
            client_order_id: entry_order.client_order_id.clone(),
            scope_explicit: false,
            target_kind: StrategyOrderTargetKind::Exchange,
        };
        self.log_execution_stage(
            run_id,
            "planned_exit",
            "info",
            "计划退出到期，先撤入口订单未成交剩余",
            serde_json::json!({
                "plan_id": plan.id,
                "symbol": cancel_config.symbol,
                "entry_order_id": cancel_order.order_id,
                "entry_client_order_id": cancel_order.client_order_id,
                "entry_order_type": entry_order.order_type,
                "entry_order_status": entry_order.status,
            }),
        )
        .await;
        match self
            .submit_exchange_cancel_intent(
                run_id,
                &cancel_config,
                db,
                private_client,
                &cancel_action,
                Some(&cancel_order),
            )
            .await
        {
            ExchangeSubmitOutcome::Submitted(_) | ExchangeSubmitOutcome::SubmittedUnknown(_) => {
                None
            }
            ExchangeSubmitOutcome::NotSubmitted { reason, retryable } if retryable => {
                Some(format!("计划退出入口剩余撤单尚未确认，暂缓平仓: {reason}"))
            }
            ExchangeSubmitOutcome::NotSubmitted { reason, .. } => {
                self.log_execution_stage(
                    run_id,
                    "planned_exit",
                    "warn",
                    format!("计划退出入口剩余撤单未提交，继续按已成交残量平仓: {reason}"),
                    serde_json::json!({
                        "plan_id": plan.id,
                        "symbol": cancel_config.symbol,
                        "entry_order_id": cancel_order.order_id,
                        "entry_client_order_id": cancel_order.client_order_id,
                    }),
                )
                .await;
                None
            }
        }
    }

    pub(super) async fn planned_exit_reference_price(
        &self,
        client: &OkxPublicClient,
        config: &LiveStrategyConfig,
        plan: &LivePlannedExitPlan,
    ) -> f64 {
        let arrival = fetch_arrival_quote(client, config).await;
        shared_planned_exit_reference_price(
            arrival.mid_px,
            arrival.bid_px,
            arrival.ask_px,
            &plan.close_side,
            plan.entry_price,
        )
        .unwrap_or(0.0)
    }

    pub(super) async fn mark_planned_exit_retry(
        &self,
        run_id: &str,
        db: &SqlitePool,
        plan_id: i64,
        config: &LiveStrategyConfig,
        attempt_count: i64,
        reason: &str,
    ) {
        let delay_ms = planned_exit_retry_delay_ms(config, attempt_count);
        let next_attempt_at = chrono::Utc::now()
            .timestamp_millis()
            .saturating_add(delay_ms);
        if let Err(error) = mark_live_planned_exit_retry(db, plan_id, next_attempt_at, reason).await
        {
            self.log_execution_stage(
                run_id,
                "planned_exit",
                "error",
                format!("更新计划退出重试时间失败: {error}"),
                serde_json::json!({ "plan_id": plan_id, "reason": reason }),
            )
            .await;
        } else {
            self.inner.planned_exit_notify.notify_one();
        }
    }

    pub(super) async fn mark_planned_exit_skipped(
        &self,
        db: &SqlitePool,
        run_id: &str,
        plan_id: i64,
        status: &str,
        reason: &str,
    ) {
        match mark_live_planned_exit_skipped(db, plan_id, run_id, status, reason).await {
            Ok(()) => {
                self.log_execution_stage(
                    run_id,
                    "planned_exit",
                    "warn",
                    format!("计划退出已跳过: {reason}"),
                    serde_json::json!({
                        "plan_id": plan_id,
                        "status": status,
                    }),
                )
                .await;
            }
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "planned_exit",
                    "error",
                    format!("更新计划退出跳过状态失败: {error}"),
                    serde_json::json!({
                        "plan_id": plan_id,
                        "status": status,
                        "reason": reason,
                    }),
                )
                .await;
            }
        }
    }
}
