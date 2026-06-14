use serde_json::{json, Map, Value};

use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    live_strategy::LiveStrategyStatus,
    okx::OkxCandle,
    strategy_engine::StrategyConfig,
    strategy_executor::{self, types::RuntimeStrategyMeta, RuntimeCandleRequirement},
};

use crate::commands::local_api::{
    helpers::{okx_client, okx_private_client},
    live::candles::{candle_to_json, load_latest_diagnostic_candles},
    market_ops, LocalApiRequest,
};

pub(super) struct LiveStrategyContextInput<'a> {
    pub(super) state: &'a AppState,
    pub(super) req: &'a LocalApiRequest,
    pub(super) runtime_meta: &'a RuntimeStrategyMeta,
    pub(super) config: &'a StrategyConfig,
    pub(super) mode: &'a str,
    pub(super) primary_candles: &'a [OkxCandle],
    pub(super) primary_candles_json: &'a [Value],
    pub(super) limit: usize,
    pub(super) fresh: bool,
    pub(super) runtime_status: &'a LiveStrategyStatus,
}

pub(super) async fn build_live_strategy_context(
    input: LiveStrategyContextInput<'_>,
) -> AppResult<Value> {
    let LiveStrategyContextInput {
        state,
        req,
        runtime_meta,
        config,
        mode,
        primary_candles,
        primary_candles_json,
        limit,
        fresh,
        runtime_status,
    } = input;
    let requirements = strategy_executor::candle_requirements(
        &runtime_meta.data_requirements,
        &config.symbol,
        &config.inst_type,
        &config.timeframe,
        limit.max(primary_candles.len()),
    );
    let mut candle_sets = Vec::new();
    for requirement in requirements {
        let normalized = strategy_executor::normalize_candle_requirement(requirement)?;
        let candles_json = if is_primary_requirement(&normalized, config)
            && primary_candles_json.len() >= normalized.min_bars
        {
            primary_candles_json.to_vec()
        } else {
            load_requirement_candles_json(state, &normalized, limit, fresh).await?
        };
        if candles_json.len() < normalized.min_bars {
            return Err(AppError::Validation(format!(
                "{} {} {} 可用 K 线数量 {} 小于策略 DATA_REQUIREMENTS.min_bars {}",
                normalized.symbol,
                normalized.inst_type,
                normalized.timeframe,
                candles_json.len(),
                normalized.min_bars
            )));
        }
        candle_sets.push((normalized, Value::Array(candles_json)));
    }

    let state_requirements: strategy_executor::RuntimeStateRequirements =
        strategy_executor::state_context_requirements(&runtime_meta.data_requirements);
    let account = match request_object(req, "account") {
        Some(account) => account,
        None if state_requirements.account => {
            if let Some(items) = state
                .realtime
                .latest_private_account_balance_items(mode)
                .await
            {
                crate::live_strategy::state_context::account_context_from_private_items_with_source(
                    items,
                    mode,
                    config.initial_capital,
                    "okx_private_ws_cache",
                )
            } else {
                let client = okx_private_client(state, mode).await.map_err(|error| {
                    AppError::Validation(format!(
                        "策略声明 DATA_REQUIREMENTS.account 需要 {} OKX 私有读取权限: {error}",
                        mode_label(mode)
                    ))
                })?;
                crate::live_strategy::state_context::fetch_private_account_context(
                    &client,
                    mode,
                    config.initial_capital,
                )
                .await
                .map_err(|error| {
                    AppError::Runtime(format!(
                        "获取 {} OKX 私有账户上下文失败: {error}",
                        mode_label(mode)
                    ))
                })?
            }
        }
        None => json!({}),
    };
    let positions = match request_object(req, "positions") {
        Some(positions) => positions,
        None if state_requirements.positions => {
            if let Some(items) = state
                .realtime
                .latest_private_position_raw_items(mode, &config.inst_type)
                .await
            {
                crate::live_strategy::state_context::positions_context_from_private_items_with_source(
                    items,
                    mode,
                    "okx_private_ws_cache",
                )
            } else {
                let client = okx_private_client(state, mode).await.map_err(|error| {
                    AppError::Validation(format!(
                        "策略声明 DATA_REQUIREMENTS.positions 需要 {} OKX 私有读取权限: {error}",
                        mode_label(mode)
                    ))
                })?;
                crate::live_strategy::state_context::fetch_private_positions_context(
                    &client,
                    mode,
                    &config.inst_type,
                )
                .await
                .map_err(|error| {
                    AppError::Runtime(format!(
                        "获取 {} OKX 私有持仓上下文失败: {error}",
                        mode_label(mode)
                    ))
                })?
            }
        }
        None => json!({}),
    };
    let orders = match request_object(req, "orders") {
        Some(orders) => orders,
        None if state_requirements.orders => {
            let local_orders =
                crate::live_strategy::query_live_order_context(&state.db, runtime_status).await?;
            let client = okx_private_client(state, mode).await.map_err(|error| {
                AppError::Validation(format!(
                    "策略声明 DATA_REQUIREMENTS.orders 需要 {} OKX 私有读取权限: {error}",
                    mode_label(mode)
                ))
            })?;
            let order_events = state
                .realtime
                .latest_private_order_raw_items(mode, &config.inst_type)
                .await;
            let fill_events = state
                .realtime
                .latest_private_fill_raw_items(mode, &config.inst_type)
                .await;
            let algo_events = state
                .realtime
                .latest_private_algo_order_raw_items(mode, &config.inst_type)
                .await;
            let rest_orders = crate::live_strategy::state_context::fetch_private_orders_context(
                &client,
                mode,
                &config.inst_type,
            )
            .await
            .map_err(|error| {
                AppError::Runtime(format!(
                    "获取 {} OKX 私有订单上下文失败: {error}",
                    mode_label(mode)
                ))
            })?;
            let private_orders =
                if crate::live_strategy::state_context::private_order_stream_cache_available(
                    &order_events,
                    &algo_events,
                    &fill_events,
                ) {
                    let stream_orders =
                        crate::live_strategy::state_context::orders_context_from_private_stream_items(
                            order_events.unwrap_or_default(),
                            algo_events.unwrap_or_default(),
                            fill_events.unwrap_or_default(),
                            mode,
                            "okx_private_ws_cache",
                        );
                    crate::live_strategy::state_context::merge_private_order_contexts_with_source(
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
                .unwrap_or("okx_private_rest")
                .to_string();
            let merged_source = format!("{private_source}+live_order_records");
            crate::live_strategy::state_context::merge_order_contexts_with_source(
                private_orders,
                local_orders,
                &merged_source,
            )
        }
        None => crate::live_strategy::query_live_order_context(&state.db, runtime_status).await?,
    };
    let timestamp = primary_candles
        .last()
        .expect("live diagnostic context requires primary candles")
        .timestamp;
    let funding = match request_object(req, "funding") {
        Some(funding) => funding,
        None => {
            fetch_funding_context(state, &runtime_meta.data_requirements, config, timestamp).await?
        }
    };
    let orderbook = match request_object(req, "orderbook") {
        Some(orderbook) => orderbook,
        None => fetch_orderbook_context(state, &runtime_meta.data_requirements, config).await?,
    };

    Ok(strategy_executor::strategy_context(
        strategy_executor::StrategyContextInput {
            config: &super::params::strategy_config_json(config),
            candles: strategy_executor::candle_tree(candle_sets),
            timestamp,
            account,
            positions,
            orders,
            funding,
            orderbook,
        },
    ))
}

async fn load_requirement_candles_json(
    state: &AppState,
    requirement: &RuntimeCandleRequirement,
    limit: usize,
    fresh: bool,
) -> AppResult<Vec<Value>> {
    let req_limit = limit.max(requirement.min_bars).clamp(3, 20_000);
    market_ops::ensure_local_candles_for_read(
        state,
        &requirement.symbol,
        &requirement.inst_type,
        &requirement.timeframe,
        req_limit as i64,
        fresh,
    )
    .await?;
    let candles = load_latest_diagnostic_candles(
        state,
        &requirement.symbol,
        &requirement.inst_type,
        &requirement.timeframe,
        req_limit as i64,
    )
    .await?;
    Ok(candles.iter().map(candle_to_json).collect::<Vec<Value>>())
}

fn mode_label(mode: &str) -> &'static str {
    if mode.eq_ignore_ascii_case("live") {
        "live"
    } else {
        "simulated"
    }
}

async fn fetch_funding_context(
    state: &AppState,
    data_requirements: &Value,
    config: &StrategyConfig,
    timestamp: i64,
) -> AppResult<Value> {
    let requirements = strategy_executor::funding_requirements(
        data_requirements,
        &config.symbol,
        &config.inst_type,
    );
    if requirements.is_empty() {
        return Ok(json!({}));
    }

    let client = okx_client(state).await?;
    let funding_table_exists = strategy_executor::local_funding_table_exists(&state.db).await?;
    let mut funding = Map::new();
    for requirement in requirements {
        let normalized = strategy_executor::normalize_funding_requirement(requirement)?;
        let history = strategy_executor::load_local_funding_history_checked(
            &state.db,
            &normalized,
            timestamp,
            normalized.history_limit,
            funding_table_exists,
        )
        .await?;
        if !history.is_empty() {
            funding.insert(
                normalized.symbol,
                strategy_executor::funding_context_value_from_history("okx_funding_rates", history),
            );
            continue;
        }

        match client.get_funding_rate(&normalized.symbol).await {
            Ok(latest) if latest.as_object().is_some_and(|item| !item.is_empty()) => {
                funding.insert(
                    normalized.symbol,
                    strategy_executor::funding_context_value_from_history(
                        "okx_public_current",
                        vec![latest],
                    ),
                );
            }
            Ok(_) if normalized.required => {
                return Err(AppError::Runtime(format!(
                    "{} 资金费率上下文为空，策略声明 DATA_REQUIREMENTS.funding 后不能用空 funding 诊断",
                    normalized.symbol
                )));
            }
            Ok(_) => {}
            Err(error) if normalized.required => {
                return Err(AppError::Runtime(format!(
                    "获取 {} 资金费率上下文失败: {error}",
                    normalized.symbol
                )));
            }
            Err(error) => {
                tracing::debug!(
                    symbol = normalized.symbol.as_str(),
                    error = %error,
                    "optional strategy diagnostics funding context fetch failed"
                );
            }
        }
    }
    Ok(Value::Object(funding))
}

async fn fetch_orderbook_context(
    state: &AppState,
    data_requirements: &Value,
    config: &StrategyConfig,
) -> AppResult<Value> {
    let requirements = strategy_executor::orderbook_requirements(
        data_requirements,
        &config.symbol,
        &config.inst_type,
    );
    if requirements.is_empty() {
        return Ok(json!({}));
    }

    let client = okx_client(state).await?;
    let mut books = Map::new();
    for requirement in requirements {
        let normalized = strategy_executor::normalize_orderbook_requirement(requirement)?;
        match client
            .get_orderbook(&normalized.symbol, normalized.depth as u32)
            .await
        {
            Ok(book) if book.as_object().is_some_and(|item| !item.is_empty()) => {
                books.insert(normalized.symbol, book);
            }
            Ok(_) if normalized.required => {
                return Err(AppError::Runtime(format!(
                    "{} 订单簿上下文为空，策略声明 DATA_REQUIREMENTS.orderbook 后不能用空盘口诊断",
                    normalized.symbol
                )));
            }
            Ok(_) => {}
            Err(error) if normalized.required => {
                return Err(AppError::Runtime(format!(
                    "获取 {} 订单簿上下文失败: {error}",
                    normalized.symbol
                )));
            }
            Err(error) => {
                tracing::debug!(
                    symbol = normalized.symbol.as_str(),
                    error = %error,
                    "optional strategy diagnostics orderbook context fetch failed"
                );
            }
        }
    }
    Ok(Value::Object(books))
}

fn is_primary_requirement(requirement: &RuntimeCandleRequirement, config: &StrategyConfig) -> bool {
    requirement.symbol == config.symbol
        && requirement
            .inst_type
            .eq_ignore_ascii_case(&config.inst_type)
        && requirement.timeframe == config.timeframe
}

fn request_object(req: &LocalApiRequest, key: &str) -> Option<Value> {
    req.body.get(key).filter(|value| value.is_object()).cloned()
}
