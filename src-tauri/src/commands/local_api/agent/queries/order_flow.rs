use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{
        body_i64, body_string, code_ok, infer_inst_type, okx_client, LocalApiRequest,
    },
    error::{AppError, AppResult},
};

use super::super::scope::resolve_agent_scope;

/// POST /api/agent/query/orderbook
pub(crate) async fn query_orderbook(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let inst_id = body_string(req, "inst_id", "");
    let inst_type = body_string(req, "inst_type", &infer_inst_type(&inst_id));
    let depth = body_i64(req, "depth", 50).clamp(1, 500);
    if inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 不能为空".to_string()));
    }
    let (inst_id, inst_type) = resolve_agent_scope(state, &inst_id, &inst_type).await?;
    let client = okx_client(state).await?;
    let book = match client.get_orderbook(&inst_id, depth as u32).await {
        Ok(book) => book,
        Err(error) => return unavailable_agent_orderbook_result(&inst_id, &inst_type, error),
    };
    Ok(code_ok(json!({
        "inst_id": inst_id,
        "inst_type": inst_type,
        "orderbook": book,
    })))
}

fn unavailable_agent_orderbook_result(
    inst_id: &str,
    inst_type: &str,
    error: impl std::fmt::Display,
) -> AppResult<Value> {
    Err(AppError::Runtime(format!(
        "OKX 盘口不可用: inst_id={inst_id}, inst_type={inst_type}: {error}"
    )))
}

/// POST /api/agent/query/recent-trades
pub(crate) async fn query_recent_trades(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = body_string(req, "inst_id", "");
    let inst_type = body_string(req, "inst_type", &infer_inst_type(&inst_id));
    let limit = body_i64(req, "limit", 50).clamp(1, 100);
    if inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 不能为空".to_string()));
    }
    let (inst_id, inst_type) = resolve_agent_scope(state, &inst_id, &inst_type).await?;
    let client = okx_client(state).await?;
    let trades = match client.get_trades(&inst_id, limit as u32).await {
        Ok(trades) => trades,
        Err(error) => return unavailable_agent_recent_trades_result(&inst_id, &inst_type, error),
    };
    let count = trades.len();
    Ok(code_ok(json!({
        "inst_id": inst_id,
        "inst_type": inst_type,
        "count": count,
        "trades": trades,
    })))
}

fn unavailable_agent_recent_trades_result(
    inst_id: &str,
    inst_type: &str,
    error: impl std::fmt::Display,
) -> AppResult<Value> {
    Err(AppError::Runtime(format!(
        "OKX recent-trades 不可用: inst_id={inst_id}, inst_type={inst_type}: {error}"
    )))
}

#[cfg(test)]
mod tests {
    use super::{unavailable_agent_orderbook_result, unavailable_agent_recent_trades_result};

    #[test]
    fn unavailable_agent_orderbook_is_error_not_empty_success_payload() {
        let error = unavailable_agent_orderbook_result("BTC-USDT-SWAP", "SWAP", "network down")
            .expect_err("unavailable OKX orderbook must not return success payload");

        let message = error.to_string();
        assert!(message.contains("BTC-USDT-SWAP"));
        assert!(message.contains("network down"));
    }

    #[test]
    fn unavailable_agent_recent_trades_is_error_not_empty_success_payload() {
        let error = unavailable_agent_recent_trades_result("BTC-USDT-SWAP", "SWAP", "network down")
            .expect_err("unavailable OKX trades must not return success payload");

        let message = error.to_string();
        assert!(message.contains("BTC-USDT-SWAP"));
        assert!(message.contains("network down"));
    }
}
