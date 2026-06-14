use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    error::{AppError, AppResult},
    okx::OkxPrivateClient,
    realtime::RealtimeManager,
    strategy_engine::StrategyConfig,
    strategy_executor::RuntimeStateRequirements,
};

use super::{
    super::super::{
        state_context, storage,
        types::{LiveStrategyConfig, LiveStrategyStatus},
    },
    events::{elapsed_ms, emit_context_log},
};

pub(super) async fn account_context(
    requirements: RuntimeStateRequirements,
    config: &LiveStrategyConfig,
    _status: &LiveStrategyStatus,
    private_client: Option<&OkxPrivateClient>,
    realtime: Option<&RealtimeManager>,
    on_context_event: &mut (dyn FnMut(&Value) + Send),
) -> AppResult<Value> {
    if !requirements.account {
        return Ok(json!({}));
    }
    let started = std::time::Instant::now();
    emit_context_log(
        on_context_event,
        "context_account",
        "info",
        format!("开始读取 {} 账户上下文", mode_label(&config.mode)),
        json!({
            "mode": config.mode,
        }),
    );
    if let Some(realtime) = realtime {
        if let Some(items) = realtime
            .latest_private_account_balance_items(&config.mode)
            .await
        {
            let account = state_context::account_context_from_private_items_with_source(
                items,
                &config.mode,
                config.initial_capital,
                "okx_private_ws_cache",
            );
            emit_context_log(
                on_context_event,
                "context_account",
                "success",
                "账户上下文读取完成",
                json!({
                    "mode": config.mode,
                    "source": "okx_private_ws_cache",
                    "elapsed_ms": elapsed_ms(started),
                }),
            );
            return Ok(account);
        }
    }
    let Some(client) = private_client else {
        emit_context_log(
            on_context_event,
            "context_account",
            "error",
            format!(
                "策略需要 {} 账户上下文，但未配置私有读取客户端",
                mode_label(&config.mode)
            ),
            json!({
                "mode": config.mode,
                "elapsed_ms": elapsed_ms(started),
            }),
        );
        return Err(AppError::Runtime(format!(
            "策略声明 DATA_REQUIREMENTS.account 需要 {} OKX 私有账户上下文，但运行时未配置私有读取客户端",
            mode_label(&config.mode)
        )));
    };
    match state_context::fetch_private_account_context(client, &config.mode, config.initial_capital)
        .await
    {
        Ok(account) => {
            emit_context_log(
                on_context_event,
                "context_account",
                "success",
                "账户上下文读取完成",
                json!({
                    "mode": config.mode,
                    "source": "okx_private_rest",
                    "elapsed_ms": elapsed_ms(started),
                }),
            );
            Ok(account)
        }
        Err(error) => {
            emit_context_log(
                on_context_event,
                "context_account",
                "error",
                format!(
                    "获取 {} OKX 私有账户上下文失败: {error}",
                    mode_label(&config.mode)
                ),
                json!({
                    "mode": config.mode,
                    "elapsed_ms": elapsed_ms(started),
                }),
            );
            Err(AppError::Runtime(format!(
                "获取 {} OKX 私有账户上下文失败: {error}",
                mode_label(&config.mode)
            )))
        }
    }
}

pub(super) async fn positions_context(
    requirements: RuntimeStateRequirements,
    live_config: &LiveStrategyConfig,
    config: &StrategyConfig,
    _status: &LiveStrategyStatus,
    private_client: Option<&OkxPrivateClient>,
    realtime: Option<&RealtimeManager>,
    on_context_event: &mut (dyn FnMut(&Value) + Send),
) -> AppResult<Value> {
    if !requirements.positions {
        return Ok(json!({}));
    }
    let started = std::time::Instant::now();
    emit_context_log(
        on_context_event,
        "context_positions",
        "info",
        format!("开始读取 {} 持仓上下文", mode_label(&live_config.mode)),
        json!({
            "mode": live_config.mode,
            "inst_type": config.inst_type,
        }),
    );
    if let Some(realtime) = realtime {
        if let Some(items) = realtime
            .latest_private_position_raw_items(&live_config.mode, &config.inst_type)
            .await
        {
            let positions = state_context::positions_context_from_private_items_with_source(
                items,
                &live_config.mode,
                "okx_private_ws_cache",
            );
            emit_context_log(
                on_context_event,
                "context_positions",
                "success",
                "持仓上下文读取完成",
                json!({
                    "mode": live_config.mode,
                    "inst_type": config.inst_type,
                    "source": "okx_private_ws_cache",
                    "elapsed_ms": elapsed_ms(started),
                }),
            );
            return Ok(positions);
        }
    }
    let Some(client) = private_client else {
        emit_context_log(
            on_context_event,
            "context_positions",
            "error",
            format!(
                "策略需要 {} 持仓上下文，但未配置私有读取客户端",
                mode_label(&live_config.mode)
            ),
            json!({
                "mode": live_config.mode,
                "inst_type": config.inst_type,
                "elapsed_ms": elapsed_ms(started),
            }),
        );
        return Err(AppError::Runtime(format!(
            "策略声明 DATA_REQUIREMENTS.positions 需要 {} OKX 私有持仓上下文，但运行时未配置私有读取客户端",
            mode_label(&live_config.mode)
        )));
    };
    match state_context::fetch_private_positions_context(
        client,
        &live_config.mode,
        &config.inst_type,
    )
    .await
    {
        Ok(positions) => {
            emit_context_log(
                on_context_event,
                "context_positions",
                "success",
                "持仓上下文读取完成",
                json!({
                    "mode": live_config.mode,
                    "inst_type": config.inst_type,
                    "source": "okx_private_rest",
                    "elapsed_ms": elapsed_ms(started),
                }),
            );
            Ok(positions)
        }
        Err(error) => {
            emit_context_log(
                on_context_event,
                "context_positions",
                "error",
                format!(
                    "获取 {} OKX 私有持仓上下文失败: {error}",
                    mode_label(&live_config.mode)
                ),
                json!({
                    "mode": live_config.mode,
                    "inst_type": config.inst_type,
                    "elapsed_ms": elapsed_ms(started),
                }),
            );
            Err(AppError::Runtime(format!(
                "获取 {} OKX 私有持仓上下文失败: {error}",
                mode_label(&live_config.mode)
            )))
        }
    }
}

pub(super) struct OrdersContextRequest<'a> {
    pub(super) requirements: RuntimeStateRequirements,
    pub(super) live_config: &'a LiveStrategyConfig,
    pub(super) config: &'a StrategyConfig,
    pub(super) db: &'a SqlitePool,
    pub(super) status: &'a LiveStrategyStatus,
    pub(super) private_client: Option<&'a OkxPrivateClient>,
    pub(super) realtime: Option<&'a RealtimeManager>,
}

pub(super) async fn orders_context(
    request: OrdersContextRequest<'_>,
    on_context_event: &mut (dyn FnMut(&Value) + Send),
) -> AppResult<Value> {
    let requirements = request.requirements;
    let live_config = request.live_config;
    let config = request.config;
    let started = std::time::Instant::now();
    emit_context_log(
        on_context_event,
        "context_orders",
        "info",
        "开始读取订单上下文",
        json!({
            "mode": live_config.mode,
            "inst_type": config.inst_type,
            "private_required": requirements.orders,
        }),
    );
    let local_orders = storage::query_live_order_context(request.db, request.status).await?;
    if !requirements.orders {
        emit_context_log(
            on_context_event,
            "context_orders",
            "success",
            "本地订单上下文读取完成",
            json!({
                "mode": live_config.mode,
                "private_required": false,
                "elapsed_ms": elapsed_ms(started),
            }),
        );
        return Ok(local_orders);
    }
    let Some(client) = request.private_client else {
        emit_context_log(
            on_context_event,
            "context_orders",
            "error",
            format!(
                "策略需要 {} 订单上下文，但未配置私有读取客户端",
                mode_label(&live_config.mode)
            ),
            json!({
                "mode": live_config.mode,
                "inst_type": config.inst_type,
                "elapsed_ms": elapsed_ms(started),
            }),
        );
        return Err(AppError::Runtime(format!(
            "策略声明 DATA_REQUIREMENTS.orders 需要 {} OKX 私有订单上下文，但运行时未配置私有读取客户端",
            mode_label(&live_config.mode)
        )));
    };
    let (order_events, fill_events, algo_events) = if let Some(realtime) = request.realtime {
        (
            realtime
                .latest_private_order_raw_items(&live_config.mode, &config.inst_type)
                .await,
            realtime
                .latest_private_fill_raw_items(&live_config.mode, &config.inst_type)
                .await,
            realtime
                .latest_private_algo_order_raw_items(&live_config.mode, &config.inst_type)
                .await,
        )
    } else {
        (None, None, None)
    };
    let rest_orders = match state_context::fetch_private_orders_context(
        client,
        &live_config.mode,
        &config.inst_type,
    )
    .await
    {
        Ok(orders) => orders,
        Err(error) => {
            emit_context_log(
                on_context_event,
                "context_orders",
                "error",
                format!(
                    "获取 {} OKX 私有订单上下文失败: {error}",
                    mode_label(&live_config.mode)
                ),
                json!({
                    "mode": live_config.mode,
                    "inst_type": config.inst_type,
                    "elapsed_ms": elapsed_ms(started),
                }),
            );
            return Err(AppError::Runtime(format!(
                "获取 {} OKX 私有订单上下文失败: {error}",
                mode_label(&live_config.mode)
            )));
        }
    };
    let private_orders = if state_context::private_order_stream_cache_available(
        &order_events,
        &algo_events,
        &fill_events,
    ) {
        let stream_orders = state_context::orders_context_from_private_stream_items(
            order_events.unwrap_or_default(),
            algo_events.unwrap_or_default(),
            fill_events.unwrap_or_default(),
            &live_config.mode,
            "okx_private_ws_cache",
        );
        state_context::merge_private_order_contexts_with_source(
            stream_orders,
            rest_orders,
            "okx_private_ws_cache+okx_private_rest",
        )
    } else {
        rest_orders
    };
    let private_source = private_orders
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Runtime("OKX 私有订单上下文缺少 source 字段".to_string()))?
        .to_string();
    let open_count = order_context_array_len(&private_orders, "open")?;
    let recent_fills_count = order_context_array_len(&private_orders, "recent_fills")?;
    let recent_rejections_count = order_context_array_len(&private_orders, "recent_rejections")?;
    emit_context_log(
        on_context_event,
        "context_orders",
        "success",
        "订单上下文读取完成",
        json!({
            "mode": live_config.mode,
            "inst_type": config.inst_type,
            "private_required": true,
            "source": private_source,
            "open": open_count,
            "recent_fills": recent_fills_count,
            "recent_rejections": recent_rejections_count,
            "elapsed_ms": elapsed_ms(started),
        }),
    );
    let merged_source = format!("{private_source}+live_order_records");
    Ok(state_context::merge_order_contexts_with_source(
        private_orders,
        local_orders,
        &merged_source,
    ))
}

fn order_context_array_len(context: &Value, key: &str) -> AppResult<usize> {
    context
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::len)
        .ok_or_else(|| AppError::Runtime(format!("OKX 私有订单上下文缺少 {key} 数组")))
}

fn mode_label(mode: &str) -> &'static str {
    if mode.eq_ignore_ascii_case("live") {
        "live"
    } else {
        "simulated"
    }
}
