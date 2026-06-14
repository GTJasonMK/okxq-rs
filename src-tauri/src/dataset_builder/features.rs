use super::{DatasetBuildConfig, RawBar};

pub(crate) const FEATURE_NAMES: &[&str] = &[
    "open",
    "high",
    "low",
    "close",
    "volume",
    "mid_price",
    "ret_1",
    "ret_5",
    "ret_20",
    "ma5_dev",
    "ma20_dev",
    "vol_5",
    "vol_20",
    "vol_ratio",
    "high_low_range",
];

pub(crate) const LABEL_NAMES: &[&str] = &["label_1m", "label_5m", "label_15m", "label_direction"];

/// 单行特征向量。
#[derive(Clone, Debug)]
pub(crate) struct FeatureRow {
    pub(crate) inst_id: String,
    pub(crate) ts: i64,
    /// 原始 OHLCV
    pub(crate) open: f64,
    pub(crate) high: f64,
    pub(crate) low: f64,
    pub(crate) close: f64,
    pub(crate) volume: f64,
    pub(crate) mid_price: f64,
    /// 滚动特征（基于最近 N 根bar计算）
    pub(crate) ret_1: f64,
    pub(crate) ret_5: f64,
    pub(crate) ret_20: f64,
    pub(crate) ma5_dev: f64,
    pub(crate) ma20_dev: f64,
    pub(crate) vol_5: f64,
    pub(crate) vol_20: f64,
    pub(crate) vol_ratio: f64,
    pub(crate) high_low_range: f64,
    /// 标签（前向收益）
    pub(crate) label_1m: Option<f64>,
    pub(crate) label_5m: Option<f64>,
    pub(crate) label_15m: Option<f64>,
    pub(crate) label_direction: Option<i64>,
}

/// 从原始序列逐行计算滚动特征。
pub(crate) fn build_features(raw: &[RawBar], config: &DatasetBuildConfig) -> Vec<FeatureRow> {
    let n = raw.len();
    let mut rows = Vec::with_capacity(n);

    for i in 0..n {
        let (ts, open, high, low, close, volume, mid_price) = raw[i];

        let w5 = config.window_5;
        let w20 = config.window_20;
        // 收益率（对数）
        let ret_1 = if i >= 1 {
            (close / raw[i - 1].3).ln()
        } else {
            0.0
        };
        let ret_5 = if i >= w5 {
            (close / raw[i - w5].3).ln()
        } else {
            0.0
        };
        let ret_20 = if i >= w20 {
            (close / raw[i - w20].3).ln()
        } else {
            0.0
        };

        // 均线偏离
        let ma5 = if i >= w5 - 1 {
            raw[i + 1 - w5..=i].iter().map(|b| b.3).sum::<f64>() / w5 as f64
        } else {
            close
        };
        let ma20 = if i >= w20 - 1 {
            raw[i + 1 - w20..=i].iter().map(|b| b.3).sum::<f64>() / w20 as f64
        } else {
            close
        };
        let ma5_dev = if ma5 > 0.0 { close / ma5 - 1.0 } else { 0.0 };
        let ma20_dev = if ma20 > 0.0 { close / ma20 - 1.0 } else { 0.0 };

        // 波动率
        let vol_5 = if i >= w5 {
            let slice = &raw[i + 1 - w5..=i];
            let mean = slice.iter().map(|b| b.3).sum::<f64>() / w5 as f64;
            let var = slice
                .iter()
                .map(|b| (b.3 - mean) * (b.3 - mean))
                .sum::<f64>()
                / w5 as f64;
            var.sqrt() / mean
        } else {
            0.0
        };
        let vol_20 = if i >= w20 {
            let slice = &raw[i + 1 - w20..=i];
            let mean = slice.iter().map(|b| b.3).sum::<f64>() / w20 as f64;
            let var = slice
                .iter()
                .map(|b| (b.3 - mean) * (b.3 - mean))
                .sum::<f64>()
                / w20 as f64;
            var.sqrt() / mean
        } else {
            0.0
        };

        // 量比
        let avg_vol_5 = if i >= w5 - 1 {
            raw[i + 1 - w5..=i].iter().map(|b| b.4).sum::<f64>() / w5 as f64
        } else {
            volume
        };
        let avg_vol_20 = if i >= w20 - 1 {
            raw[i + 1 - w20..=i].iter().map(|b| b.4).sum::<f64>() / w20 as f64
        } else {
            volume
        };
        let vol_ratio = if avg_vol_20 > 0.0 {
            avg_vol_5 / avg_vol_20
        } else {
            1.0
        };

        // 高低价差比
        let high_low_range = if close > 0.0 {
            (high - low) / close
        } else {
            0.0
        };

        rows.push(FeatureRow {
            inst_id: config.inst_id.clone(),
            ts,
            open,
            high,
            low,
            close,
            volume,
            mid_price,
            ret_1,
            ret_5,
            ret_20,
            ma5_dev,
            ma20_dev,
            vol_5,
            vol_20,
            vol_ratio,
            high_low_range,
            label_1m: None,
            label_5m: None,
            label_15m: None,
            label_direction: None,
        });
    }

    rows
}

/// 为每行生成前向收益标签。
pub(crate) fn build_labels(mut rows: Vec<FeatureRow>) -> Vec<FeatureRow> {
    let n = rows.len();
    let w1m = 60; // 1分钟 = 60根秒级bar
    let w5m = 300;
    let w15m = 900;

    for i in 0..n {
        let current_close = rows[i].close;

        let fwd_idx_1m = i + w1m;
        let fwd_idx_5m = i + w5m;
        let fwd_idx_15m = i + w15m;

        rows[i].label_1m = if fwd_idx_1m < n {
            Some((rows[fwd_idx_1m].close / current_close).ln())
        } else {
            None
        };
        rows[i].label_5m = if fwd_idx_5m < n {
            Some((rows[fwd_idx_5m].close / current_close).ln())
        } else {
            None
        };
        rows[i].label_15m = if fwd_idx_15m < n {
            Some((rows[fwd_idx_15m].close / current_close).ln())
        } else {
            None
        };
        rows[i].label_direction = rows[i].label_1m.map(|v| if v > 0.0 { 1 } else { 0 });
    }

    rows
}
