use serde_json::Value;
use sqlx::SqlitePool;

use crate::{
    error::AppResult,
    live_strategy::{
        arrival::ArrivalQuote,
        storage::{
            insert_live_attached_algo_order, insert_live_exchange_order,
            update_live_algo_order_exchange_state_by_identity_and_symbol,
            update_live_exchange_order_state_by_identity_and_symbol, LiveOrderExchangeState,
        },
        types::LiveStrategyConfig,
        LiveStrategyRuntime,
    },
    okx::OkxPrivateClient,
    strategy_engine::StrategyActionRecord,
    trading_semantics::is_contract_inst_type,
};

use super::{apply_blocked_order_status, explicit_configured_leverage, order_size_string};

impl LiveStrategyRuntime {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn pre_register_exchange_order(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        action_record: &StrategyActionRecord,
        order_side: &str,
        order_type: &str,
        quantity: f64,
        arrival: ArrivalQuote,
        client_order_id: &str,
    ) -> Result<i64, String> {
        let message = "本地已预登记 OKX 下单请求，等待交易所响应";
        match insert_live_exchange_order(
            db,
            config,
            order_side,
            order_type,
            quantity,
            action_record.price,
            &action_record.action,
            "submitting",
            false,
            message,
            run_id,
            action_record.timestamp,
            arrival,
            "",
            client_order_id,
        )
        .await
        {
            Ok(row_id) => {
                self.log_execution_stage(
                    run_id,
                    "persist",
                    "info",
                    message,
                    serde_json::json!({
                        "local_order_id": row_id,
                        "symbol": config.symbol,
                        "order_side": order_side,
                        "order_type": order_type,
                        "size": quantity,
                        "client_order_id": client_order_id,
                    }),
                )
                .await;
                Ok(row_id)
            }
            Err(error) => {
                let reason = format!("预登记 OKX 下单请求失败，已拒绝提交交易所订单: {error}");
                self.log_execution_stage(
                    run_id,
                    "persist",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_side": order_side,
                        "order_type": order_type,
                        "size": quantity,
                        "client_order_id": client_order_id,
                    }),
                )
                .await;
                self.set_error(run_id, reason.clone()).await;
                Err(reason)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn pre_register_exchange_algo_order(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        action_record: &StrategyActionRecord,
        order_side: &str,
        order_type: &str,
        quantity: f64,
        arrival: ArrivalQuote,
        client_order_id: &str,
    ) -> Result<i64, String> {
        let message = "本地已预登记 OKX 保护单请求，等待交易所响应";
        match insert_live_exchange_order(
            db,
            config,
            order_side,
            order_type,
            quantity,
            action_record.price,
            "place_risk_order",
            "algo_submitting",
            false,
            message,
            run_id,
            action_record.timestamp,
            arrival,
            "",
            client_order_id,
        )
        .await
        {
            Ok(row_id) => {
                self.log_execution_stage(
                    run_id,
                    "persist",
                    "info",
                    message,
                    serde_json::json!({
                        "local_order_id": row_id,
                        "symbol": config.symbol,
                        "order_side": order_side,
                        "order_type": order_type,
                        "size": quantity,
                        "algo_client_order_id": client_order_id,
                    }),
                )
                .await;
                Ok(row_id)
            }
            Err(error) => {
                let reason = format!("预登记 OKX 保护单请求失败，已拒绝提交交易所保护单: {error}");
                self.log_execution_stage(
                    run_id,
                    "persist",
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_side": order_side,
                        "order_type": order_type,
                        "size": quantity,
                        "algo_client_order_id": client_order_id,
                    }),
                )
                .await;
                self.set_error(run_id, reason.clone()).await;
                Err(reason)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn record_exchange_submit_success(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        action_record: &StrategyActionRecord,
        order_side: &str,
        quantity: f64,
        order_type: &str,
        order_id: &str,
        client_order_id: &str,
        submit_message: &str,
        arrival: ArrivalQuote,
    ) -> Result<(), String> {
        match update_live_exchange_order_state_by_identity_and_symbol(
            db,
            &config.mode,
            &config.symbol,
            order_id,
            client_order_id,
            &LiveOrderExchangeState {
                status: "submitted".to_string(),
                success: true,
                error_message: submit_message.to_string(),
                order_id: order_id.to_string(),
                client_order_id: client_order_id.to_string(),
            },
        )
        .await
        {
            Ok(changed) if changed > 0 => Ok(()),
            Ok(_) => insert_live_exchange_order(
                db,
                config,
                order_side,
                order_type,
                quantity,
                action_record.price,
                &action_record.action,
                "submitted",
                true,
                submit_message,
                run_id,
                action_record.timestamp,
                arrival,
                order_id,
                client_order_id,
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn record_algo_order_submit_success(
        &self,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        action_record: &StrategyActionRecord,
        order_side: &str,
        quantity: f64,
        order_type: &str,
        algo_id: &str,
        match_client_order_id: &str,
        recorded_client_order_id: &str,
        submit_message: &str,
        run_id: &str,
        arrival: ArrivalQuote,
    ) -> Result<(), String> {
        match update_live_algo_order_exchange_state_by_identity_and_symbol(
            db,
            &config.mode,
            &config.symbol,
            algo_id,
            match_client_order_id,
            &LiveOrderExchangeState {
                status: "algo_submitted".to_string(),
                success: true,
                error_message: submit_message.to_string(),
                order_id: algo_id.to_string(),
                client_order_id: recorded_client_order_id.to_string(),
            },
        )
        .await
        {
            Ok(changed) if changed > 0 => Ok(()),
            Ok(_) => insert_live_exchange_order(
                db,
                config,
                order_side,
                order_type,
                quantity,
                action_record.price,
                "place_risk_order",
                "algo_submitted",
                true,
                submit_message,
                run_id,
                action_record.timestamp,
                arrival,
                algo_id,
                recorded_client_order_id,
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn record_attached_algo_orders_after_parent_submit(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        action_record: &StrategyActionRecord,
        quantity: f64,
        arrival: ArrivalQuote,
        attached_risk_orders: &[crate::live_strategy::decision::StrategyRiskOrderIntent],
        attached_algos: &[crate::okx::OkxAttachedAlgoOrder],
        attached_algo_client_order_ids: &[String],
        parent_order_id: &str,
        parent_client_order_id: &str,
    ) {
        if attached_risk_orders.is_empty() {
            return;
        }
        if attached_risk_orders.len() != attached_algo_client_order_ids.len()
            || attached_risk_orders.len() != attached_algos.len()
        {
            self.log_execution_stage(
                run_id,
                "persist",
                "warn",
                "附加保护单本地登记数量不一致，已跳过保护单本地登记",
                serde_json::json!({
                    "symbol": config.symbol,
                    "risk_order_count": attached_risk_orders.len(),
                    "attached_algo_count": attached_algos.len(),
                    "attached_algo_client_order_id_count": attached_algo_client_order_ids.len(),
                }),
            )
            .await;
            return;
        }

        for ((risk, algo), client_order_id) in attached_risk_orders
            .iter()
            .zip(attached_algos.iter())
            .zip(attached_algo_client_order_ids.iter())
        {
            let client_order_id = client_order_id.trim();
            if client_order_id.is_empty() {
                continue;
            }
            let trigger_price = attached_algo_trigger_price(algo).unwrap_or(action_record.price);
            let risk_action_record = StrategyActionRecord {
                action: "place_risk_order".to_string(),
                side: risk.side.clone(),
                price: trigger_price,
                reason: risk.reason.clone(),
                strength: action_record.strength,
                timestamp: action_record.timestamp,
                position_size: None,
            };
            let message = "OKX 父订单已提交，随单保护单等待交易所回报 algoClOrdId";
            match insert_live_attached_algo_order(
                db,
                config,
                &risk.side,
                &risk.order_type,
                quantity,
                risk_action_record.price,
                "algo_submitted",
                true,
                message,
                run_id,
                risk_action_record.timestamp,
                arrival,
                "",
                client_order_id,
                parent_order_id,
                parent_client_order_id,
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
            {
                Ok(()) => {
                    self.log_execution_stage(
                        run_id,
                        "persist",
                        "info",
                        "已登记随单保护单同步身份",
                        serde_json::json!({
                            "symbol": config.symbol,
                            "side": risk.side,
                            "order_type": risk.order_type,
                            "trigger_price": trigger_price,
                            "algo_client_order_id": client_order_id,
                            "parent_order_id": parent_order_id,
                            "parent_client_order_id": parent_client_order_id,
                        }),
                    )
                    .await;
                }
                Err(error) => {
                    self.log_execution_stage(
                        run_id,
                        "persist",
                        "error",
                        format!("保存随单保护单本地记录失败: {error}"),
                        serde_json::json!({
                            "symbol": config.symbol,
                            "side": risk.side,
                            "order_type": risk.order_type,
                            "algo_client_order_id": client_order_id,
                        }),
                    )
                    .await;
                    self.set_error(run_id, format!("保存随单保护单本地记录失败: {error}"))
                        .await;
                }
            }
        }
    }

    pub(super) async fn ensure_exchange_leverage(
        &self,
        config: &LiveStrategyConfig,
        private_client: &OkxPrivateClient,
        td_mode: &str,
        pos_side: &str,
    ) -> AppResult<Option<f64>> {
        let Some(leverage) = explicit_configured_leverage(config)? else {
            return Ok(None);
        };
        if !is_contract_inst_type(&config.inst_type) {
            return Ok(None);
        }
        let td_mode = td_mode.trim().to_ascii_lowercase();
        let pos_side = if td_mode == "isolated" {
            pos_side.trim().to_ascii_lowercase()
        } else {
            String::new()
        };
        let key = format!(
            "{}|{}|{}|{}|{:.8}",
            config.mode.trim().to_ascii_lowercase(),
            config.symbol.trim().to_ascii_uppercase(),
            td_mode,
            pos_side,
            leverage
        );
        {
            let synced = self.inner.synced_leverage_keys.lock().await;
            if synced.contains(&key) {
                return Ok(Some(leverage));
            }
        }
        let leverage_text = order_size_string(leverage);
        private_client
            .set_leverage(&config.symbol, &leverage_text, &td_mode, &pos_side)
            .await
            .map_err(|error| {
                crate::error::AppError::Runtime(format!(
                    "设置 OKX {} {} {} 杠杆为 {}x 失败，已拒绝下单: {error}",
                    config.symbol,
                    if td_mode == "isolated" {
                        "逐仓"
                    } else {
                        "全仓"
                    },
                    if pos_side.is_empty() {
                        ""
                    } else {
                        pos_side.as_str()
                    },
                    leverage_text
                ))
            })?;
        let mut synced = self.inner.synced_leverage_keys.lock().await;
        synced.insert(key);
        Ok(Some(leverage))
    }

    pub(super) async fn record_order_management_request_unknown(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        action_record: &StrategyActionRecord,
        stage: &str,
        order_action: &str,
        order_type: &str,
        requested_status: &str,
        order_id: &str,
        client_order_id: &str,
        message: &str,
        details: Value,
    ) {
        let changed = match update_live_order_management_state_by_identity(
            db,
            &config.mode,
            &config.symbol,
            order_id,
            client_order_id,
            requested_status,
            &LiveOrderExchangeState {
                status: requested_status.to_string(),
                success: false,
                error_message: message.to_string(),
                order_id: order_id.to_string(),
                client_order_id: client_order_id.to_string(),
            },
        )
        .await
        {
            Ok(changed) => changed,
            Err(error) => {
                let reason = format!("保存 OKX 订单管理请求待确认状态失败: {error}");
                self.log_execution_stage(
                    run_id,
                    stage,
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "requested_status": requested_status,
                        "order_id": order_id,
                        "client_order_id": client_order_id,
                    }),
                )
                .await;
                self.set_error(run_id, reason).await;
                return;
            }
        };
        if changed == 0 {
            self.record_order_management_sync_candidate(
                run_id,
                config,
                db,
                action_record,
                stage,
                order_action,
                order_type,
                requested_status,
                order_id,
                client_order_id,
                message,
                false,
            )
            .await;
        }
        self.log_execution_stage(
            run_id,
            stage,
            "warn",
            if changed > 0 {
                message.to_string()
            } else {
                format!("OKX 订单管理请求待确认，但本地未找到可更新订单: {message}")
            },
            serde_json::json!({
                "requested_status": requested_status,
                "order_id": order_id,
                "client_order_id": client_order_id,
                "local_order_records_changed": changed,
                "details": details,
            }),
        )
        .await;
        let mut status = self.inner.status.write().await;
        if status.run_id == run_id {
            status.last_action = requested_status.to_string();
            status.last_action_reason = message.to_string();
            status.error_message.clear();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn record_order_management_sync_candidate(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        action_record: &StrategyActionRecord,
        stage: &str,
        order_action: &str,
        order_type: &str,
        requested_status: &str,
        order_id: &str,
        client_order_id: &str,
        message: &str,
        success: bool,
    ) {
        match insert_live_exchange_order(
            db,
            config,
            "hold",
            order_type,
            0.0,
            action_record.price,
            order_action,
            requested_status,
            success,
            message,
            run_id,
            action_record.timestamp,
            ArrivalQuote::default(),
            order_id,
            client_order_id,
        )
        .await
        {
            Ok(row_id) => {
                self.log_execution_stage(
                    run_id,
                    stage,
                    "info",
                    "本地未找到原订单，已记录订单管理同步候选",
                    serde_json::json!({
                        "local_order_id": row_id,
                        "symbol": config.symbol,
                        "order_action": order_action,
                        "order_type": order_type,
                        "requested_status": requested_status,
                        "order_id": order_id,
                        "client_order_id": client_order_id,
                    }),
                )
                .await;
            }
            Err(error) => {
                let reason = format!("记录订单管理同步候选失败: {error}");
                self.log_execution_stage(
                    run_id,
                    stage,
                    "error",
                    reason.clone(),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "order_action": order_action,
                        "order_type": order_type,
                        "requested_status": requested_status,
                        "order_id": order_id,
                        "client_order_id": client_order_id,
                    }),
                )
                .await;
                self.set_error(run_id, reason).await;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn record_exchange_submit_unknown(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        action_record: &StrategyActionRecord,
        order_side: &str,
        quantity: f64,
        order_type: &str,
        client_order_id: &str,
        arrival: ArrivalQuote,
        error_message: &str,
    ) {
        let message = format!("OKX 下单请求已发出，但响应结果待同步确认: {error_message}");
        let updated = match update_live_exchange_order_state_by_identity_and_symbol(
            db,
            &config.mode,
            &config.symbol,
            "",
            client_order_id,
            &LiveOrderExchangeState {
                status: "submit_unknown".to_string(),
                success: false,
                error_message: message.clone(),
                order_id: String::new(),
                client_order_id: client_order_id.to_string(),
            },
        )
        .await
        {
            Ok(changed) => changed > 0,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "persist",
                    "warn",
                    format!("更新预登记下单待确认记录失败，将尝试插入待确认记录: {error}"),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "client_order_id": client_order_id,
                    }),
                )
                .await;
                false
            }
        };
        if !updated {
            if let Err(error) = insert_live_exchange_order(
                db,
                config,
                order_side,
                order_type,
                quantity,
                action_record.price,
                &action_record.action,
                "submit_unknown",
                false,
                &message,
                run_id,
                action_record.timestamp,
                arrival,
                "",
                client_order_id,
            )
            .await
            {
                self.set_error(run_id, format!("保存实时策略下单待确认记录失败: {error}"))
                    .await;
                return;
            }
        }

        let mut status = self.inner.status.write().await;
        if status.run_id == run_id {
            status.total_orders += 1;
            status.last_action = "submit_unknown".to_string();
            status.last_action_reason = format!(
                "OKX order submit result pending sync; side={order_side}; size={}; clOrdId={client_order_id}",
                order_size_string(quantity)
            );
            status.error_message.clear();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn record_algo_order_submit_unknown(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        action_record: &StrategyActionRecord,
        order_side: &str,
        quantity: f64,
        order_type: &str,
        client_order_id: &str,
        arrival: ArrivalQuote,
        error_message: &str,
    ) {
        let message = format!("OKX 独立保护单提交结果待确认: {error_message}");
        let updated = match update_live_algo_order_exchange_state_by_identity_and_symbol(
            db,
            &config.mode,
            &config.symbol,
            "",
            client_order_id,
            &LiveOrderExchangeState {
                status: "algo_submit_unknown".to_string(),
                success: false,
                error_message: message.clone(),
                order_id: String::new(),
                client_order_id: client_order_id.to_string(),
            },
        )
        .await
        {
            Ok(changed) => changed > 0,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "persist",
                    "warn",
                    format!("更新预登记保护单待确认记录失败，将尝试插入待确认记录: {error}"),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "algo_client_order_id": client_order_id,
                    }),
                )
                .await;
                false
            }
        };
        if !updated {
            if let Err(error) = insert_live_exchange_order(
                db,
                config,
                order_side,
                order_type,
                quantity,
                action_record.price,
                "place_risk_order",
                "algo_submit_unknown",
                false,
                &message,
                run_id,
                action_record.timestamp,
                arrival,
                "",
                client_order_id,
            )
            .await
            {
                self.set_error(run_id, format!("保存保护单待确认记录失败: {error}"))
                    .await;
                return;
            }
        }

        let mut status = self.inner.status.write().await;
        if status.run_id == run_id {
            status.total_orders += 1;
            status.last_action = "algo_submit_unknown".to_string();
            status.last_action_reason = format!(
                "OKX protective algo submit result pending sync; side={order_side}; size={}; algoClOrdId={client_order_id}",
                order_size_string(quantity)
            );
            status.error_message.clear();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn record_exchange_submit_failure(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
        action_record: &StrategyActionRecord,
        order_side: &str,
        quantity: f64,
        order_type: &str,
        client_order_id: &str,
        arrival: ArrivalQuote,
        error_message: &str,
    ) {
        let updated = match update_live_submit_state_by_identity(
            db,
            &config.mode,
            &config.symbol,
            "",
            client_order_id,
            &action_record.action,
            &LiveOrderExchangeState {
                status: "submit_failed".to_string(),
                success: false,
                error_message: error_message.to_string(),
                order_id: String::new(),
                client_order_id: client_order_id.to_string(),
            },
        )
        .await
        {
            Ok(changed) => changed > 0,
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "persist",
                    "warn",
                    format!("更新预登记下单失败记录失败，将尝试插入失败记录: {error}"),
                    serde_json::json!({
                        "symbol": config.symbol,
                        "client_order_id": client_order_id,
                    }),
                )
                .await;
                false
            }
        };
        if updated {
            let mut status = self.inner.status.write().await;
            if status.run_id == run_id {
                status.total_orders += 1;
                status.failed_orders += 1;
                apply_blocked_order_status(
                    &mut status,
                    action_record,
                    "submit_failed",
                    error_message,
                );
            }
            return;
        }
        match insert_live_exchange_order(
            db,
            config,
            order_side,
            order_type,
            quantity,
            action_record.price,
            &action_record.action,
            "submit_failed",
            false,
            error_message,
            run_id,
            action_record.timestamp,
            arrival,
            "",
            client_order_id,
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
                        "submit_failed",
                        error_message,
                    );
                }
            }
            Err(error) => {
                self.set_error(run_id, format!("保存实时策略下单失败记录失败: {error}"))
                    .await;
            }
        }
    }
}

fn attached_algo_trigger_price(algo: &crate::okx::OkxAttachedAlgoOrder) -> Option<f64> {
    algo.sl_trigger_px
        .as_deref()
        .or(algo.tp_trigger_px.as_deref())
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
}

async fn update_live_order_management_state_by_identity(
    db: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
    requested_status: &str,
    state: &LiveOrderExchangeState,
) -> AppResult<u64> {
    if requested_status
        .trim()
        .to_ascii_lowercase()
        .starts_with("algo_")
    {
        update_live_algo_order_exchange_state_by_identity_and_symbol(
            db,
            mode,
            inst_id,
            order_id,
            client_order_id,
            state,
        )
        .await
    } else {
        update_live_exchange_order_state_by_identity_and_symbol(
            db,
            mode,
            inst_id,
            order_id,
            client_order_id,
            state,
        )
        .await
    }
}

async fn update_live_submit_state_by_identity(
    db: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
    action_name: &str,
    state: &LiveOrderExchangeState,
) -> AppResult<u64> {
    if action_name.trim().eq_ignore_ascii_case("place_risk_order") {
        update_live_algo_order_exchange_state_by_identity_and_symbol(
            db,
            mode,
            inst_id,
            order_id,
            client_order_id,
            state,
        )
        .await
    } else {
        update_live_exchange_order_state_by_identity_and_symbol(
            db,
            mode,
            inst_id,
            order_id,
            client_order_id,
            state,
        )
        .await
    }
}
