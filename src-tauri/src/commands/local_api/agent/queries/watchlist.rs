use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{
        body_i64, body_string, code_ok, round2, round4, simple_ma, simple_rsi, LocalApiRequest,
    },
    error::AppResult,
};

use super::super::scope::{
    enabled_watchlist_inst_ids, load_latest_candles_for_scope, resolve_agent_watchlist_inst_type,
};

/// POST /api/agent/query/watchlist-scan
pub(crate) async fn query_watchlist_scan(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_type =
        resolve_agent_watchlist_inst_type(state, &body_string(req, "inst_type", "")).await?;
    let limit = body_i64(req, "limit", 30).clamp(1, 200);
    let candles_limit = body_i64(req, "candles_limit", 200).clamp(20, 2000);
    let sort_by = body_string(req, "sort_by", "signal_score");

    let symbols = enabled_watchlist_inst_ids(state, &inst_type, Some(limit as usize)).await?;

    let mut results = Vec::new();
    for symbol in &symbols {
        let candles =
            load_latest_candles_for_scope(state, symbol, &inst_type, "1H", candles_limit).await?;
        if candles.len() < 20 {
            continue;
        }

        let closes: Vec<f64> = candles
            .iter()
            .map(|candle| candle.close)
            .filter(|v| *v > 0.0)
            .collect();

        let last_close = *closes.last().unwrap_or(&0.0);
        let rsi_val = simple_rsi(&closes, 14);
        let ma20 = simple_ma(&closes, 20);
        let trend_score = if ma20 > 0.0 {
            (last_close - ma20) / ma20 * 100.0
        } else {
            0.0
        };
        let volume: f64 = candles
            .iter()
            .rev()
            .take(5)
            .map(|candle| candle.volume)
            .sum();
        let signal_score = round2(trend_score.abs() * 2.0 + (rsi_val - 50.0).abs() * 0.5);

        results.push(json!({
            "inst_id": symbol,
            "inst_type": inst_type,
            "last_close": round4(last_close),
            "rsi": round2(rsi_val),
            "ma20": round4(ma20),
            "trend_score": round2(trend_score),
            "volume_5bar": round2(volume),
            "signal_score": round2(signal_score),
        }));
    }

    match sort_by.as_str() {
        "volume_24h" => {
            results.sort_by(|a, b| {
                b.get("volume_5bar")
                    .and_then(|v| v.as_f64())
                    .partial_cmp(&a.get("volume_5bar").and_then(|v| v.as_f64()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        _ => {
            results.sort_by(|a, b| {
                b.get("signal_score")
                    .and_then(|v| v.as_f64())
                    .partial_cmp(&a.get("signal_score").and_then(|v| v.as_f64()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    Ok(code_ok(json!({
        "total": results.len(),
        "sort_by": sort_by,
        "items": results,
    })))
}
