use super::*;

pub(super) struct OrderNotFoundRejectionRequest<'a> {
    pub(super) run_id: &'a str,
    pub(super) config: &'a LiveStrategyConfig,
    pub(super) db: &'a SqlitePool,
    pub(super) candidate: &'a LiveOrderSyncCandidate,
    pub(super) order_id: &'a str,
    pub(super) client_order_id: &'a str,
    pub(super) error_message: &'a str,
}

struct PlannedExitOrderStateUpdateRequest<'a> {
    run_id: &'a str,
    config: &'a LiveStrategyConfig,
    db: &'a SqlitePool,
    candidate: &'a LivePlannedExitOrderSyncCandidate,
    response: &'a serde_json::Value,
    order_id: &'a str,
    client_order_id: &'a str,
}

struct PlannedExitOrderNotFoundRetryRequest<'a> {
    run_id: &'a str,
    config: &'a LiveStrategyConfig,
    db: &'a SqlitePool,
    candidate: &'a LivePlannedExitOrderSyncCandidate,
    order_id: &'a str,
    client_order_id: &'a str,
    error_message: &'a str,
}

impl LiveStrategyRuntime {
    pub(in crate::live_strategy) async fn sync_exchange_order_states(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        private_client: &OkxPrivateClient,
    ) {
        let candidates =
            match query_live_order_sync_candidates(db, &config.mode, &config.strategy_id, 50).await
            {
                Ok(candidates) => candidates,
                Err(error) => {
                    self.log_execution_stage(
                        run_id,
                        "order_sync",
                        "warn",
                        format!("读取待同步订单失败: {error}"),
                        serde_json::json!({
                            "mode": config.mode,
                            "strategy_id": config.strategy_id,
                        }),
                    )
                    .await;
                    return;
                }
            };
        if candidates.is_empty() {
            self.sync_algo_order_states(run_id, config, db, private_client)
                .await;
            self.sync_planned_exit_order_states(run_id, config, db, private_client)
                .await;
            self.sync_exchange_fills(run_id, config, db, private_client)
                .await;
            return;
        }

        let mut changed_count = 0_i64;
        let mut checked_count = 0_i64;
        for candidate in candidates {
            checked_count += 1;
            if self
                .sync_single_exchange_order_state(run_id, config, db, private_client, &candidate)
                .await
            {
                changed_count += 1;
            }
        }
        if changed_count > 0 {
            self.log_execution_stage(
                run_id,
                "order_sync",
                "success",
                format!("订单状态同步完成，更新 {changed_count}/{checked_count} 条"),
                serde_json::json!({
                    "checked_count": checked_count,
                    "changed_count": changed_count,
                }),
            )
            .await;
        }
        self.sync_algo_order_states(run_id, config, db, private_client)
            .await;
        self.sync_planned_exit_order_states(run_id, config, db, private_client)
            .await;
        self.sync_exchange_fills(run_id, config, db, private_client)
            .await;
    }

    async fn sync_single_exchange_order_state(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        private_client: &OkxPrivateClient,
        candidate: &LiveOrderSyncCandidate,
    ) -> bool {
        let Some((order_id, client_order_id)) = sync_order_identity(candidate) else {
            self.log_execution_stage(
                run_id,
                "order_sync",
                "warn",
                format!("跳过无法查单的本地订单: id={}", candidate.id),
                serde_json::json!({
                    "local_order_id": candidate.id,
                    "symbol": candidate.symbol,
                    "order_id": candidate.order_id,
                    "client_order_id": candidate.client_order_id,
                }),
            )
            .await;
            return false;
        };

        let response = match private_client
            .get_order(&candidate.symbol, &order_id, &client_order_id)
            .await
        {
            Ok(response) => response,
            Err(error) => {
                let error_message = error.to_string();
                if okx_order_not_found_error(&error_message) {
                    if let Some(history_order) = self
                        .find_exchange_order_history_by_identity(
                            run_id,
                            private_client,
                            candidate,
                            &order_id,
                            &client_order_id,
                        )
                        .await
                    {
                        return self
                            .apply_exchange_order_state_update(
                                run_id,
                                config,
                                db,
                                candidate,
                                &history_order,
                                "OKX 订单活动查单为空，已从历史订单同步状态",
                            )
                            .await;
                    }
                }
                if is_exchange_order_not_found_after_grace(
                    candidate,
                    &error_message,
                    chrono::Utc::now().timestamp_millis(),
                ) {
                    return self
                        .mark_not_found_order_rejected(OrderNotFoundRejectionRequest {
                            run_id,
                            config,
                            db,
                            candidate,
                            order_id: &order_id,
                            client_order_id: &client_order_id,
                            error_message: &error_message,
                        })
                        .await;
                }
                self.log_execution_stage(
                    run_id,
                    "order_sync",
                    "warn",
                    format!("查询 OKX 订单状态失败: {error_message}"),
                    serde_json::json!({
                        "local_order_id": candidate.id,
                        "symbol": candidate.symbol,
                        "order_id": candidate.order_id,
                        "client_order_id": candidate.client_order_id,
                    }),
                )
                .await;
                return false;
            }
        };
        self.apply_exchange_order_state_update(
            run_id,
            config,
            db,
            candidate,
            &response,
            "订单状态已更新",
        )
        .await
    }

    async fn find_exchange_order_history_by_identity(
        &self,
        run_id: &str,
        private_client: &OkxPrivateClient,
        candidate: &LiveOrderSyncCandidate,
        order_id: &str,
        client_order_id: &str,
    ) -> Option<serde_json::Value> {
        let inst_type = candidate.inst_type.trim();
        let history = match private_client
            .get_order_history(
                (!inst_type.is_empty()).then_some(inst_type),
                Some(&candidate.symbol),
                100,
            )
            .await
        {
            Ok(history) => history,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "order_sync",
                    "warn",
                    format!("OKX 活动查单为空后查询历史订单失败: {error}"),
                    serde_json::json!({
                        "local_order_id": candidate.id,
                        "symbol": candidate.symbol,
                        "inst_type": candidate.inst_type,
                        "order_id": candidate.order_id,
                        "client_order_id": candidate.client_order_id,
                    }),
                )
                .await;
                return None;
            }
        };
        let matched = find_exchange_order_by_identity(history, order_id, client_order_id);
        if matched.is_none() {
            self.log_execution_stage(
                run_id,
                "order_sync",
                "warn",
                "OKX 活动查单为空，历史订单也未找到本地订单身份",
                serde_json::json!({
                    "local_order_id": candidate.id,
                    "symbol": candidate.symbol,
                    "inst_type": candidate.inst_type,
                    "order_id": candidate.order_id,
                    "client_order_id": candidate.client_order_id,
                }),
            )
            .await;
        }
        matched
    }

    async fn apply_exchange_order_state_update(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        candidate: &LiveOrderSyncCandidate,
        response: &serde_json::Value,
        changed_log_prefix: &str,
    ) -> bool {
        let Some(update) = exchange_order_state_from_okx_order(response, candidate) else {
            self.log_execution_stage(
                run_id,
                "order_sync",
                "warn",
                "OKX 查单响应缺少有效订单状态",
                serde_json::json!({
                    "local_order_id": candidate.id,
                    "symbol": candidate.symbol,
                    "response": response,
                }),
            )
            .await;
            return false;
        };

        let changed = match update_live_order_exchange_state(db, candidate.id, &update).await {
            Ok(changed) => changed,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "order_sync",
                    "warn",
                    format!("更新本地订单状态失败: {error}"),
                    serde_json::json!({
                        "local_order_id": candidate.id,
                        "symbol": candidate.symbol,
                        "exchange_status": update.status,
                    }),
                )
                .await;
                return false;
            }
        };
        if changed {
            self.log_execution_stage(
                run_id,
                "order_sync",
                "info",
                format!(
                    "{changed_log_prefix}: {} {} -> {}",
                    candidate.symbol, candidate.status, update.status
                ),
                serde_json::json!({
                    "local_order_id": candidate.id,
                    "symbol": candidate.symbol,
                    "previous_status": candidate.status,
                    "status": update.status,
                    "order_id": update.order_id,
                    "client_order_id": update.client_order_id,
                }),
            )
            .await;
        }
        let mut planned_exit_changed = 0_u64;
        match mark_live_planned_exit_order_terminal_for_strategy(
            db,
            &config.mode,
            &config.strategy_id,
            &candidate.symbol,
            &update.order_id,
            &update.client_order_id,
            &update.status,
            &update.error_message,
            next_exit_order_retry_at(),
        )
        .await
        {
            Ok(changed) => {
                planned_exit_changed += changed;
            }
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "order_sync",
                    "warn",
                    format!("更新计划退出终态失败: {error}"),
                    serde_json::json!({
                        "local_order_id": candidate.id,
                        "symbol": candidate.symbol,
                        "status": update.status,
                    }),
                )
                .await;
            }
        }
        if is_unfilled_terminal_order_state(&update) {
            match mark_live_planned_exit_entry_order_failed_for_strategy(
                db,
                &config.mode,
                &config.strategy_id,
                &candidate.symbol,
                &update.order_id,
                &update.client_order_id,
                &update.status,
                &update.error_message,
            )
            .await
            {
                Ok(changed) => {
                    planned_exit_changed += changed;
                }
                Err(error) => {
                    self.log_execution_stage(
                        run_id,
                        "order_sync",
                        "warn",
                        format!("入口订单失败后取消计划退出失败: {error}"),
                        serde_json::json!({
                            "local_order_id": candidate.id,
                            "symbol": candidate.symbol,
                            "status": update.status,
                        }),
                    )
                    .await;
                }
            }
        }
        if planned_exit_changed > 0 {
            self.inner.planned_exit_notify.notify_one();
        }
        changed || planned_exit_changed > 0
    }

    pub(super) async fn mark_not_found_order_rejected(
        &self,
        request: OrderNotFoundRejectionRequest<'_>,
    ) -> bool {
        let OrderNotFoundRejectionRequest {
            run_id,
            config,
            db,
            candidate,
            order_id,
            client_order_id,
            error_message,
        } = request;
        let message =
            format!("OKX 查无本地非终态订单，标记为 rejected 以允许后续同步/重试: {error_message}");
        let update = LiveOrderExchangeState {
            status: "rejected".to_string(),
            success: false,
            error_message: message.clone(),
            order_id: order_id.to_string(),
            client_order_id: client_order_id.to_string(),
        };
        let order_changed = match update_live_order_exchange_state(db, candidate.id, &update).await
        {
            Ok(changed) => changed,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "order_sync",
                    "warn",
                    format!("标记待确认订单为 rejected 失败: {error}"),
                    serde_json::json!({
                        "local_order_id": candidate.id,
                        "symbol": candidate.symbol,
                        "client_order_id": candidate.client_order_id,
                    }),
                )
                .await;
                false
            }
        };
        let mut planned_exit_changed = match mark_live_planned_exit_order_terminal_for_strategy(
            db,
            &config.mode,
            &config.strategy_id,
            &candidate.symbol,
            order_id,
            client_order_id,
            "rejected",
            &message,
            next_exit_order_retry_at(),
        )
        .await
        {
            Ok(changed) => changed,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "order_sync",
                    "warn",
                    format!("订单查无后更新计划退出重试状态失败: {error}"),
                    serde_json::json!({
                        "local_order_id": candidate.id,
                        "symbol": candidate.symbol,
                        "client_order_id": candidate.client_order_id,
                    }),
                )
                .await;
                0
            }
        };
        match mark_live_planned_exit_entry_order_failed_for_strategy(
            db,
            &config.mode,
            &config.strategy_id,
            &candidate.symbol,
            order_id,
            client_order_id,
            "rejected",
            &message,
        )
        .await
        {
            Ok(changed) => {
                planned_exit_changed += changed;
            }
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "order_sync",
                    "warn",
                    format!("入口订单查无后取消计划退出失败: {error}"),
                    serde_json::json!({
                        "local_order_id": candidate.id,
                        "symbol": candidate.symbol,
                        "client_order_id": client_order_id,
                    }),
                )
                .await;
            }
        }
        if planned_exit_changed > 0 {
            self.inner.planned_exit_notify.notify_one();
        }
        self.log_execution_stage(
            run_id,
            "order_sync",
            "warn",
            message,
            serde_json::json!({
                "local_order_id": candidate.id,
                "symbol": candidate.symbol,
                "order_id": order_id,
                "client_order_id": client_order_id,
                "order_changed": order_changed,
                "planned_exit_changed": planned_exit_changed,
            }),
        )
        .await;
        order_changed || planned_exit_changed > 0
    }

    async fn sync_algo_order_states(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        private_client: &OkxPrivateClient,
    ) {
        let candidates =
            match query_live_algo_order_sync_candidates(db, &config.mode, &config.strategy_id, 20)
                .await
            {
                Ok(candidates) => candidates,
                Err(error) => {
                    self.log_execution_stage(
                        run_id,
                        "algo_order_sync",
                        "warn",
                        format!("读取待同步保护单失败: {error}"),
                        serde_json::json!({
                            "mode": config.mode,
                            "strategy_id": config.strategy_id,
                        }),
                    )
                    .await;
                    return;
                }
            };
        if candidates.is_empty() {
            return;
        }

        let mut changed_count = 0_i64;
        let mut checked_count = 0_i64;
        for candidate in candidates {
            checked_count += 1;
            if self
                .sync_single_algo_order_state(run_id, db, private_client, &candidate)
                .await
            {
                changed_count += 1;
            }
        }
        if changed_count > 0 {
            self.log_execution_stage(
                run_id,
                "algo_order_sync",
                "success",
                format!("保护单状态同步完成，更新 {changed_count}/{checked_count} 条"),
                serde_json::json!({
                    "checked_count": checked_count,
                    "changed_count": changed_count,
                }),
            )
            .await;
        }
    }

    async fn sync_single_algo_order_state(
        &self,
        run_id: &str,
        db: &SqlitePool,
        private_client: &OkxPrivateClient,
        candidate: &LiveOrderSyncCandidate,
    ) -> bool {
        if candidate.inst_type.trim().is_empty() {
            self.log_execution_stage(
                run_id,
                "algo_order_sync",
                "warn",
                format!("跳过缺少 inst_type 的本地保护单: id={}", candidate.id),
                serde_json::json!({
                    "local_order_id": candidate.id,
                    "symbol": candidate.symbol,
                    "algo_id": candidate.order_id,
                    "algo_client_order_id": candidate.client_order_id,
                    "status": candidate.status,
                }),
            )
            .await;
            return false;
        }
        let Some(response) = self
            .find_okx_conditional_algo_order(private_client, candidate)
            .await
        else {
            self.log_execution_stage(
                run_id,
                "algo_order_sync",
                "warn",
                format!("OKX 未返回本地保护单状态: id={}", candidate.id),
                serde_json::json!({
                    "local_order_id": candidate.id,
                    "symbol": candidate.symbol,
                    "inst_type": candidate.inst_type,
                    "algo_id": candidate.order_id,
                    "algo_client_order_id": candidate.client_order_id,
                    "status": candidate.status,
                }),
            )
            .await;
            return false;
        };
        let Some(update) = algo_order_state_from_okx_order(&response, candidate) else {
            self.log_execution_stage(
                run_id,
                "algo_order_sync",
                "warn",
                "OKX 保护单响应缺少有效状态",
                serde_json::json!({
                    "local_order_id": candidate.id,
                    "symbol": candidate.symbol,
                    "response": response,
                }),
            )
            .await;
            return false;
        };
        let changed = match update_live_order_exchange_state(db, candidate.id, &update).await {
            Ok(changed) => changed,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "algo_order_sync",
                    "warn",
                    format!("更新本地保护单状态失败: {error}"),
                    serde_json::json!({
                        "local_order_id": candidate.id,
                        "symbol": candidate.symbol,
                        "exchange_status": update.status,
                    }),
                )
                .await;
                return false;
            }
        };
        if changed {
            self.log_execution_stage(
                run_id,
                "algo_order_sync",
                "info",
                format!(
                    "保护单状态已更新: {} {} -> {}",
                    candidate.symbol, candidate.status, update.status
                ),
                serde_json::json!({
                    "local_order_id": candidate.id,
                    "symbol": candidate.symbol,
                    "previous_status": candidate.status,
                    "status": update.status,
                    "algo_id": update.order_id,
                    "algo_client_order_id": update.client_order_id,
                }),
            )
            .await;
        }
        changed
    }

    async fn find_okx_conditional_algo_order(
        &self,
        private_client: &OkxPrivateClient,
        candidate: &LiveOrderSyncCandidate,
    ) -> Option<Value> {
        let algo_id = candidate.order_id.trim();
        let algo_client_order_id = candidate.client_order_id.trim();
        let inst_type = candidate.inst_type.trim();
        if inst_type.is_empty() {
            return None;
        }
        if !algo_id.is_empty() {
            if let Some(item) = private_client
                .get_conditional_algo_orders_pending(
                    Some(inst_type),
                    Some(&candidate.symbol),
                    algo_id,
                    20,
                )
                .await
                .ok()
                .and_then(|items| find_algo_order_by_identity(items, algo_id, algo_client_order_id))
            {
                return Some(item);
            }
            return private_client
                .get_conditional_algo_orders_history(
                    Some(inst_type),
                    Some(&candidate.symbol),
                    None,
                    algo_id,
                    20,
                )
                .await
                .ok()
                .and_then(|items| {
                    find_algo_order_by_identity(items, algo_id, algo_client_order_id)
                });
        }

        if algo_client_order_id.is_empty() {
            return None;
        }
        if let Some(item) = private_client
            .get_conditional_algo_orders_pending(Some(inst_type), Some(&candidate.symbol), "", 100)
            .await
            .ok()
            .and_then(|items| find_algo_order_by_identity(items, "", algo_client_order_id))
        {
            return Some(item);
        }
        for state in ["effective", "canceled", "order_failed"] {
            if let Some(item) = private_client
                .get_conditional_algo_orders_history(
                    Some(inst_type),
                    Some(&candidate.symbol),
                    Some(state),
                    "",
                    100,
                )
                .await
                .ok()
                .and_then(|items| find_algo_order_by_identity(items, "", algo_client_order_id))
            {
                return Some(item);
            }
        }
        None
    }

    async fn sync_planned_exit_order_states(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        private_client: &OkxPrivateClient,
    ) {
        let candidates = match query_submitted_live_planned_exit_order_sync_candidates(
            db,
            &config.mode,
            &config.strategy_id,
            20,
        )
        .await
        {
            Ok(candidates) => candidates,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "planned_exit_sync",
                    "warn",
                    format!("读取计划退出查单候选失败: {error}"),
                    serde_json::json!({
                        "mode": config.mode,
                        "strategy_id": config.strategy_id,
                    }),
                )
                .await;
                return;
            }
        };
        if candidates.is_empty() {
            return;
        }

        let mut checked_count = 0_i64;
        let mut changed_count = 0_i64;
        for candidate in candidates {
            checked_count += 1;
            if self
                .sync_single_planned_exit_order_state(
                    run_id,
                    config,
                    db,
                    private_client,
                    &candidate,
                )
                .await
            {
                changed_count += 1;
            }
        }
        if changed_count > 0 {
            self.inner.planned_exit_notify.notify_one();
            self.log_execution_stage(
                run_id,
                "planned_exit_sync",
                "success",
                format!("计划退出订单状态同步完成，更新 {changed_count}/{checked_count} 个计划"),
                serde_json::json!({
                    "checked_count": checked_count,
                    "changed_count": changed_count,
                }),
            )
            .await;
        }
    }

    async fn sync_single_planned_exit_order_state(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        private_client: &OkxPrivateClient,
        candidate: &LivePlannedExitOrderSyncCandidate,
    ) -> bool {
        let Some((order_id, client_order_id)) = planned_exit_order_sync_identity(candidate) else {
            self.log_execution_stage(
                run_id,
                "planned_exit_sync",
                "warn",
                format!("跳过无法查单的计划退出: plan_id={}", candidate.id),
                serde_json::json!({
                    "plan_id": candidate.id,
                    "symbol": candidate.symbol,
                    "order_id": candidate.order_id,
                    "client_order_id": candidate.client_order_id,
                }),
            )
            .await;
            return false;
        };

        let response = match private_client
            .get_order(&candidate.symbol, &order_id, &client_order_id)
            .await
        {
            Ok(response) => response,
            Err(error) => {
                let error_message = error.to_string();
                if okx_order_not_found_error(&error_message) {
                    if let Some(history_order) = self
                        .find_planned_exit_order_history_by_identity(
                            run_id,
                            private_client,
                            candidate,
                            &order_id,
                            &client_order_id,
                        )
                        .await
                    {
                        return self
                            .apply_planned_exit_order_state_update(
                                PlannedExitOrderStateUpdateRequest {
                                    run_id,
                                    config,
                                    db,
                                    candidate,
                                    response: &history_order,
                                    order_id: &order_id,
                                    client_order_id: &client_order_id,
                                },
                            )
                            .await;
                    }
                }
                if is_planned_exit_order_not_found_after_grace(
                    candidate,
                    &error_message,
                    chrono::Utc::now().timestamp_millis(),
                ) {
                    return self
                        .mark_planned_exit_order_not_found_for_retry(
                            PlannedExitOrderNotFoundRetryRequest {
                                run_id,
                                config,
                                db,
                                candidate,
                                order_id: &order_id,
                                client_order_id: &client_order_id,
                                error_message: &error_message,
                            },
                        )
                        .await;
                }
                self.log_execution_stage(
                    run_id,
                    "planned_exit_sync",
                    "warn",
                    format!("查询计划退出 OKX 订单状态失败: {error_message}"),
                    serde_json::json!({
                        "plan_id": candidate.id,
                        "symbol": candidate.symbol,
                        "order_id": candidate.order_id,
                        "client_order_id": candidate.client_order_id,
                    }),
                )
                .await;
                return false;
            }
        };
        self.apply_planned_exit_order_state_update(PlannedExitOrderStateUpdateRequest {
            run_id,
            config,
            db,
            candidate,
            response: &response,
            order_id: &order_id,
            client_order_id: &client_order_id,
        })
        .await
    }

    async fn find_planned_exit_order_history_by_identity(
        &self,
        run_id: &str,
        private_client: &OkxPrivateClient,
        candidate: &LivePlannedExitOrderSyncCandidate,
        order_id: &str,
        client_order_id: &str,
    ) -> Option<serde_json::Value> {
        let inst_type = candidate.inst_type.trim();
        let history = match private_client
            .get_order_history(
                (!inst_type.is_empty()).then_some(inst_type),
                Some(&candidate.symbol),
                100,
            )
            .await
        {
            Ok(history) => history,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "planned_exit_sync",
                    "warn",
                    format!("计划退出活动查单为空后查询历史订单失败: {error}"),
                    serde_json::json!({
                        "plan_id": candidate.id,
                        "symbol": candidate.symbol,
                        "inst_type": candidate.inst_type,
                        "order_id": candidate.order_id,
                        "client_order_id": candidate.client_order_id,
                    }),
                )
                .await;
                return None;
            }
        };
        let matched = find_exchange_order_by_identity(history, order_id, client_order_id);
        if matched.is_none() {
            self.log_execution_stage(
                run_id,
                "planned_exit_sync",
                "warn",
                "计划退出活动查单为空，历史订单也未找到平仓单身份",
                serde_json::json!({
                    "plan_id": candidate.id,
                    "symbol": candidate.symbol,
                    "inst_type": candidate.inst_type,
                    "order_id": candidate.order_id,
                    "client_order_id": candidate.client_order_id,
                }),
            )
            .await;
        }
        matched
    }

    async fn apply_planned_exit_order_state_update(
        &self,
        request: PlannedExitOrderStateUpdateRequest<'_>,
    ) -> bool {
        let PlannedExitOrderStateUpdateRequest {
            run_id,
            config,
            db,
            candidate,
            response,
            order_id,
            client_order_id,
        } = request;
        let Some(update) =
            exchange_order_state_from_okx_order_identity(response, order_id, client_order_id)
        else {
            self.log_execution_stage(
                run_id,
                "planned_exit_sync",
                "warn",
                "OKX 计划退出查单响应缺少有效订单状态",
                serde_json::json!({
                    "plan_id": candidate.id,
                    "symbol": candidate.symbol,
                    "response": response,
                }),
            )
            .await;
            return false;
        };

        match mark_live_planned_exit_order_terminal_for_strategy(
            db,
            &config.mode,
            &config.strategy_id,
            &candidate.symbol,
            &update.order_id,
            &update.client_order_id,
            &update.status,
            &update.error_message,
            next_exit_order_retry_at(),
        )
        .await
        {
            Ok(changed) => changed > 0,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "planned_exit_sync",
                    "warn",
                    format!("更新计划退出订单终态失败: {error}"),
                    serde_json::json!({
                        "plan_id": candidate.id,
                        "symbol": candidate.symbol,
                        "status": update.status,
                    }),
                )
                .await;
                false
            }
        }
    }

    async fn mark_planned_exit_order_not_found_for_retry(
        &self,
        request: PlannedExitOrderNotFoundRetryRequest<'_>,
    ) -> bool {
        let PlannedExitOrderNotFoundRetryRequest {
            run_id,
            config,
            db,
            candidate,
            order_id,
            client_order_id,
            error_message,
        } = request;
        let message = format!("OKX 查无计划退出平仓单，已重新排队等待再次平仓: {error_message}");
        let changed = match mark_live_planned_exit_order_terminal_for_strategy(
            db,
            &config.mode,
            &config.strategy_id,
            &candidate.symbol,
            order_id,
            client_order_id,
            "rejected",
            &message,
            next_exit_order_retry_at(),
        )
        .await
        {
            Ok(changed) => changed,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "planned_exit_sync",
                    "warn",
                    format!("计划退出平仓单查无后重新排队失败: {error}"),
                    serde_json::json!({
                        "plan_id": candidate.id,
                        "symbol": candidate.symbol,
                        "order_id": order_id,
                        "client_order_id": client_order_id,
                    }),
                )
                .await;
                0
            }
        };
        if changed > 0 {
            self.inner.planned_exit_notify.notify_one();
        }
        self.log_execution_stage(
            run_id,
            "planned_exit_sync",
            if changed > 0 { "warn" } else { "error" },
            if changed > 0 {
                message
            } else {
                format!("OKX 查无计划退出平仓单，但未找到可重新排队的计划: {error_message}")
            },
            serde_json::json!({
                "plan_id": candidate.id,
                "symbol": candidate.symbol,
                "order_id": order_id,
                "client_order_id": client_order_id,
                "planned_exit_changed": changed,
            }),
        )
        .await;
        changed > 0
    }

    pub(super) async fn sync_exchange_fills(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        private_client: &OkxPrivateClient,
    ) {
        let scopes = match query_live_fill_sync_scopes(
            db,
            &config.mode,
            &config.strategy_id,
            live_fill_sync_symbol_limit(config),
        )
        .await
        {
            Ok(scopes) => scopes,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "fill_sync",
                    "warn",
                    format!("读取成交同步币种失败: {error}"),
                    serde_json::json!({
                        "mode": config.mode,
                        "strategy_id": config.strategy_id,
                    }),
                )
                .await;
                return;
            }
        };
        if scopes.is_empty() {
            return;
        }

        let mut fetched = 0_i64;
        let mut stored = 0_i64;
        let mut arrival_matched = 0_i64;
        let mut order_changed = 0_u64;
        let mut planned_exit_changed = 0_u64;
        for scope in scopes {
            let items = match private_client
                .get_fills(
                    Some(scope.inst_type.trim()),
                    Some(&scope.symbol),
                    LIVE_FILL_SYNC_LIMIT_PER_SYMBOL,
                )
                .await
            {
                Ok(items) => items,
                Err(error) => {
                    self.log_execution_stage(
                        run_id,
                        "fill_sync",
                        "warn",
                        format!("同步 OKX 成交失败: {error}"),
                        serde_json::json!({
                            "symbol": scope.symbol,
                            "inst_type": scope.inst_type,
                        }),
                    )
                    .await;
                    continue;
                }
            };
            fetched += items.len() as i64;
            for item in &items {
                let trade_id = okx_text(item, &["tradeId", "trade_id"]);
                let inst_id = okx_text(item, &["instId", "inst_id"]);
                if trade_id.is_empty() || inst_id.is_empty() {
                    continue;
                }
                let order_id = okx_text(item, &["ordId", "ord_id"]);
                let client_order_id = okx_text(item, &["clOrdId", "cl_ord_id", "client_order_id"]);
                let arrival = match lookup_arrival_evidence_for_symbol(
                    db,
                    &config.mode,
                    &inst_id,
                    &order_id,
                    &client_order_id,
                )
                .await
                {
                    Ok(arrival) => arrival,
                    Err(error) => {
                        self.log_execution_stage(
                            run_id,
                            "fill_sync",
                            "warn",
                            format!("读取成交 arrival 证据失败: {error}"),
                            serde_json::json!({
                                "trade_id": trade_id,
                                "order_id": order_id,
                                "client_order_id": client_order_id,
                            }),
                        )
                        .await;
                        continue;
                    }
                };
                if arrival.has_complete_arrival_quote() {
                    arrival_matched += 1;
                }
                match upsert_local_fill(UpsertLocalFillRequest {
                    db,
                    mode: &config.mode,
                    trade_id: &trade_id,
                    inst_id: &inst_id,
                    item,
                    order_id: &order_id,
                    client_order_id: &client_order_id,
                    arrival: &arrival,
                })
                .await
                {
                    Ok(()) => {
                        stored += 1;
                        match persist_linked_algo_actual_order_event(
                            db,
                            &config.mode,
                            &inst_id,
                            item,
                        )
                        .await
                        {
                            Ok(changed) => {
                                order_changed += changed;
                            }
                            Err(error) => {
                                self.log_execution_stage(
                                    run_id,
                                    "fill_sync",
                                    "warn",
                                    format!("根据 OKX 成交关联保护单实际订单失败: {error}"),
                                    serde_json::json!({
                                        "trade_id": trade_id,
                                        "inst_id": inst_id,
                                        "order_id": order_id,
                                        "client_order_id": client_order_id,
                                    }),
                                )
                                .await;
                            }
                        }
                        match apply_fill_aggregate_to_live_order(
                            db,
                            &config.mode,
                            &inst_id,
                            &order_id,
                            &client_order_id,
                        )
                        .await
                        {
                            Ok(outcome) => {
                                order_changed += outcome.order_changed;
                                planned_exit_changed += outcome.planned_exit_changed;
                            }
                            Err(error) => {
                                self.log_execution_stage(
                                    run_id,
                                    "fill_sync",
                                    "warn",
                                    format!("根据 OKX 成交聚合更新本地订单失败: {error}"),
                                    serde_json::json!({
                                        "trade_id": trade_id,
                                        "inst_id": inst_id,
                                        "order_id": order_id,
                                        "client_order_id": client_order_id,
                                    }),
                                )
                                .await;
                            }
                        }
                    }
                    Err(error) => {
                        self.log_execution_stage(
                            run_id,
                            "fill_sync",
                            "warn",
                            format!("保存 OKX 成交失败: {error}"),
                            serde_json::json!({
                                "trade_id": trade_id,
                                "inst_id": inst_id,
                                "order_id": order_id,
                            }),
                        )
                        .await;
                    }
                }
            }
        }
        if planned_exit_changed > 0 {
            self.inner.planned_exit_notify.notify_one();
        }
        if stored > 0 {
            self.log_execution_stage(
                run_id,
                "fill_sync",
                "success",
                format!("成交同步完成，写入/更新 {stored} 条"),
                serde_json::json!({
                    "fetched": fetched,
                    "stored": stored,
                    "arrival_matched": arrival_matched,
                    "order_changed": order_changed,
                    "planned_exit_changed": planned_exit_changed,
                }),
            )
            .await;
        }
    }
}
