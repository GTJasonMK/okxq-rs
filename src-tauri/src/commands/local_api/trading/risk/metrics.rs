use super::super::*;

pub(super) fn equities_from_snapshots(snapshots: &[Value]) -> Vec<f64> {
    snapshots
        .iter()
        .filter_map(|item| item.get("total_equity").and_then(Value::as_f64))
        .filter(|value| *value > 0.0)
        .collect()
}

pub(super) fn returns_from_equities(equities: &[f64]) -> Vec<f64> {
    equities
        .windows(2)
        .filter_map(|pair| {
            if pair[0] > 0.0 {
                Some((pair[1] - pair[0]) / pair[0])
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn historical_var(returns: &[f64], confidence: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    let mut sorted = returns.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = (((1.0 - confidence) * sorted.len() as f64).floor() as usize).min(sorted.len() - 1);
    round4(-sorted[idx])
}

pub(super) fn parametric_var(returns: &[f64], confidence: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    let z = if confidence >= 0.99 {
        2.326
    } else if confidence >= 0.975 {
        1.96
    } else {
        1.645
    };
    round4(-(mean(returns) - z * std_dev(returns)))
}

pub(super) fn sharpe_ratio(returns: &[f64]) -> f64 {
    let sd = std_dev(returns);
    if sd == 0.0 {
        0.0
    } else {
        mean(returns) / sd * 252.0_f64.sqrt()
    }
}

pub(super) fn sortino_ratio(returns: &[f64]) -> f64 {
    let downside = returns
        .iter()
        .copied()
        .filter(|value| *value < 0.0)
        .collect::<Vec<_>>();
    let sd = std_dev(&downside);
    if sd == 0.0 {
        0.0
    } else {
        mean(returns) / sd * 252.0_f64.sqrt()
    }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

pub(super) fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let avg = mean(values);
    let variance = values
        .iter()
        .map(|value| (value - avg).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    variance.sqrt()
}

pub(super) struct DrawdownInfo {
    pub(super) max_drawdown: f64,
    pub(super) current_drawdown: f64,
    pub(super) max_drawdown_duration: i64,
    pub(super) peak: f64,
    pub(super) series: Vec<f64>,
}

pub(super) fn max_drawdown(equities: &[f64]) -> DrawdownInfo {
    let mut peak = 0.0;
    let mut max_dd = 0.0;
    let mut current_duration = 0;
    let mut max_duration = 0;
    let mut series = Vec::new();
    for value in equities {
        if *value > peak {
            peak = *value;
            current_duration = 0;
        }
        let dd = if peak > 0.0 {
            (value - peak) / peak
        } else {
            0.0
        };
        if dd < 0.0 {
            current_duration += 1;
        }
        if current_duration > max_duration {
            max_duration = current_duration;
        }
        if dd < max_dd {
            max_dd = dd;
        }
        series.push(round4(dd));
    }
    DrawdownInfo {
        max_drawdown: round4(max_dd),
        current_drawdown: round4(*series.last().unwrap_or(&0.0)),
        max_drawdown_duration: max_duration,
        peak,
        series,
    }
}
