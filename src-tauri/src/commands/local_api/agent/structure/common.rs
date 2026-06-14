use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{
        body_i64, body_string, code_ok, infer_inst_type, round2, round4, LocalApiRequest,
    },
    error::{AppError, AppResult},
    okx::OkxCandle,
};

use super::super::scope::{candle_ohlc_series, load_agent_candles};

pub(super) const MIN_STRUCTURE_CANDLES: usize = 30;

pub(super) struct StructureCandleInput {
    pub(super) inst_id: String,
    pub(super) timeframe: String,
    pub(super) candles: Vec<OkxCandle>,
    pub(super) highs: Vec<f64>,
    pub(super) lows: Vec<f64>,
    pub(super) closes: Vec<f64>,
}

impl StructureCandleInput {
    pub(super) fn last_close(&self) -> f64 {
        *self.closes.last().unwrap_or(&0.0)
    }
}

pub(super) async fn load_structure_candle_input(
    state: &AppState,
    req: &LocalApiRequest,
    default_limit: i64,
) -> AppResult<StructureCandleInput> {
    let inst_id = body_string(req, "inst_id", "");
    let inferred_type = infer_inst_type(&inst_id);
    let inst_type = body_string(req, "inst_type", &inferred_type);
    let timeframe = body_string(req, "timeframe", "1H");
    let limit = body_i64(req, "limit", default_limit).clamp(50, 500);
    if inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 不能为空".to_string()));
    }

    let (inst_id, _inst_type, candles) =
        load_agent_candles(state, &inst_id, &inst_type, &timeframe, limit).await?;
    let (highs, lows, closes) = candle_ohlc_series(&candles);
    Ok(StructureCandleInput {
        inst_id,
        timeframe,
        candles,
        highs,
        lows,
        closes,
    })
}

pub(super) fn insufficient_structure_data(inst_id: &str, message: &str) -> Value {
    code_ok(json!({"inst_id": inst_id, "status": "insufficient_data", "message": message}))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SwingStructureDirection {
    Bullish,
    Bearish,
    Consolidation,
}

impl SwingStructureDirection {
    pub(super) fn market_label(self) -> &'static str {
        match self {
            Self::Bullish => "bullish",
            Self::Bearish => "bearish",
            Self::Consolidation => "consolidation",
        }
    }

    pub(super) fn trade_bias_label(self) -> &'static str {
        match self {
            Self::Bullish => "long",
            Self::Bearish => "short",
            Self::Consolidation => "neutral",
        }
    }
}

pub(super) struct SwingStructure {
    pub(super) direction: SwingStructureDirection,
    pub(super) strength_pct: f64,
}

pub(super) fn swing_structure_from_points(
    highs: &[f64],
    lows: &[f64],
    swing_highs: &[usize],
    swing_lows: &[usize],
) -> SwingStructure {
    if swing_highs.len() < 2 || swing_lows.len() < 2 {
        return SwingStructure {
            direction: SwingStructureDirection::Consolidation,
            strength_pct: 0.0,
        };
    }

    let h1 = highs[swing_highs[swing_highs.len() - 2]];
    let h2 = highs[*swing_highs.last().unwrap()];
    let l1 = lows[swing_lows[swing_lows.len() - 2]];
    let l2 = lows[*swing_lows.last().unwrap()];

    let hh = h2 > h1;
    let hl = l2 > l1;
    let lh = h2 < h1;
    let ll = l2 < l1;

    if hh && hl {
        SwingStructure {
            direction: SwingStructureDirection::Bullish,
            strength_pct: ((h2 - h1) / h1 + (l2 - l1) / l1) / 2.0 * 100.0,
        }
    } else if lh && ll {
        SwingStructure {
            direction: SwingStructureDirection::Bearish,
            strength_pct: ((h1 - h2) / h1 + (l1 - l2) / l1) / 2.0 * 100.0,
        }
    } else {
        SwingStructure {
            direction: SwingStructureDirection::Consolidation,
            strength_pct: 0.0,
        }
    }
}

/// 找摆动高点和低点。窗口=5（左右各5根K线）。
pub(super) fn find_swing_points(
    highs: &[f64],
    lows: &[f64],
    window: usize,
) -> (Vec<usize>, Vec<usize>) {
    let mut swing_highs = Vec::new();
    let mut swing_lows = Vec::new();
    let n = highs.len();
    if n < window * 2 + 1 {
        return (swing_highs, swing_lows);
    }
    for i in window..n - window {
        let h = highs[i];
        let l = lows[i];
        let is_swing_high = (i - window..i + window + 1).all(|j| j == i || highs[j] <= h);
        let is_swing_low = (i - window..i + window + 1).all(|j| j == i || lows[j] >= l);
        if is_swing_high {
            swing_highs.push(i);
        }
        if is_swing_low {
            swing_lows.push(i);
        }
    }
    (swing_highs, swing_lows)
}

/// 聚类接近的价格水平（容差 = price * 0.5%）。
pub(super) fn cluster_levels(levels: &[(usize, f64)], price_scale: f64) -> Vec<(f64, i64)> {
    if levels.is_empty() {
        return vec![];
    }
    let tolerance = price_scale * 0.005;
    let mut sorted: Vec<_> = levels.to_vec();
    sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut clusters: Vec<(f64, i64)> = Vec::new();
    let mut current_sum = sorted[0].1;
    let mut current_count = 1_i64;

    for window in sorted.windows(2) {
        let (_, curr) = window[0];
        let (_, next) = window[1];
        if (next - curr).abs() <= tolerance {
            current_sum += next;
            current_count += 1;
        } else {
            clusters.push((current_sum / current_count as f64, current_count));
            current_sum = next;
            current_count = 1;
        }
    }
    clusters.push((current_sum / current_count as f64, current_count));
    clusters.sort_by_key(|cluster| std::cmp::Reverse(cluster.1));
    clusters
}

/// 计算斐波那契回撤和扩展水平。
pub(super) fn fibonacci_levels(swing_low: f64, swing_high: f64, is_uptrend: bool) -> Value {
    let diff = (swing_high - swing_low).abs();
    if diff <= 0.0 {
        return json!({});
    }
    let retracements = [0.0, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0];
    let extensions = [1.0, 1.272, 1.414, 1.618, 2.0, 2.618];

    if is_uptrend {
        let retrace: Vec<Value> = retracements
            .iter()
            .map(|r| {
                json!({
                    "ratio": r,
                    "price": round4(swing_high - diff * r),
                    "type": "retracement"
                })
            })
            .collect();
        let extend: Vec<Value> = extensions
            .iter()
            .map(|e| {
                json!({
                    "ratio": e,
                    "price": round4(swing_low + diff * e),
                    "type": "extension"
                })
            })
            .collect();
        json!({"retracements": retrace, "extensions": extend, "swing_low": swing_low, "swing_high": swing_high, "direction": "up"})
    } else {
        let retrace: Vec<Value> = retracements
            .iter()
            .map(|r| {
                json!({
                    "ratio": r,
                    "price": round4(swing_low + diff * r),
                    "type": "retracement"
                })
            })
            .collect();
        let extend: Vec<Value> = extensions
            .iter()
            .map(|e| {
                json!({
                    "ratio": e,
                    "price": round4(swing_high - diff * e),
                    "type": "extension"
                })
            })
            .collect();
        json!({"retracements": retrace, "extensions": extend, "swing_low": swing_low, "swing_high": swing_high, "direction": "down"})
    }
}

pub(super) fn atr(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> f64 {
    if highs.len() < period + 1 {
        return 0.0;
    }
    let mut tr_sum = 0.0;
    for i in highs.len() - period..highs.len() {
        let h = highs[i];
        let l = lows[i];
        let prev_close = if i > 0 { closes[i - 1] } else { closes[i] };
        let tr = (h - l)
            .max((h - prev_close).abs())
            .max((l - prev_close).abs());
        tr_sum += tr;
    }
    tr_sum / period as f64
}

pub(super) fn linear_regression_slope(values: &[f64]) -> f64 {
    let n = values.len() as f64;
    let sum_x: f64 = (0..values.len()).map(|i| i as f64).sum();
    let sum_y: f64 = values.iter().sum();
    let sum_xy: f64 = values.iter().enumerate().map(|(i, y)| i as f64 * y).sum();
    let sum_x2: f64 = (0..values.len()).map(|i| (i as f64).powi(2)).sum();
    let denominator = n * sum_x2 - sum_x.powi(2);
    if denominator != 0.0 {
        (n * sum_xy - sum_x * sum_y) / denominator
    } else {
        0.0
    }
}

pub(super) fn high_volume_nodes(candles: &[OkxCandle]) -> Vec<Value> {
    let mut vol_prices: Vec<(f64, f64)> = Vec::new();
    for candle in candles {
        let mid = (candle.high + candle.low) / 2.0;
        if candle.high > 0.0 && candle.low > 0.0 && candle.volume > 0.0 {
            vol_prices.push((mid, candle.volume));
        }
    }
    vol_prices.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    vol_prices
        .iter()
        .take(3)
        .map(|(price, vol)| {
            json!({"price": round4(*price), "volume": round2(*vol), "type": "volume_node"})
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{swing_structure_from_points, SwingStructureDirection};

    #[test]
    fn swing_structure_maps_higher_highs_and_lows_to_shared_bullish_labels() {
        let highs = [100.0, 105.0, 112.0];
        let lows = [90.0, 92.0, 96.0];
        let structure = swing_structure_from_points(&highs, &lows, &[0, 2], &[0, 2]);

        assert_eq!(structure.direction, SwingStructureDirection::Bullish);
        assert_eq!(structure.direction.market_label(), "bullish");
        assert_eq!(structure.direction.trade_bias_label(), "long");
        assert!(structure.strength_pct > 0.0);
    }

    #[test]
    fn swing_structure_maps_lower_highs_and_lows_to_shared_bearish_labels() {
        let highs = [112.0, 105.0, 100.0];
        let lows = [96.0, 92.0, 90.0];
        let structure = swing_structure_from_points(&highs, &lows, &[0, 2], &[0, 2]);

        assert_eq!(structure.direction, SwingStructureDirection::Bearish);
        assert_eq!(structure.direction.market_label(), "bearish");
        assert_eq!(structure.direction.trade_bias_label(), "short");
        assert!(structure.strength_pct > 0.0);
    }

    #[test]
    fn swing_structure_keeps_mixed_or_missing_swings_consolidated() {
        let highs = [100.0, 105.0, 112.0];
        let lows = [96.0, 92.0, 90.0];
        let mixed = swing_structure_from_points(&highs, &lows, &[0, 2], &[0, 2]);
        let missing = swing_structure_from_points(&highs, &lows, &[2], &[2]);

        assert_eq!(mixed.direction, SwingStructureDirection::Consolidation);
        assert_eq!(mixed.direction.market_label(), "consolidation");
        assert_eq!(mixed.direction.trade_bias_label(), "neutral");
        assert_eq!(mixed.strength_pct, 0.0);
        assert_eq!(missing.direction, SwingStructureDirection::Consolidation);
    }
}
