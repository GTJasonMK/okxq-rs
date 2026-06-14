use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{body_i64, body_string, code_ok, request_string_array, LocalApiRequest},
    error::AppResult,
};

use super::super::scope::{
    enabled_watchlist_inst_ids, resolve_agent_scope, resolve_agent_watchlist_inst_type,
};

/// POST /api/agent/analysis/watchlist-correlation
pub(crate) async fn analyze_watchlist_correlation(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_type =
        resolve_agent_watchlist_inst_type(state, &body_string(req, "inst_type", "")).await?;
    let timeframe = body_string(req, "timeframe", "1H");
    let lookback = body_i64(req, "lookback", 100).clamp(20, 200);
    let requested_inst_ids = request_string_array(req, "inst_ids");

    let mut inst_ids = if requested_inst_ids.is_empty() {
        enabled_watchlist_inst_ids(state, &inst_type, None).await?
    } else {
        Vec::new()
    };
    for inst_id in requested_inst_ids {
        let (normalized_id, _) = resolve_agent_scope(state, &inst_id, &inst_type).await?;
        inst_ids.push(normalized_id);
    }

    if inst_ids.len() < 2 {
        return Ok(code_ok(json!({
            "symbols": [],
            "matrix": [],
            "top_positive": [],
            "lowest": [],
            "data_points": 0,
            "message": "至少需要两个币种",
        })));
    }

    for inst_id in &inst_ids {
        super::super::super::market_ops::ensure_local_candles_for_read(
            state, inst_id, &inst_type, &timeframe, lookback, false,
        )
        .await?;
    }

    crate::correlation::compute_correlation_matrix(
        state, &inst_ids, &inst_type, &timeframe, lookback,
    )
    .await
}
