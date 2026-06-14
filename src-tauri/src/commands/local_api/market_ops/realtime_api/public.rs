use serde_json::Value;

use super::super::*;

pub(in crate::commands::local_api) async fn realtime_status(state: &AppState) -> AppResult<Value> {
    Ok(code_ok(state.realtime.status().await))
}

pub(in crate::commands::local_api) async fn subscribe_realtime_ticker(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = request_inst_id(req);
    ensure_realtime_inst_allowed(state, &inst_id).await?;
    let status = state.realtime.subscribe_ticker(&inst_id).await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn unsubscribe_realtime_ticker(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = request_inst_id(req);
    let status = state.realtime.unsubscribe_ticker(&inst_id).await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn subscribe_realtime_candle(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = request_inst_id(req);
    ensure_realtime_inst_allowed(state, &inst_id).await?;
    let requested_timeframe = normalize_timeframe_name(&request_string(req, "timeframe", "1H"));
    ensure_timeframe_allowed_for_scope(
        state,
        &inst_id,
        &infer_inst_type(&inst_id),
        &requested_timeframe,
    )
    .await?;
    let mut status = state
        .realtime
        .subscribe_candle(&inst_id, &requested_timeframe)
        .await?;
    if let Some(obj) = status.as_object_mut() {
        obj.insert(
            "requested_timeframe".to_string(),
            Value::String(requested_timeframe.clone()),
        );
        obj.insert(
            "source_timeframe".to_string(),
            Value::String(requested_timeframe),
        );
    }
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn unsubscribe_realtime_candle(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = request_inst_id(req);
    let requested_timeframe = normalize_timeframe_name(&request_string(req, "timeframe", "1H"));
    let mut status = state
        .realtime
        .unsubscribe_candle(&inst_id, &requested_timeframe)
        .await?;
    if let Some(obj) = status.as_object_mut() {
        obj.insert(
            "requested_timeframe".to_string(),
            Value::String(requested_timeframe.clone()),
        );
        obj.insert(
            "source_timeframe".to_string(),
            Value::String(requested_timeframe),
        );
    }
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn subscribe_realtime_trades(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = request_inst_id(req);
    ensure_realtime_inst_allowed(state, &inst_id).await?;
    let status = state.realtime.subscribe_trades(&inst_id).await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn unsubscribe_realtime_trades(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = request_inst_id(req);
    let status = state.realtime.unsubscribe_trades(&inst_id).await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn subscribe_realtime_orderbook(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = request_inst_id(req);
    ensure_realtime_inst_allowed(state, &inst_id).await?;
    let channel = request_string(req, "channel", "books5");
    let status = state
        .realtime
        .subscribe_orderbook(&inst_id, &channel)
        .await?;
    Ok(code_ok(status))
}

pub(in crate::commands::local_api) async fn unsubscribe_realtime_orderbook(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = request_inst_id(req);
    let channel = request_string(req, "channel", "books5");
    let status = state
        .realtime
        .unsubscribe_orderbook(&inst_id, &channel)
        .await?;
    Ok(code_ok(status))
}
