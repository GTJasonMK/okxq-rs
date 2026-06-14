use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{
        body_bool, body_i64, body_string, code_ok, infer_inst_type, okx_client,
        request_string_array, request_trading_mode, LocalApiRequest,
    },
    error::{AppError, AppResult},
};

use super::super::scope::{load_latest_candles_for_scope, resolve_agent_scope};
use super::position::query_position_snapshot;

/// POST /api/agent/query/trading-context
pub(crate) async fn query_trading_context(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = body_string(req, "inst_id", "");
    if inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 不能为空".to_string()));
    }
    let inst_type = body_string(req, "inst_type", &infer_inst_type(&inst_id));
    let timeframes = request_string_array(req, "timeframes");
    let tf_list: Vec<String> = if timeframes.is_empty() {
        vec!["5m".into(), "1H".into(), "4H".into()]
    } else {
        timeframes
    };
    let candles_limit = body_i64(req, "candles_limit", 240).clamp(20, 5000);
    let include_orderbook = body_bool(req, "include_orderbook", true);
    let orderbook_depth = body_i64(req, "orderbook_depth", 50).clamp(1, 500);
    let include_recent_trades = body_bool(req, "include_recent_trades", true);
    let recent_trade_limit = body_i64(req, "recent_trade_limit", 50).clamp(1, 100);
    let include_position = body_bool(req, "include_position", false);
    let mode = request_trading_mode(state, req).await?;
    let (inst_id, inst_type) = resolve_agent_scope(state, &inst_id, &inst_type).await?;

    let mut multi_tf = Vec::new();
    for tf in &tf_list {
        let candles =
            load_latest_candles_for_scope(state, &inst_id, &inst_type, tf, candles_limit).await?;
        let candle_objs: Vec<Value> = candles.iter().map(|candle| candle.to_json()).collect();

        let indicators = crate::indicators::IndicatorCalculator::new(&candles).calculate_all();

        multi_tf.push(json!({
            "timeframe": tf,
            "count": candle_objs.len(),
            "candles": candle_objs,
            "indicators": indicators,
        }));
    }

    let orderbook = if include_orderbook {
        if let Ok(client) = okx_client(state).await {
            client
                .get_orderbook(&inst_id, orderbook_depth as u32)
                .await
                .ok()
        } else {
            None
        }
    } else {
        None
    };

    let recent_trades = if include_recent_trades {
        if let Ok(client) = okx_client(state).await {
            client
                .get_trades(&inst_id, recent_trade_limit as u32)
                .await
                .ok()
        } else {
            None
        }
    } else {
        None
    };

    let position = if include_position {
        query_position_snapshot(state, mode.as_str()).await.ok()
    } else {
        None
    };

    let last_price = resolve_trading_context_last_price(&multi_tf);

    Ok(code_ok(json!({
        "inst_id": inst_id,
        "inst_type": inst_type,
        "last_price": last_price,
        "timeframes": multi_tf,
        "orderbook": orderbook,
        "recent_trades": recent_trades,
        "position": position,
    })))
}

fn resolve_trading_context_last_price(multi_tf: &[Value]) -> Option<f64> {
    multi_tf
        .first()
        .and_then(|tf| tf.get("candles"))
        .and_then(|candles| candles.as_array())
        .and_then(|arr| arr.last())
        .and_then(|c| c.get("close"))
        .and_then(|v| v.as_f64())
        .filter(|value| value.is_finite() && *value > 0.0)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::resolve_trading_context_last_price;

    #[test]
    fn trading_context_last_price_is_not_fabricated_for_empty_timeframes() {
        let last_price = resolve_trading_context_last_price(&[]);

        assert!(last_price.is_none());
    }

    #[test]
    fn trading_context_last_price_reads_latest_primary_timeframe_close() {
        let multi_tf = vec![json!({
            "timeframe": "1H",
            "candles": [
                {"close": 100.0},
                {"close": 101.25}
            ]
        })];

        let last_price = resolve_trading_context_last_price(&multi_tf);

        assert_eq!(last_price, Some(101.25));
    }
}
