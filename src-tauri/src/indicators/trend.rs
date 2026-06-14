use super::averages::ema;

pub(super) fn macd(
    closes: &[f64],
    fast: usize,
    slow: usize,
    signal: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = closes.len();
    let ema_fast = ema(closes, fast);
    let ema_slow = ema(closes, slow);

    let mut dif = vec![f64::NAN; n];
    for i in 0..n {
        if ema_fast[i].is_finite() && ema_slow[i].is_finite() {
            dif[i] = ema_fast[i] - ema_slow[i];
        }
    }

    let valid_start = dif.iter().position(|v| v.is_finite()).unwrap_or(n);
    let valid_dif: Vec<f64> = dif[valid_start..]
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .collect();
    let valid_dea = ema(&valid_dif, signal);
    let mut dea = vec![f64::NAN; n];
    for (offset, val) in valid_dea.iter().enumerate() {
        dea[valid_start + offset] = *val;
    }

    let mut hist = vec![f64::NAN; n];
    for i in 0..n {
        if dif[i].is_finite() && dea[i].is_finite() {
            hist[i] = (dif[i] - dea[i]) * 2.0;
        }
    }
    (dif, dea, hist)
}
