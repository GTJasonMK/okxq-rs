use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{code_ok, round2, round4, simple_ma, LocalApiRequest},
    error::AppResult,
};

use super::common::{
    cluster_levels, find_swing_points, high_volume_nodes, insufficient_structure_data,
    load_structure_candle_input, MIN_STRUCTURE_CANDLES,
};

/// POST /api/agent/analysis/support-resistance — 支撑阻力分析
pub(crate) async fn analyze_support_resistance(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let input = load_structure_candle_input(state, req, 300).await?;
    if input.closes.len() < MIN_STRUCTURE_CANDLES {
        return Ok(insufficient_structure_data(&input.inst_id, "数据不足"));
    }

    let last_close = input.last_close();
    let price_scale = last_close.max(1.0);

    let (swing_highs_idx, swing_lows_idx) = find_swing_points(&input.highs, &input.lows, 5);
    let swing_high_levels: Vec<_> = swing_highs_idx
        .iter()
        .map(|&i| (i, input.highs[i]))
        .collect();
    let swing_low_levels: Vec<_> = swing_lows_idx.iter().map(|&i| (i, input.lows[i])).collect();

    let resistance_clusters = cluster_levels(&swing_high_levels, price_scale);
    let support_clusters = cluster_levels(&swing_low_levels, price_scale);
    let high_volume_nodes = high_volume_nodes(&input.candles);

    let ma20 = simple_ma(&input.closes, 20);
    let ma50 = simple_ma(&input.closes, 50.min(input.closes.len()));
    let ma200 = simple_ma(&input.closes, 200.min(input.closes.len()));

    let resistances: Vec<Value> = resistance_clusters
        .iter()
        .take(5)
        .map(|(price, touches)| {
            let distance_pct = (price - last_close) / last_close * 100.0;
            json!({"price": round4(*price), "touches": touches, "distance_pct": round2(distance_pct), "type": "resistance"})
        })
        .collect();

    let supports: Vec<Value> = support_clusters
        .iter()
        .take(5)
        .map(|(price, touches)| {
            let distance_pct = (last_close - price) / last_close * 100.0;
            json!({"price": round4(*price), "touches": touches, "distance_pct": round2(distance_pct), "type": "support"})
        })
        .collect();

    let nearest_resistance = resistances.first().and_then(|r| r.get("price").cloned());
    let nearest_support = supports.first().and_then(|s| s.get("price").cloned());

    Ok(code_ok(json!({
        "inst_id": input.inst_id,
        "timeframe": input.timeframe,
        "last_close": round4(last_close),
        "resistances": resistances,
        "supports": supports,
        "nearest_resistance": nearest_resistance,
        "nearest_support": nearest_support,
        "dynamic_levels": {
            "ma20": round4(ma20),
            "ma50": round4(ma50),
            "ma200": round4(ma200),
        },
        "high_volume_nodes": high_volume_nodes,
    })))
}
