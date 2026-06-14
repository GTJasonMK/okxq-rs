use serde_json::Value;

use super::super::*;

pub(in crate::commands::local_api) async fn subscribe_realtime_account(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let credentials = okx_private_credentials(state, &mode).await?;
    let status = state.realtime.subscribe_account(&mode, credentials).await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn unsubscribe_realtime_account(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let status = state.realtime.unsubscribe_account(&mode).await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn subscribe_realtime_private_orders(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let credentials = okx_private_credentials(state, &mode).await?;
    let status = state.realtime.subscribe_orders(&mode, credentials).await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn unsubscribe_realtime_private_orders(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let status = state.realtime.unsubscribe_orders(&mode).await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn subscribe_realtime_private_algo_orders(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let credentials = okx_private_credentials(state, &mode).await?;
    let status = state
        .realtime
        .subscribe_algo_orders(&mode, credentials)
        .await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn unsubscribe_realtime_private_algo_orders(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let status = state.realtime.unsubscribe_algo_orders(&mode).await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn subscribe_realtime_private_fills(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let credentials = okx_private_credentials(state, &mode).await?;
    let status = state.realtime.subscribe_fills(&mode, credentials).await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn unsubscribe_realtime_private_fills(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let status = state.realtime.unsubscribe_fills(&mode).await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn subscribe_realtime_private_positions(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let credentials = okx_private_credentials(state, &mode).await?;
    let status = state
        .realtime
        .subscribe_positions(&mode, credentials)
        .await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn unsubscribe_realtime_private_positions(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let status = state.realtime.unsubscribe_positions(&mode).await?;
    Ok(code_ok(status))
}
