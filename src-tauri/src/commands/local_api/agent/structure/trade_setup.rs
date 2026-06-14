use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{code_ok, round2, round4, simple_ma, LocalApiRequest},
    error::AppResult,
};

use super::common::{
    atr, cluster_levels, find_swing_points, insufficient_structure_data,
    load_structure_candle_input, swing_structure_from_points, MIN_STRUCTURE_CANDLES,
};

/// POST /api/agent/analysis/trade-setup — 交易设置建议
pub(crate) async fn analyze_trade_setup(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let input = load_structure_candle_input(state, req, 200).await?;
    if input.closes.len() < MIN_STRUCTURE_CANDLES {
        return Ok(insufficient_structure_data(&input.inst_id, "数据不足"));
    }

    let last_close = input.last_close();
    let price_scale = last_close.max(1.0);

    let (swing_highs, swing_lows) = find_swing_points(&input.highs, &input.lows, 5);
    let swing_structure =
        swing_structure_from_points(&input.highs, &input.lows, &swing_highs, &swing_lows);
    let bias = swing_structure.direction.trade_bias_label();

    let ma20 = simple_ma(&input.closes, 20);
    let ma50 = simple_ma(&input.closes, 50.min(input.closes.len()));
    let ma_alignment = if last_close > ma20 && ma20 > ma50 {
        "bullish_aligned"
    } else if last_close < ma20 && ma20 < ma50 {
        "bearish_aligned"
    } else {
        "not_aligned"
    };

    let swing_high_levels: Vec<_> = swing_highs.iter().map(|&i| (i, input.highs[i])).collect();
    let swing_low_levels: Vec<_> = swing_lows.iter().map(|&i| (i, input.lows[i])).collect();
    let resistances = cluster_levels(&swing_high_levels, price_scale);
    let supports = cluster_levels(&swing_low_levels, price_scale);

    let nearest_resistance = resistances
        .first()
        .map(|r| r.0)
        .unwrap_or(last_close * 1.05);
    let nearest_support = supports.first().map(|s| s.0).unwrap_or(last_close * 0.95);
    let atr_val = atr(&input.highs, &input.lows, &input.closes, 14);

    let (entry, stop_loss, take_profit, direction) = match bias {
        "long" => {
            let sl = (nearest_support - atr_val * 0.5).max(last_close * 0.9);
            let tp1 = nearest_resistance;
            let tp2 = last_close + atr_val * 3.0;
            let tp = tp1.max(tp2);
            (last_close, sl, tp, "long")
        }
        "short" => {
            let sl = (nearest_resistance + atr_val * 0.5).min(last_close * 1.1);
            let tp1 = nearest_support;
            let tp2 = last_close - atr_val * 3.0;
            let tp = tp1.min(tp2);
            (last_close, sl, tp, "short")
        }
        _ => (last_close, nearest_support, nearest_resistance, "neutral"),
    };

    let risk = (entry - stop_loss).abs();
    let reward = (take_profit - entry).abs();
    let risk_reward = if risk > 0.0 { reward / risk } else { 0.0 };
    let entry_pct = if entry > 0.0 {
        risk / entry * 100.0
    } else {
        0.0
    };

    Ok(code_ok(json!({
        "inst_id": input.inst_id,
        "timeframe": input.timeframe,
        "last_close": round4(last_close),
        "bias": bias,
        "ma_alignment": ma_alignment,
        "setup": {
            "direction": direction,
            "entry": round4(entry),
            "stop_loss": round4(stop_loss),
            "take_profit": round4(take_profit),
            "risk_amount": round4(risk),
            "reward_amount": round4(reward),
            "risk_reward_ratio": round2(risk_reward),
            "risk_pct": round2(entry_pct),
        },
        "atr": round4(atr_val),
        "ma20": round4(ma20),
        "ma50": round4(ma50),
        "nearest_resistance": round4(nearest_resistance),
        "nearest_support": round4(nearest_support),
        "confidence": if bias != "neutral" && ma_alignment.contains("aligned") { "high" } else if bias != "neutral" { "medium" } else { "low" },
    })))
}
