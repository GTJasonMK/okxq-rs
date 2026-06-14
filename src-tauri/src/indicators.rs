// 完整技术指标计算模块
// 提供 SMA/EMA/MACD/RSI/Bollinger/KDJ/ATR/Volume MA 等常用指标

use serde_json::{Map, Value};

use crate::okx::OkxCandle;

mod averages;
mod format;
mod momentum;
mod trend;
mod volatility;

use self::{
    averages::{ema, sma},
    format::vec_to_values,
    momentum::{kdj, rsi},
    trend::macd,
    volatility::{atr, bollinger_bands},
};

/// 指标计算器：从 K 线数据批量产出所有常用指标序列
pub struct IndicatorCalculator {
    closes: Vec<f64>,
    highs: Vec<f64>,
    lows: Vec<f64>,
    volumes: Vec<f64>,
}

impl IndicatorCalculator {
    pub fn new(candles: &[OkxCandle]) -> Self {
        let closes = candles.iter().map(|item| item.close).collect::<Vec<_>>();
        let highs = candles.iter().map(|item| item.high).collect::<Vec<_>>();
        let lows = candles.iter().map(|item| item.low).collect::<Vec<_>>();
        let volumes = candles.iter().map(|item| item.volume).collect::<Vec<_>>();
        Self {
            closes,
            highs,
            lows,
            volumes,
        }
    }

    /// 计算所有常用指标，返回 JSON
    pub fn calculate_all(&self) -> Value {
        let mut map = Map::new();
        map.insert("ma5".to_string(), vec_to_values(&sma(&self.closes, 5)));
        map.insert("ma10".to_string(), vec_to_values(&sma(&self.closes, 10)));
        map.insert("ma20".to_string(), vec_to_values(&sma(&self.closes, 20)));
        map.insert("ma60".to_string(), vec_to_values(&sma(&self.closes, 60)));
        map.insert("ema12".to_string(), vec_to_values(&ema(&self.closes, 12)));
        map.insert("ema26".to_string(), vec_to_values(&ema(&self.closes, 26)));

        let (dif, dea, hist) = macd(&self.closes, 12, 26, 9);
        map.insert("macd_dif".to_string(), vec_to_values(&dif));
        map.insert("macd_dea".to_string(), vec_to_values(&dea));
        map.insert("macd_hist".to_string(), vec_to_values(&hist));

        map.insert("rsi".to_string(), vec_to_values(&rsi(&self.closes, 14)));

        let (upper, middle, lower) = bollinger_bands(&self.closes, 20, 2.0);
        map.insert("bollinger_upper".to_string(), vec_to_values(&upper));
        map.insert("bollinger_middle".to_string(), vec_to_values(&middle));
        map.insert("bollinger_lower".to_string(), vec_to_values(&lower));

        let (k, d, j) = kdj(&self.highs, &self.lows, &self.closes, 9, 3, 3);
        map.insert("kdj_k".to_string(), vec_to_values(&k));
        map.insert("kdj_d".to_string(), vec_to_values(&d));
        map.insert("kdj_j".to_string(), vec_to_values(&j));

        map.insert(
            "atr".to_string(),
            vec_to_values(&atr(&self.highs, &self.lows, &self.closes, 14)),
        );
        map.insert(
            "volume_ma".to_string(),
            vec_to_values(&sma(&self.volumes, 20)),
        );

        Value::Object(map)
    }
}
