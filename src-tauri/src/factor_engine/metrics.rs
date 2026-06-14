use super::bars::FactorBar;

const FACTOR_ORDER: [&str; 10] = [
    "roc_5",
    "roc_10",
    "roc_20",
    "momentum_score",
    "realized_vol_20",
    "high_low_ratio",
    "ma_trend_strength",
    "price_vs_ma20",
    "volume_ratio",
    "volume_trend",
];

pub(super) fn calculate_factor_values(bars: &[FactorBar]) -> Vec<(&'static str, f64)> {
    let closes = bars.iter().map(|bar| bar.3).collect::<Vec<_>>();
    let volumes = bars.iter().map(|bar| bar.4).collect::<Vec<_>>();
    let mut values = std::collections::BTreeMap::new();

    values.extend(momentum_factors(&closes));
    values.extend(volatility_factors(bars, &closes));
    values.extend(trend_factors(&closes));
    values.extend(volume_factors(&volumes));

    FACTOR_ORDER
        .iter()
        .filter_map(|name| values.remove(name).map(|value| (*name, value)))
        .collect()
}

fn momentum_factors(closes: &[f64]) -> Vec<(&'static str, f64)> {
    let roc_5 = rate_of_change(closes, 5);
    let roc_10 = rate_of_change(closes, 10);
    let roc_20 = rate_of_change(closes, 20);
    let momentum_score = 0.5 * roc_5 + 0.3 * roc_10 + 0.2 * roc_20;
    vec![
        ("roc_5", roc_5),
        ("roc_10", roc_10),
        ("roc_20", roc_20),
        ("momentum_score", momentum_score),
    ]
}

fn volatility_factors(bars: &[FactorBar], closes: &[f64]) -> Vec<(&'static str, f64)> {
    let returns = closes
        .windows(2)
        .map(|window| (window[1] / window[0]).ln())
        .collect::<Vec<_>>();
    let realized_vol_20 = if returns.is_empty() {
        0.0
    } else {
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance =
            returns.iter().map(|r| (r - mean) * (r - mean)).sum::<f64>() / returns.len() as f64;
        variance.sqrt() * (365.0 * 24.0 * 3600.0_f64).sqrt()
    };

    let high_low_ratio =
        bars.iter().map(|bar| (bar.1 - bar.2) / bar.3).sum::<f64>() / bars.len().max(1) as f64;
    vec![
        ("realized_vol_20", realized_vol_20),
        ("high_low_ratio", high_low_ratio),
    ]
}

fn trend_factors(closes: &[f64]) -> Vec<(&'static str, f64)> {
    let ma_5 = sma(closes, 5);
    let ma_20 = sma(closes, 20);
    let last = closes.last().copied().unwrap_or(0.0);
    let ma_trend_strength = if ma_20 > 0.0 { ma_5 / ma_20 - 1.0 } else { 0.0 };
    let price_vs_ma20 = if ma_20 > 0.0 { last / ma_20 - 1.0 } else { 0.0 };
    vec![
        ("ma_trend_strength", ma_trend_strength),
        ("price_vs_ma20", price_vs_ma20),
    ]
}

fn volume_factors(volumes: &[f64]) -> Vec<(&'static str, f64)> {
    let n = volumes.len();
    let avg_5 = if n >= 5 {
        volumes[n - 5..].iter().sum::<f64>() / 5.0
    } else {
        0.0
    };
    let avg_20 = volumes.iter().sum::<f64>() / n.clamp(1, 20) as f64;
    let volume_ratio = if avg_20 > 0.0 { avg_5 / avg_20 } else { 1.0 };

    let avg_5_10 = if n >= 10 {
        volumes[n - 10..n - 5].iter().sum::<f64>() / 5.0
    } else {
        avg_5
    };
    let volume_trend = if avg_5_10 > 0.0 {
        avg_5 / avg_5_10 - 1.0
    } else {
        0.0
    };
    vec![
        ("volume_ratio", volume_ratio),
        ("volume_trend", volume_trend),
    ]
}

fn rate_of_change(closes: &[f64], period: usize) -> f64 {
    if closes.len() > period && closes[closes.len() - 1 - period] != 0.0 {
        (closes[closes.len() - 1] - closes[closes.len() - 1 - period])
            / closes[closes.len() - 1 - period]
    } else {
        0.0
    }
}

fn sma(data: &[f64], period: usize) -> f64 {
    if data.len() < period {
        return 0.0;
    }
    data[data.len() - period..].iter().sum::<f64>() / period as f64
}
