use std::time::Instant;

use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    error::AppResult,
    okx::{OkxCandle, OkxPrivateClient, OkxPublicClient},
    realtime::RealtimeManager,
    strategy_engine::StrategyConfig,
};

use super::super::types::{LiveStrategyConfig, LiveStrategyStatus};

mod candle_sets;
mod events;
mod funding;
mod orderbook;
mod private_state;
mod types;

use self::{
    candle_sets::{fetch_candle_context_sets, CandleContextSetRequest},
    events::{candle_requirement_shape_counts, elapsed_ms, emit_context_log},
    funding::fetch_funding_context,
    orderbook::fetch_orderbook_context,
    private_state::{account_context, orders_context, positions_context, OrdersContextRequest},
};

pub(super) struct LiveRuntimeContextRequest<'a> {
    pub(super) db: &'a SqlitePool,
    pub(super) client: &'a OkxPublicClient,
    pub(super) data_requirements: &'a Value,
    pub(super) live_config: &'a LiveStrategyConfig,
    pub(super) config: &'a StrategyConfig,
    pub(super) config_json: &'a Value,
    pub(super) primary_candles: &'a [OkxCandle],
    pub(super) primary_candles_json: &'a [Value],
    pub(super) status: &'a LiveStrategyStatus,
    pub(super) private_client: Option<&'a OkxPrivateClient>,
    pub(super) realtime: Option<&'a RealtimeManager>,
}

pub(super) async fn build_live_runtime_context(
    request: LiveRuntimeContextRequest<'_>,
    on_context_event: &mut (dyn FnMut(&Value) + Send),
) -> AppResult<Value> {
    let db = request.db;
    let client = request.client;
    let data_requirements = request.data_requirements;
    let live_config = request.live_config;
    let config = request.config;
    let config_json = request.config_json;
    let primary_candles = request.primary_candles;
    let primary_candles_json = request.primary_candles_json;
    let status = request.status;
    let private_client = request.private_client;
    let realtime = request.realtime;
    let requirements = crate::strategy_executor::candle_requirements(
        data_requirements,
        &config.symbol,
        &config.inst_type,
        &config.timeframe,
        primary_candles.len().max(3),
    );
    let funding_requirements = crate::strategy_executor::funding_requirements(
        data_requirements,
        &config.symbol,
        &config.inst_type,
    );
    let orderbook_requirements = crate::strategy_executor::orderbook_requirements(
        data_requirements,
        &config.symbol,
        &config.inst_type,
    );
    let state_requirements =
        crate::strategy_executor::state_context_requirements(data_requirements);
    let (unique_symbol_count, unique_timeframe_count) =
        candle_requirement_shape_counts(&requirements);
    emit_context_log(
        on_context_event,
        "context",
        "info",
        "策略上下文数据需求已解析",
        json!({
            "candle_requirement_count": requirements.len(),
            "candle_unique_symbol_count": unique_symbol_count,
            "candle_unique_timeframe_count": unique_timeframe_count,
            "funding_requirement_count": funding_requirements.len(),
            "orderbook_requirement_count": orderbook_requirements.len(),
            "account_required": state_requirements.account,
            "positions_required": state_requirements.positions,
            "orders_required": state_requirements.orders,
        }),
    );
    let candle_requirement_count = requirements.len();
    let candles_started = Instant::now();
    emit_context_log(
        on_context_event,
        "context_candles",
        "info",
        format!(
            "开始读取上下文 K 线组，共 {candle_requirement_count} 组（{unique_symbol_count} 个币种，{unique_timeframe_count} 个周期）"
        ),
        json!({
            "requirement_count": candle_requirement_count,
            "unique_symbol_count": unique_symbol_count,
            "unique_timeframe_count": unique_timeframe_count,
        }),
    );
    let candle_sets = fetch_candle_context_sets(
        CandleContextSetRequest {
            db,
            client,
            config,
            requirements,
            primary_candles_json,
            unique_symbol_count,
            unique_timeframe_count,
        },
        on_context_event,
    )
    .await?;
    emit_context_log(
        on_context_event,
        "context_candles",
        "success",
        "上下文 K 线读取完成",
        json!({
            "requirement_count": candle_requirement_count,
            "unique_symbol_count": unique_symbol_count,
            "unique_timeframe_count": unique_timeframe_count,
            "elapsed_ms": elapsed_ms(candles_started),
        }),
    );

    let timestamp = primary_candles
        .last()
        .expect("live runtime context requires primary candles")
        .timestamp;
    let funding = fetch_funding_context(
        db,
        client,
        funding_requirements,
        timestamp,
        on_context_event,
    )
    .await?;
    let orderbook =
        fetch_orderbook_context(client, orderbook_requirements, on_context_event).await?;
    let account = account_context(
        state_requirements,
        live_config,
        status,
        private_client,
        realtime,
        on_context_event,
    )
    .await?;
    let positions = positions_context(
        state_requirements,
        live_config,
        config,
        status,
        private_client,
        realtime,
        on_context_event,
    )
    .await?;
    let orders = orders_context(
        OrdersContextRequest {
            requirements: state_requirements,
            live_config,
            config,
            db,
            status,
            private_client,
            realtime,
        },
        on_context_event,
    )
    .await?;

    Ok(crate::strategy_executor::strategy_context(
        crate::strategy_executor::StrategyContextInput {
            config: config_json,
            candles: crate::strategy_executor::candle_tree(candle_sets),
            timestamp,
            account,
            positions,
            orders,
            funding,
            orderbook,
        },
    ))
}

pub(super) fn candle_to_json(candle: &OkxCandle) -> Value {
    candle.to_json()
}
