pub(crate) fn simple_ma(values: &[f64], period: usize) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let start = values.len().saturating_sub(period.max(1));
    let window = &values[start..];
    window.iter().sum::<f64>() / window.len() as f64
}

pub(crate) fn simple_rsi(closes: &[f64], period: usize) -> f64 {
    if closes.len() <= period || period == 0 {
        return 50.0;
    }
    let start = closes.len() - period;
    let mut gains = 0.0;
    let mut losses = 0.0;
    for idx in (start + 1)..closes.len() {
        let diff = closes[idx] - closes[idx - 1];
        if diff >= 0.0 {
            gains += diff;
        } else {
            losses += -diff;
        }
    }
    if losses == 0.0 {
        return 100.0;
    }
    let rs = gains / losses;
    100.0 - (100.0 / (1.0 + rs))
}

pub(crate) fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub(crate) fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}
