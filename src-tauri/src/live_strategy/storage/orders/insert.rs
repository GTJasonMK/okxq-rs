use sqlx::SqlitePool;

use crate::{
    error::{AppError, AppResult},
    okx::generate_okx_client_order_id,
    trading_semantics::exchange_order_type_requires_price_safe,
};

use super::super::super::{arrival::ArrivalQuote, types::LiveStrategyConfig};
use super::identity::{
    ensure_parent_identity_available, identity_scope_for_action,
    order_identity_participates_in_sync,
};

pub(in crate::live_strategy) async fn insert_live_order(
    pool: &SqlitePool,
    config: &LiveStrategyConfig,
    side: &str,
    size: f64,
    price: f64,
    action: &str,
    status: &str,
    success: bool,
    error_message: &str,
    run_id: &str,
    action_timestamp: i64,
    arrival: ArrivalQuote,
) -> AppResult<i64> {
    insert_live_order_with_type(
        pool,
        config,
        side,
        "market",
        size,
        price,
        action,
        status,
        success,
        error_message,
        run_id,
        action_timestamp,
        arrival,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::live_strategy) async fn insert_live_exchange_order(
    pool: &SqlitePool,
    config: &LiveStrategyConfig,
    side: &str,
    order_type: &str,
    size: f64,
    price: f64,
    action: &str,
    status: &str,
    success: bool,
    error_message: &str,
    run_id: &str,
    action_timestamp: i64,
    arrival: ArrivalQuote,
    order_id: &str,
    client_order_id: &str,
) -> AppResult<i64> {
    insert_live_order_record(
        pool,
        config,
        side,
        order_type,
        size,
        price,
        action,
        status,
        success,
        error_message,
        run_id,
        action_timestamp,
        arrival,
        order_id,
        client_order_id,
        "",
        "",
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::live_strategy) async fn insert_live_attached_algo_order(
    pool: &SqlitePool,
    config: &LiveStrategyConfig,
    side: &str,
    order_type: &str,
    size: f64,
    price: f64,
    status: &str,
    success: bool,
    error_message: &str,
    run_id: &str,
    action_timestamp: i64,
    arrival: ArrivalQuote,
    algo_id: &str,
    algo_client_order_id: &str,
    parent_order_id: &str,
    parent_client_order_id: &str,
) -> AppResult<i64> {
    if parent_order_id.trim().is_empty() && parent_client_order_id.trim().is_empty() {
        return Err(AppError::Validation(
            "随单保护单必须带父订单身份".to_string(),
        ));
    }
    insert_live_order_record(
        pool,
        config,
        side,
        order_type,
        size,
        price,
        "place_risk_order",
        status,
        success,
        error_message,
        run_id,
        action_timestamp,
        arrival,
        algo_id,
        algo_client_order_id,
        parent_order_id,
        parent_client_order_id,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::live_strategy) async fn insert_live_order_with_type(
    pool: &SqlitePool,
    config: &LiveStrategyConfig,
    side: &str,
    order_type: &str,
    size: f64,
    price: f64,
    action: &str,
    status: &str,
    success: bool,
    error_message: &str,
    run_id: &str,
    action_timestamp: i64,
    arrival: ArrivalQuote,
) -> AppResult<i64> {
    let order_id = String::new();
    let client_order_id = live_strategy_client_order_id();
    insert_live_order_record(
        pool,
        config,
        side,
        order_type,
        size,
        price,
        action,
        status,
        success,
        error_message,
        run_id,
        action_timestamp,
        arrival,
        &order_id,
        &client_order_id,
        "",
        "",
    )
    .await
}

pub(in crate::live_strategy) fn live_strategy_client_order_id() -> String {
    generate_okx_client_order_id()
}

#[allow(clippy::too_many_arguments)]
async fn insert_live_order_record(
    pool: &SqlitePool,
    config: &LiveStrategyConfig,
    side: &str,
    order_type: &str,
    size: f64,
    price: f64,
    action: &str,
    status: &str,
    success: bool,
    error_message: &str,
    run_id: &str,
    action_timestamp: i64,
    arrival: ArrivalQuote,
    order_id: &str,
    client_order_id: &str,
    parent_order_id: &str,
    parent_client_order_id: &str,
) -> AppResult<i64> {
    if !size.is_finite() || (size <= 0.0 && !live_order_record_allows_unknown_size(status)) {
        return Err(AppError::Validation(
            "实时策略订单数量必须是有限正数".to_string(),
        ));
    }
    if !matches!(
        action.trim(),
        "open_position"
            | "close_position"
            | "place_risk_order"
            | "cancel_order"
            | "modify_order"
            | "hold"
    ) {
        return Err(AppError::Validation(format!(
            "实时策略订单动作必须是规范 actions 协议动作，当前为 {}",
            action.trim()
        )));
    }
    let price = live_order_record_price(action, order_type, price)?;
    if order_identity_participates_in_sync(status, success) {
        ensure_parent_identity_available(
            pool,
            &config.mode,
            &config.symbol,
            identity_scope_for_action(action),
            order_id,
            client_order_id,
            None,
        )
        .await?;
    }

    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, inst_type, side, order_type,
          size, price, order_id, client_order_id, parent_order_id, parent_client_order_id,
          status, action,
          error_message, mode, success, run_id, action_timestamp,
          arrival_ts, arrival_mid_px, arrival_bid_px, arrival_ask_px, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&config.strategy_id)
    .bind(&config.strategy_name)
    .bind(&config.symbol)
    .bind(&config.symbol)
    .bind(config.inst_type.trim().to_ascii_uppercase())
    .bind(side)
    .bind(order_type)
    .bind(size)
    .bind(price)
    .bind(order_id)
    .bind(client_order_id)
    .bind(parent_order_id)
    .bind(parent_client_order_id)
    .bind(status)
    .bind(action)
    .bind(error_message)
    .bind(&config.mode)
    .bind(if success { 1_i64 } else { 0_i64 })
    .bind(run_id)
    .bind(action_timestamp)
    .bind(arrival.ts_ms)
    .bind(arrival.mid_px)
    .bind(arrival.bid_px)
    .bind(arrival.ask_px)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(result.last_insert_rowid())
}

fn live_order_record_price(action: &str, order_type: &str, price: f64) -> AppResult<Option<f64>> {
    if price.is_finite() && price > 0.0 {
        return Ok(Some(price));
    }
    if live_order_record_requires_price(action, order_type) {
        return Err(AppError::Validation(
            "实时策略订单价格必须是有限正数".to_string(),
        ));
    }
    Ok(None)
}

fn live_order_record_requires_price(action: &str, order_type: &str) -> bool {
    if action.trim() == "open_position" {
        return true;
    }
    exchange_order_type_requires_price_safe(order_type)
}

fn live_order_record_allows_unknown_size(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "cancel_requested" | "modify_requested" | "algo_cancel_requested" | "algo_modify_requested"
    )
}
