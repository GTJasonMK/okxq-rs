use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{code_ok, round2, round4, simple_ma, LocalApiRequest},
    error::AppResult,
};

use super::common::{
    find_swing_points, insufficient_structure_data, load_structure_candle_input,
    swing_structure_from_points, MIN_STRUCTURE_CANDLES,
};

/// POST /api/agent/analysis/market-structure — 市场结构分析
pub(crate) async fn analyze_market_structure(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let input = load_structure_candle_input(state, req, 200).await?;
    if input.closes.len() < MIN_STRUCTURE_CANDLES {
        return Ok(insufficient_structure_data(
            &input.inst_id,
            "数据不足，至少需要30根K线",
        ));
    }

    let swing_window = 5;
    let (swing_highs, swing_lows) = find_swing_points(&input.highs, &input.lows, swing_window);
    let swing_structure =
        swing_structure_from_points(&input.highs, &input.lows, &swing_highs, &swing_lows);
    let last_close = input.last_close();

    let ma20 = simple_ma(&input.closes, 20);
    let ma50 = simple_ma(&input.closes, 50.min(input.closes.len()));
    let ma_trend = if last_close > ma20 && ma20 > ma50 {
        "bullish"
    } else if last_close < ma20 && ma20 < ma50 {
        "bearish"
    } else {
        "neutral"
    };

    let recent_highs: Vec<Value> = swing_highs
        .iter()
        .rev()
        .take(5)
        .map(|&i| json!({"index": i, "price": round4(input.highs[i])}))
        .collect();
    let recent_lows: Vec<Value> = swing_lows
        .iter()
        .rev()
        .take(5)
        .map(|&i| json!({"index": i, "price": round4(input.lows[i])}))
        .collect();

    Ok(code_ok(json!({
        "inst_id": input.inst_id,
        "timeframe": input.timeframe,
        "structure": swing_structure.direction.market_label(),
        "structure_strength_pct": round2(swing_structure.strength_pct),
        "ma_trend": ma_trend,
        "last_close": round4(last_close),
        "ma20": round4(ma20),
        "ma50": round4(ma50),
        "swing_high_count": swing_highs.len(),
        "swing_low_count": swing_lows.len(),
        "recent_swing_highs": recent_highs,
        "recent_swing_lows": recent_lows,
    })))
}
