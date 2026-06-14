use std::{collections::HashSet, sync::Arc, time::Duration};

use sqlx::SqlitePool;
use tokio::sync::{oneshot, Mutex, RwLock};

use crate::{
    config::ApiCredentials,
    error::{AppError, AppResult},
    okx::{OkxPrivateClient, OkxPublicClient},
    realtime::RealtimeManager,
    strategy_executor,
};

use super::super::{
    runtime_helpers::{canonical_timeframe, finite_or, normalize_timeframe},
    storage::{
        next_live_planned_exit_wakeup, recover_stale_live_planned_exit_claims_from_orders,
        requeue_stale_live_planned_exit_claims,
    },
    types::{LiveStrategyConfig, LiveStrategyStatus},
    LiveStrategyInner, LiveStrategyRuntime,
};

mod subscriptions;
#[cfg(test)]
mod tests;
mod trigger;

use subscriptions::{
    has_portfolio_layers, live_candle_subscriptions, live_trigger_subscriptions,
    portfolio_layers_rejection_message, LivePrivateSubscription, LIVE_PRIVATE_SUBSCRIPTIONS,
};
use trigger::{wait_for_strategy_candle_or_watchdog, LiveLoopTrigger};

const PLANNED_EXIT_IDLE_POLL: Duration = Duration::from_secs(15);
const PLANNED_EXIT_PROCESSING_LEASE: Duration = Duration::from_secs(120);
const PLANNED_EXIT_WORKER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

impl LiveStrategyRuntime {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(LiveStrategyInner {
                status: RwLock::new(LiveStrategyStatus::default()),
                stop_tx: Mutex::new(None),
                submitted_action_keys: Mutex::new(HashSet::new()),
                synced_leverage_keys: Mutex::new(std::collections::HashSet::new()),
                planned_exit_notify: tokio::sync::Notify::new(),
                execution_logs: Mutex::new(std::collections::VecDeque::new()),
                execution_log_seq: Mutex::new(0),
                execution_log_db: RwLock::new(None),
            }),
        }
    }

    pub async fn status(&self) -> LiveStrategyStatus {
        self.inner.status.read().await.clone()
    }

    pub async fn start_with_private_client(
        &self,
        mut config: LiveStrategyConfig,
        db: SqlitePool,
        client: OkxPublicClient,
        private_client: Option<OkxPrivateClient>,
        private_credentials: ApiCredentials,
        realtime: RealtimeManager,
    ) -> AppResult<LiveStrategyStatus> {
        let private_client = private_client.ok_or_else(|| {
            AppError::Validation("实时策略启动必须提供 OKX 私有交易客户端".to_string())
        })?;
        {
            let mut log_db = self.inner.execution_log_db.write().await;
            *log_db = Some(db.clone());
        }
        self.clear_execution_logs().await;
        let runtime_meta =
            strategy_executor::ensure_registered(&config.project_root, &config.strategy_id)?;
        if config.strategy_name.trim().is_empty() {
            config.strategy_name = runtime_meta.strategy_name.clone();
        }
        normalize_live_config_scope(&mut config)?;
        config.check_interval = config.check_interval.clamp(1, 86_400);
        config.position_size = finite_or(config.position_size, 0.2).clamp(0.01, 1.0);
        config.initial_capital = finite_or(config.initial_capital, 10_000.0).max(1.0);
        config.stop_loss = finite_or(config.stop_loss, 0.05).clamp(0.0, 1.0);
        config.take_profit = finite_or(config.take_profit, 0.10).clamp(0.0, 5.0);
        let normalized_timeframe = canonical_timeframe(&config.timeframe)
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "不支持的实时策略 K 线周期: {}",
                    config.timeframe.trim()
                ))
            })?
            .to_string();
        config.timeframe = normalized_timeframe;
        config.risk_timeframe = normalize_timeframe(&config.risk_timeframe);
        let candle_subscriptions =
            live_candle_subscriptions(&config, &runtime_meta.data_requirements)?;
        let trigger_subscriptions = live_trigger_subscriptions(&config, &candle_subscriptions)?;
        if has_portfolio_layers(&config.params) {
            return Err(AppError::Validation(
                portfolio_layers_rejection_message().to_string(),
            ));
        }

        {
            let current = self.inner.status.read().await;
            if matches!(current.status.as_str(), "starting" | "running" | "stopping") {
                return Err(AppError::Validation(format!(
                    "实时策略引擎正在运行: {} @ {}",
                    current.strategy_name, current.symbol
                )));
            }
        }

        let run_id = format!("live_{}", uuid::Uuid::new_v4().simple());
        let strategy_session = strategy_executor::PythonRunnerSession::new(&config.project_root)
            .map_err(|error| {
                AppError::Runtime(format!("启动 Python 策略常驻执行器失败: {error}"))
            })?;
        let private_subscriptions = self
            .subscribe_live_private_channels(
                &run_id,
                &realtime,
                &config.mode,
                private_credentials.clone(),
            )
            .await?;
        self.log_execution_stage(
            &run_id,
            "start",
            "info",
            format!(
                "启动实时策略 {}，目标 {} {}",
                config.strategy_name, config.symbol, config.timeframe
            ),
            serde_json::json!({
                "strategy_id": config.strategy_id,
                "mode": config.mode,
                "symbol": config.symbol,
                "timeframe": config.timeframe,
                "inst_type": config.inst_type,
                "check_interval": config.check_interval,
                "candle_subscriptions": candle_subscriptions,
                "trigger_subscriptions": trigger_subscriptions,
                "python_runner": "persistent_session",
            }),
        )
        .await;

        if let Err(error) = realtime.subscribe_candles(&candle_subscriptions).await {
            self.unsubscribe_live_private_channels(
                &run_id,
                &realtime,
                &config.mode,
                &private_subscriptions,
            )
            .await;
            return Err(AppError::Runtime(format!(
                "订阅实时策略 K 线 WebSocket 失败，已拒绝启动: {error}"
            )));
        }
        self.log_execution_stage(
            &run_id,
            "subscribe",
            "success",
            "实时策略 K 线 WebSocket 订阅完成",
            serde_json::json!({ "candle_subscriptions": candle_subscriptions }),
        )
        .await;

        let (stop_tx, stop_rx) = oneshot::channel();
        {
            let mut guard = self.inner.stop_tx.lock().await;
            *guard = Some(stop_tx);
        }
        {
            let mut submitted_action_keys = self.inner.submitted_action_keys.lock().await;
            submitted_action_keys.clear();
        }
        {
            let mut synced_leverage_keys = self.inner.synced_leverage_keys.lock().await;
            synced_leverage_keys.clear();
        }

        let now = chrono::Utc::now().to_rfc3339();
        let execution_mode = config.runtime_execution_mode().to_string();
        {
            let mut status = self.inner.status.write().await;
            *status = LiveStrategyStatus {
                status: "running".to_string(),
                run_id: run_id.clone(),
                mode: config.mode.clone(),
                strategy_id: config.strategy_id.clone(),
                strategy_name: config.strategy_name.clone(),
                symbol: config.symbol.clone(),
                timeframe: config.timeframe.clone(),
                inst_type: config.inst_type.clone(),
                initial_capital: config.initial_capital,
                position_size: config.position_size,
                stop_loss: config.stop_loss,
                take_profit: config.take_profit,
                params: config.params.clone(),
                start_time: Some(now),
                check_interval: config.check_interval,
                risk_timeframe: config.risk_timeframe.clone(),
                execution_mode,
                ..LiveStrategyStatus::default()
            };
        }

        let runtime = self.clone();
        tauri::async_runtime::spawn(async move {
            runtime
                .run_loop(
                    run_id,
                    config,
                    db,
                    client,
                    private_client,
                    realtime,
                    candle_subscriptions,
                    trigger_subscriptions,
                    private_subscriptions,
                    stop_rx,
                    strategy_session,
                )
                .await;
        });

        Ok(self.status().await)
    }

    pub async fn stop(&self) -> LiveStrategyStatus {
        if let Some(sender) = self.inner.stop_tx.lock().await.take() {
            let _ = sender.send(());
        }
        {
            let mut submitted_action_keys = self.inner.submitted_action_keys.lock().await;
            submitted_action_keys.clear();
        }
        {
            let mut synced_leverage_keys = self.inner.synced_leverage_keys.lock().await;
            synced_leverage_keys.clear();
        }
        let mut status = self.inner.status.write().await;
        if matches!(
            status.status.as_str(),
            "running" | "starting" | "stopping" | "error"
        ) {
            status.status = "stopped".to_string();
        }
        status.error_message.clear();
        let run_id = status.run_id.clone();
        drop(status);
        self.log_execution_stage(
            &run_id,
            "stop",
            "info",
            "已请求停止实时策略",
            serde_json::json!({}),
        )
        .await;
        let status = self.inner.status.read().await;
        status.clone()
    }

    async fn run_loop(
        &self,
        run_id: String,
        config: LiveStrategyConfig,
        db: SqlitePool,
        client: OkxPublicClient,
        private_client: OkxPrivateClient,
        realtime: RealtimeManager,
        candle_subscriptions: Vec<(String, String)>,
        trigger_subscriptions: Vec<(String, String)>,
        private_subscriptions: Vec<LivePrivateSubscription>,
        stop_rx: oneshot::Receiver<()>,
        mut strategy_session: strategy_executor::PythonRunnerSession,
    ) {
        let mut stop_rx = stop_rx;
        let mut candle_events = realtime.subscribe_confirmed_candles();
        let (planned_exit_stop_tx, planned_exit_stop_rx) = oneshot::channel();
        let mut planned_exit_stop_tx = Some(planned_exit_stop_tx);
        let planned_exit_worker = {
            let runtime = self.clone();
            let worker_run_id = run_id.clone();
            let worker_config = config.clone();
            let worker_db = db.clone();
            let worker_client = client.clone();
            let worker_private_client = private_client.clone();
            tauri::async_runtime::spawn(async move {
                runtime
                    .planned_exit_worker_loop(
                        worker_run_id,
                        worker_config,
                        worker_db,
                        worker_client,
                        worker_private_client,
                        planned_exit_stop_rx,
                    )
                    .await;
            })
        };
        self.log_execution_stage(
            &run_id,
            "evaluate",
            "info",
            "启动后立即执行一次策略评估",
            serde_json::json!({ "trigger": "startup" }),
        )
        .await;
        self.sync_exchange_order_states(&run_id, &config, &db, &private_client)
            .await;
        self.evaluate_once(
            &run_id,
            &config,
            &db,
            &client,
            Some(&private_client),
            Some(&realtime),
            Some(&mut strategy_session),
        )
        .await;

        loop {
            let wait_timeout = Duration::from_secs(config.check_interval.max(1));
            tokio::select! {
                _ = &mut stop_rx => {
                    if let Some(sender) = planned_exit_stop_tx.take() {
                        let _ = sender.send(());
                    }
                    self.mark_stopped(&run_id).await;
                    self.log_execution_stage(&run_id, "stop", "success", "实时策略运行循环已停止", serde_json::json!({})).await;
                    break;
                }
                event = wait_for_strategy_candle_or_watchdog(&mut candle_events, &trigger_subscriptions, wait_timeout) => {
                    match event {
                        LiveLoopTrigger::ConfirmedCandle(event) => {
                            self.log_execution_stage(
                                &run_id,
                                "trigger",
                                "info",
                                format!("收到确认 K 线触发评估: {} {}", event.inst_id, event.timeframe),
                                serde_json::json!({
                                    "trigger": "websocket_candle",
                                    "symbol": event.inst_id,
                                    "timeframe": event.timeframe,
                                    "candle_timestamp": event.timestamp,
                                    "trigger_subscriptions": trigger_subscriptions,
                                }),
                            )
                            .await;
                            tracing::debug!(
                                strategy_id = %config.strategy_id,
                                symbol = %event.inst_id,
                                timeframe = %event.timeframe,
                                candle_ts = event.timestamp,
                                "实时策略由 WebSocket 确认 K 线触发"
                            );
                        }
                        LiveLoopTrigger::RestWatchdog => {
                            self.log_execution_stage(
                                &run_id,
                                "trigger",
                                "info",
                                "REST watchdog 到期，触发策略评估",
                                serde_json::json!({
                                    "trigger": "rest_watchdog",
                                    "check_interval": config.check_interval,
                                }),
                            )
                            .await;
                            tracing::debug!(
                                strategy_id = %config.strategy_id,
                                symbol = %config.symbol,
                                timeframe = %config.timeframe,
                                "实时策略由 REST watchdog 触发"
                            );
                        }
                    }
                    self.sync_exchange_order_states(&run_id, &config, &db, &private_client).await;
                    self.evaluate_once(
                        &run_id,
                        &config,
                        &db,
                        &client,
                        Some(&private_client),
                        Some(&realtime),
                        Some(&mut strategy_session),
                    ).await;
                }
            }
        }
        if let Some(sender) = planned_exit_stop_tx.take() {
            let _ = sender.send(());
        }
        match tokio::time::timeout(PLANNED_EXIT_WORKER_SHUTDOWN_TIMEOUT, planned_exit_worker).await
        {
            Ok(_) => {}
            Err(_) => {
                self.log_execution_stage(
                    &run_id,
                    "planned_exit_worker",
                    "warn",
                    "计划退出 worker 停止超时，已放弃等待",
                    serde_json::json!({}),
                )
                .await;
            }
        }

        if let Err(error) = realtime.unsubscribe_candles(&candle_subscriptions).await {
            self.log_execution_stage(
                &run_id,
                "unsubscribe",
                "warn",
                format!("释放实时策略 K 线 WebSocket 订阅失败: {error}"),
                serde_json::json!({}),
            )
            .await;
            tracing::warn!(
                strategy_id = %config.strategy_id,
                symbol = %config.symbol,
                timeframe = %config.timeframe,
                error = %error,
                "释放实时策略 K 线 WebSocket 订阅失败"
            );
        } else {
            self.log_execution_stage(
                &run_id,
                "unsubscribe",
                "success",
                "实时策略 K 线 WebSocket 订阅已释放",
                serde_json::json!({}),
            )
            .await;
        }
        self.unsubscribe_live_private_channels(
            &run_id,
            &realtime,
            &config.mode,
            &private_subscriptions,
        )
        .await;
    }

    async fn subscribe_live_private_channels(
        &self,
        run_id: &str,
        realtime: &RealtimeManager,
        mode: &str,
        credentials: ApiCredentials,
    ) -> AppResult<Vec<LivePrivateSubscription>> {
        let mut subscribed = Vec::new();
        for channel in LIVE_PRIVATE_SUBSCRIPTIONS {
            if let Err(error) = channel.subscribe(realtime, mode, credentials.clone()).await {
                self.unsubscribe_live_private_channels(run_id, realtime, mode, &subscribed)
                    .await;
                return Err(AppError::Runtime(format!(
                    "订阅实时策略 OKX 私有 {} WebSocket 失败，已拒绝启动: {error}",
                    channel.label()
                )));
            }
            subscribed.push(channel);
        }
        self.log_execution_stage(
            run_id,
            "subscribe_private",
            "success",
            "实时策略 OKX 私有 WebSocket 订阅完成",
            serde_json::json!({
                "mode": mode,
                "channels": subscribed.iter().map(|item| item.channel_key()).collect::<Vec<_>>(),
            }),
        )
        .await;
        Ok(subscribed)
    }

    async fn unsubscribe_live_private_channels(
        &self,
        run_id: &str,
        realtime: &RealtimeManager,
        mode: &str,
        subscriptions: &[LivePrivateSubscription],
    ) {
        let mut failed = Vec::new();
        for channel in subscriptions.iter().rev().copied() {
            if let Err(error) = channel.unsubscribe(realtime, mode).await {
                failed.push(serde_json::json!({
                    "channel": channel.channel_key(),
                    "error": error.to_string(),
                }));
            }
        }
        if failed.is_empty() {
            self.log_execution_stage(
                run_id,
                "unsubscribe_private",
                "success",
                "实时策略 OKX 私有 WebSocket 订阅已释放",
                serde_json::json!({
                    "mode": mode,
                    "channels": subscriptions.iter().map(|item| item.channel_key()).collect::<Vec<_>>(),
                }),
            )
            .await;
        } else {
            self.log_execution_stage(
                run_id,
                "unsubscribe_private",
                "warn",
                "释放实时策略 OKX 私有 WebSocket 订阅时部分通道失败",
                serde_json::json!({
                    "mode": mode,
                    "failed": failed,
                }),
            )
            .await;
        }
    }

    async fn planned_exit_worker_loop(
        &self,
        run_id: String,
        config: LiveStrategyConfig,
        db: SqlitePool,
        client: OkxPublicClient,
        private_client: OkxPrivateClient,
        stop_rx: oneshot::Receiver<()>,
    ) {
        let mut stop_rx = stop_rx;
        self.log_execution_stage(
            &run_id,
            "planned_exit_worker",
            "info",
            "计划退出 worker 已启动，独立处理到期平仓",
            serde_json::json!({ "strategy_id": config.strategy_id, "mode": config.mode }),
        )
        .await;
        loop {
            self.requeue_stale_planned_exit_claims(&run_id, &config, &db)
                .await;
            let next_wakeup =
                match next_live_planned_exit_wakeup(&db, &config.mode, &config.strategy_id).await {
                    Ok(value) => value,
                    Err(error) => {
                        self.log_execution_stage(
                            &run_id,
                            "planned_exit_worker",
                            "warn",
                            format!("读取下一个计划退出时间失败，稍后重试: {error}"),
                            serde_json::json!({}),
                        )
                        .await;
                        None
                    }
                };
            let wait_duration = planned_exit_worker_wait_duration(
                chrono::Utc::now().timestamp_millis(),
                next_wakeup,
            );
            if wait_duration.is_zero() {
                self.sync_exchange_order_states(&run_id, &config, &db, &private_client)
                    .await;
                self.process_due_planned_exits(&run_id, &config, &db, &client, &private_client)
                    .await;
                continue;
            }
            tokio::select! {
                _ = &mut stop_rx => {
                    self.log_execution_stage(
                        &run_id,
                        "planned_exit_worker",
                        "success",
                        "计划退出 worker 已停止",
                        serde_json::json!({}),
                    )
                    .await;
                    break;
                }
                _ = self.inner.planned_exit_notify.notified() => {
                    continue;
                }
                _ = tokio::time::sleep(wait_duration) => {
                    continue;
                }
            }
        }
    }

    async fn requeue_stale_planned_exit_claims(
        &self,
        run_id: &str,
        config: &LiveStrategyConfig,
        db: &SqlitePool,
    ) {
        let lease_ms = i64::try_from(PLANNED_EXIT_PROCESSING_LEASE.as_millis())
            .unwrap_or(120_000)
            .max(1);
        let stale_before =
            (chrono::Utc::now() - chrono::Duration::milliseconds(lease_ms)).to_rfc3339();
        let retry_at = chrono::Utc::now().timestamp_millis();
        match recover_stale_live_planned_exit_claims_from_orders(
            db,
            &config.mode,
            &config.strategy_id,
            &stale_before,
            retry_at,
        )
        .await
        {
            Ok(0) => {}
            Ok(count) => {
                self.log_execution_stage(
                    run_id,
                    "planned_exit_worker",
                    "warn",
                    format!("发现 {count} 个超时处理中计划已有平仓订单记录，已恢复提交状态"),
                    serde_json::json!({
                        "recovered": count,
                        "lease_ms": lease_ms,
                    }),
                )
                .await;
                self.inner.planned_exit_notify.notify_one();
            }
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "planned_exit_worker",
                    "warn",
                    format!("根据本地订单恢复超时计划退出失败，稍后重试: {error}"),
                    serde_json::json!({ "lease_ms": lease_ms }),
                )
                .await;
            }
        }

        match requeue_stale_live_planned_exit_claims(
            db,
            &config.mode,
            &config.strategy_id,
            &stale_before,
            retry_at,
        )
        .await
        {
            Ok(0) => {}
            Ok(count) => {
                self.log_execution_stage(
                    run_id,
                    "planned_exit_worker",
                    "warn",
                    format!("发现 {count} 个超时的计划退出处理状态，已重新排队"),
                    serde_json::json!({
                        "requeued": count,
                        "lease_ms": lease_ms,
                        "retry_at": retry_at,
                    }),
                )
                .await;
                self.inner.planned_exit_notify.notify_one();
            }
            Err(error) => {
                self.log_execution_stage(
                    run_id,
                    "planned_exit_worker",
                    "warn",
                    format!("恢复超时计划退出失败，稍后重试: {error}"),
                    serde_json::json!({ "lease_ms": lease_ms }),
                )
                .await;
            }
        }
    }
}

fn normalize_live_config_scope(config: &mut LiveStrategyConfig) -> AppResult<()> {
    let raw_symbol = config.symbol.trim();
    let mut inst_type = config.inst_type.trim().to_ascii_uppercase();
    if inst_type.is_empty() {
        inst_type = if raw_symbol.to_ascii_uppercase().ends_with("-SWAP") {
            "SWAP".to_string()
        } else {
            "SPOT".to_string()
        };
    }
    config.inst_type = inst_type;
    config.symbol = strategy_executor::normalize_runtime_inst_id(raw_symbol, &config.inst_type)?;
    Ok(())
}

fn planned_exit_worker_wait_duration(now_ms: i64, next_wakeup: Option<i64>) -> Duration {
    match next_wakeup {
        Some(next_wakeup) if next_wakeup <= now_ms => Duration::ZERO,
        Some(next_wakeup) => Duration::from_millis(next_wakeup.saturating_sub(now_ms) as u64),
        None => PLANNED_EXIT_IDLE_POLL,
    }
}
