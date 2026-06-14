use super::super::normalize::*;
use super::super::*;

pub(crate) async fn trading_max_size(
    state: &AppState,
    inst_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let td_mode = param_string(req, "td_mode", "cash");
    let client = okx_private_client(state, &mode).await?;
    let items = client.get_max_size(inst_id, &td_mode).await?;
    Ok(normalize_max_size(items))
}

pub(crate) async fn trading_contract_max_size(
    state: &AppState,
    inst_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mut params = req.params.clone();
    params
        .entry("td_mode".to_string())
        .or_insert_with(|| Value::String("cross".to_string()));
    let next_req = LocalApiRequest {
        method: req.method.clone(),
        path: req.path.clone(),
        params,
        body: req.body.clone(),
    };
    trading_max_size(state, inst_id, &next_req).await
}

pub(crate) async fn trading_contract_leverage(
    state: &AppState,
    inst_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let mgn_mode = param_string(req, "mgn_mode", "cross");
    let client = okx_private_client(state, &mode).await?;
    let items = client.get_leverage(inst_id, &mgn_mode).await?;
    Ok(Value::Array(items))
}

/// GET /api/trading/max-avail-size/{inst_id} — 最大可交易数量
pub(crate) async fn trading_max_avail_size(
    state: &AppState,
    inst_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let td_mode = param_string(req, "td_mode", "cash");
    let client = okx_private_client(state, &mode).await?;
    client.get_max_avail_size(inst_id, &td_mode).await
}
