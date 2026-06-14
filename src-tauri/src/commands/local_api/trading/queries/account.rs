use super::super::normalize::*;
use super::super::*;

pub(crate) async fn trading_status(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let cfg = state.config.read().await;
    let creds_configured = if mode == "live" {
        cfg.okx.live.is_valid()
    } else {
        cfg.okx.demo.is_valid()
    };
    Ok(json!({
        "mode": mode,
        "default_mode": cfg.okx.default_mode(),
        "api_configured": creds_configured,
        "trading_enabled": creds_configured,
        "mode_locked": mode != cfg.okx.default_mode(),
        "rust_runtime": true,
        "private_read_enabled": creds_configured,
        "message": "Rust OKX 客户端已完整接入（含下单/撤单/设置杠杆等写操作）"
    }))
}

pub(crate) async fn trading_account(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let client = okx_private_client(state, &mode).await?;
    let items = client.get_account_balance().await?;
    Ok(normalize_account_balance(items))
}

pub(crate) async fn trading_holdings_base(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let client = okx_private_client(state, &mode).await?;
    let items = client.get_account_balance().await?;
    Ok(Value::Array(normalize_holdings_from_balance(items)))
}

pub(crate) async fn trading_spot_holdings(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    trading_holdings_base(state, req).await
}

pub(crate) async fn trading_positions(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let inst_type = param_string(req, "inst_type", "");
    let inst_id = param_string(req, "inst_id", "");
    let client = okx_private_client(state, &mode).await?;
    let items = client
        .get_positions(optional_param(&inst_type), optional_param(&inst_id))
        .await?;
    Ok(Value::Array(normalize_positions(items)))
}

pub(crate) async fn trading_contract_positions(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mut params = req.params.clone();
    params
        .entry("inst_type".to_string())
        .or_insert_with(|| Value::String("SWAP".to_string()));
    let next_req = LocalApiRequest {
        method: req.method.clone(),
        path: req.path.clone(),
        params,
        body: req.body.clone(),
    };
    trading_positions(state, &next_req).await
}

pub(crate) async fn trading_contract_account_config(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let client = okx_private_client(state, &mode).await?;
    client.get_account_config().await
}
