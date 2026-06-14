use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{code_ok, round2, round4, LocalApiRequest},
    error::AppResult,
};

use super::common::{
    atr, fibonacci_levels, find_swing_points, insufficient_structure_data, linear_regression_slope,
    load_structure_candle_input, MIN_STRUCTURE_CANDLES,
};

/// POST /api/agent/analysis/price-projection — 价格预测分析
pub(crate) async fn analyze_price_projection(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let input = load_structure_candle_input(state, req, 200).await?;
    if input.closes.len() < MIN_STRUCTURE_CANDLES {
        return Ok(insufficient_structure_data(&input.inst_id, "数据不足"));
    }

    let last_close = input.last_close();
    let (swing_highs, swing_lows) = find_swing_points(&input.highs, &input.lows, 5);
    let mut fib = json!({});

    if !swing_highs.is_empty() && !swing_lows.is_empty() {
        let most_recent_high = input.highs[*swing_highs.last().unwrap()];
        let most_recent_low = input.lows[*swing_lows.last().unwrap()];
        let is_uptrend = last_close > (most_recent_high + most_recent_low) / 2.0;
        fib = fibonacci_levels(
            most_recent_low.min(most_recent_high),
            most_recent_high.max(most_recent_low),
            is_uptrend,
        );
    }

    let atr_val = atr(&input.highs, &input.lows, &input.closes, 14);
    let atr_pct = if last_close > 0.0 {
        atr_val / last_close * 100.0
    } else {
        0.0
    };
    let projections: Vec<Value> = [1, 2, 3, 5]
        .iter()
        .map(|&multiplier| {
            let upper = last_close + atr_val * multiplier as f64;
            let lower = last_close - atr_val * multiplier as f64;
            json!({
                "periods_ahead": multiplier,
                "upper_bound": round4(upper),
                "lower_bound": round4(lower),
                "range_pct": round2(atr_pct * multiplier as f64),
            })
        })
        .collect();

    let lookback = 20.min(input.closes.len());
    let slice = &input.closes[input.closes.len() - lookback..];
    let slope = linear_regression_slope(slice);
    let trend_proj_5 = last_close + slope * 5.0;
    let trend_proj_10 = last_close + slope * 10.0;

    let trend_direction = if slope > 0.0 {
        "up"
    } else if slope < 0.0 {
        "down"
    } else {
        "flat"
    };

    Ok(code_ok(json!({
        "inst_id": input.inst_id,
        "timeframe": input.timeframe,
        "last_close": round4(last_close),
        "fibonacci": fib,
        "atr": round4(atr_val),
        "atr_pct": round2(atr_pct),
        "volatility_projections": projections,
        "trend_slope": round4(slope),
        "trend_direction": trend_direction,
        "trend_projection_5": round4(trend_proj_5),
        "trend_projection_10": round4(trend_proj_10),
    })))
}
