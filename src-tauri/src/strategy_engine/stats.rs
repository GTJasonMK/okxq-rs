pub(super) fn sharpe(returns: &[f64]) -> f64 {
    let mean = average(returns.iter().copied());
    let std = stddev(returns.iter().copied(), mean);
    if std > 0.0 {
        mean / std * 252.0_f64.sqrt()
    } else {
        0.0
    }
}

pub(super) fn sortino(returns: &[f64]) -> f64 {
    let mean = average(returns.iter().copied());
    let downside = returns
        .iter()
        .copied()
        .filter(|value| *value < 0.0)
        .collect::<Vec<_>>();
    let downside_std = stddev(downside.iter().copied(), 0.0);
    if downside_std > 0.0 {
        mean / downside_std * 252.0_f64.sqrt()
    } else {
        0.0
    }
}

pub(super) fn average<I>(items: I) -> f64
where
    I: Iterator<Item = f64>,
{
    let mut count = 0.0;
    let mut total = 0.0;
    for item in items {
        if item.is_finite() {
            count += 1.0;
            total += item;
        }
    }
    if count > 0.0 {
        total / count
    } else {
        0.0
    }
}

fn stddev<I>(items: I, mean: f64) -> f64
where
    I: Iterator<Item = f64>,
{
    let values = items.filter(|item| item.is_finite()).collect::<Vec<_>>();
    if values.len() < 2 {
        return 0.0;
    }
    let variance =
        values.iter().map(|item| (item - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}
