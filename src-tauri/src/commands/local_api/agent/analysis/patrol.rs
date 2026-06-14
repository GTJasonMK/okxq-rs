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

/// POST /api/agent/analysis/opportunity-patrol
pub(crate) async fn analyze_opportunity_patrol(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_type =
        resolve_agent_watchlist_inst_type(state, &body_string(req, "inst_type", "")).await?;
    let limit = body_i64(req, "limit", 50).clamp(1, 200);
    let candles_limit = body_i64(req, "candles_limit", 100).clamp(20, 500);

    let symbols = enabled_watchlist_inst_ids(state, &inst_type, Some(limit as usize)).await?;

    let mut opportunities = Vec::new();
    for symbol in &symbols {
        let candles =
            load_latest_candles_for_scope(state, symbol, &inst_type, "1H", candles_limit).await?;
        if candles.len() < 50 {
            continue;
        }

        let closes: Vec<f64> = candles
            .iter()
            .map(|candle| candle.close)
            .filter(|v| *v > 0.0)
            .collect();
        let volumes: Vec<f64> = candles
            .iter()
            .map(|candle| candle.volume.max(0.0))
            .collect();

        if closes.len() < 50 {
            continue;
        }

        let last = *closes.last().unwrap_or(&0.0);
        let rsi = simple_rsi(&closes, 14);
        let ma20 = simple_ma(&closes, 20);
        let ma60 = simple_ma(&closes, 60.min(closes.len()));

        let ma_alignment = if last > ma20 && ma20 > ma60 {
            "bullish"
        } else if last < ma20 && ma20 < ma60 {
            "bearish"
        } else {
            "mixed"
        };

        let breakout_pct = if ma20 > 0.0 {
            (last - ma20) / ma20 * 100.0
        } else {
            0.0
        };

        let vol_avg = volumes.iter().sum::<f64>() / volumes.len() as f64;
        let vol_latest = *volumes.last().unwrap_or(&0.0);
        let volume_surge = vol_avg > 0.0 && vol_latest > vol_avg * 1.5;

        let rsi_oversold = rsi < 30.0;
        let rsi_overbought = rsi > 70.0;

        let mut score = 0;
        let mut signals: Vec<&str> = Vec::new();

        if ma_alignment == "bullish" && breakout_pct.abs() > 1.0 {
            score += 20;
            signals.push("ma_bullish_alignment");
        }
        if volume_surge {
            score += 15;
            signals.push("volume_surge");
        }
        if rsi_oversold {
            score += 25;
            signals.push("rsi_oversold_reversal");
        }
        if rsi_overbought && ma_alignment == "bearish" {
            score += 20;
            signals.push("rsi_overbought_bearish");
        }
        if breakout_pct > 2.0 && volume_surge {
            score += 25;
            signals.push("breakout_with_volume");
        }
        if breakout_pct < -2.0 && volume_surge {
            score += 20;
            signals.push("breakdown_with_volume");
        }

        if score >= 20 {
            opportunities.push(json!({
                "inst_id": symbol,
                "inst_type": inst_type,
                "last_close": round4(last),
                "rsi": round2(rsi),
                "ma20": round4(ma20),
                "ma_alignment": ma_alignment,
                "breakout_pct": round2(breakout_pct),
                "volume_surge": volume_surge,
                "score": score,
                "signals": signals,
            }));
        }
    }

    opportunities.sort_by(|a, b| {
        b.get("score")
            .and_then(|v| v.as_i64())
            .cmp(&a.get("score").and_then(|v| v.as_i64()))
    });

    Ok(code_ok(json!({
        "patrol_time": chrono::Utc::now().to_rfc3339(),
        "total_opportunities": opportunities.len(),
        "opportunities": opportunities,
    })))
}
