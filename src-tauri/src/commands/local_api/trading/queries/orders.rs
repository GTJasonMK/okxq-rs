use super::super::normalize::*;
use super::super::*;

pub(crate) async fn trading_orders(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let inst_type = param_string(req, "inst_type", "SPOT");
    let inst_id = param_string(req, "inst_id", "");
    let client = okx_private_client(state, &mode).await?;
    let items = client
        .get_pending_orders(optional_param(&inst_type), optional_param(&inst_id))
        .await?;
    Ok(Value::Array(normalize_orders(items)))
}

pub(crate) async fn trading_order_history(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let inst_type = param_string(req, "inst_type", "SPOT");
    let inst_id = param_string(req, "inst_id", "");
    let limit = param_i64(req, "limit", 50).clamp(1, 100) as u32;
    let client = okx_private_client(state, &mode).await?;
    let items = client
        .get_order_history(optional_param(&inst_type), optional_param(&inst_id), limit)
        .await?;
    Ok(Value::Array(normalize_orders(items)))
}

pub(crate) async fn trading_fills(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let inst_type = param_string(req, "inst_type", "SPOT");
    let inst_id = param_string(req, "inst_id", "");
    let limit = param_i64(req, "limit", 50).clamp(1, 200);
    let mode = request_trading_mode(state, req).await?;
    let client = okx_private_client(state, &mode).await?;
    let items = client
        .get_fills(
            optional_param(&inst_type),
            optional_param(&inst_id),
            limit.clamp(1, 100) as u32,
        )
        .await?;
    Ok(Value::Array(normalize_fills(items)))
}

/// GET /api/trading/order — 查询单个订单详情
pub(crate) async fn trading_get_order(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let inst_id = param_string(req, "inst_id", "");
    let ord_id = param_string(req, "ord_id", "");
    let client_order_id = param_string(req, "cl_ord_id", "");
    if inst_id.is_empty() || (ord_id.is_empty() && client_order_id.is_empty()) {
        return Err(AppError::Validation(
            "inst_id 和 ord_id/cl_ord_id 至少传一个".to_string(),
        ));
    }
    let client = okx_private_client(state, &mode).await?;
    client.get_order(&inst_id, &ord_id, &client_order_id).await
}

/// GET /api/trading/fills-history — 历史成交记录（最近3个月）
pub(crate) async fn trading_fills_history(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let inst_type = param_string(req, "inst_type", "SPOT");
    let inst_id = param_string(req, "inst_id", "");
    let limit = param_i64(req, "limit", 100).clamp(1, 100) as u32;
    let after = param_string(req, "after", "");
    let before = param_string(req, "before", "");
    let client = okx_private_client(state, &mode).await?;
    let items = client
        .get_fills_history(&inst_type, &inst_id, limit, &after, &before)
        .await?;
    Ok(Value::Array(items))
}
