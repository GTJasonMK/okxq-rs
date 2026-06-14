use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{
        body_i64, body_string, code_ok, infer_inst_type, request_string_array, round2, round4,
        simple_ma, LocalApiRequest,
    },
    error::{AppError, AppResult},
};

use super::super::scope::{load_latest_candles_for_scope, resolve_agent_scope};

/// POST /api/agent/analysis/multi-timeframe-alignment
pub(crate) async fn analyze_multi_timeframe_alignment(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = body_string(req, "inst_id", "");
    let inst_type = body_string(req, "inst_type", &infer_inst_type(&inst_id));
    let timeframes = request_string_array(req, "timeframes");
    let tf_list: Vec<String> = if timeframes.is_empty() {
        vec!["1H".into(), "4H".into(), "1D".into()]
    } else {
        timeframes
    };
    let limit = body_i64(req, "limit", 100).clamp(20, 500);
    if inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 不能为空".to_string()));
    }
    let (inst_id, inst_type) = resolve_agent_scope(state, &inst_id, &inst_type).await?;

    let mut analyses = Vec::new();
    let mut bullish = 0;
    let mut bearish = 0;

    for tf in &tf_list {
        let candles = load_latest_candles_for_scope(state, &inst_id, &inst_type, tf, limit).await?;
        let closes: Vec<f64> = candles
            .iter()
            .map(|candle| candle.close)
            .filter(|v| *v > 0.0)
            .collect();

        if closes.len() < 20 {
            analyses.push(json!({
                "timeframe": tf,
                "bias": "insufficient_data",
                "strength": 0.0,
                "reason": "数据不足",
            }));
            continue;
        }

        let ma20 = simple_ma(&closes, 20);
        let ma60 = simple_ma(&closes, 60.min(closes.len()));
        let last = *closes.last().unwrap_or(&0.0);
        let trend = last - ma20;
        let trend_str = if ma60 > 0.0 {
            (ma20 - ma60) / ma60 * 100.0
        } else {
            0.0
        };

        let bias = if trend > 0.0 && trend_str > 0.0 {
            bullish += 1;
            "bullish"
        } else if trend < 0.0 && trend_str < 0.0 {
            bearish += 1;
            "bearish"
        } else {
            "neutral"
        };

        analyses.push(json!({
            "timeframe": tf,
            "bias": bias,
            "last_close": round4(last),
            "ma20": round4(ma20),
            "ma60": round4(ma60),
            "trend_strength_pct": round2(trend_str),
        }));
    }

    let total = tf_list.len();
    let consensus = if bullish == total {
        "strong_bullish"
    } else if bearish == total {
        "strong_bearish"
    } else if bullish > bearish {
        "weak_bullish"
    } else if bearish > bullish {
        "weak_bearish"
    } else {
        "neutral"
    };

    Ok(code_ok(json!({
        "inst_id": inst_id,
        "timeframes_analyzed": total,
        "bullish_count": bullish,
        "bearish_count": bearish,
        "consensus": consensus,
        "analyses": analyses,
    })))
}
