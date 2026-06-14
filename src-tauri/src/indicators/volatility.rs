pub(super) fn bollinger_bands(
    closes: &[f64],
    period: usize,
    multiplier: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = closes.len();
    let mut upper = vec![f64::NAN; n];
    let mut middle = vec![f64::NAN; n];
    let mut lower = vec![f64::NAN; n];
    if n < period || period == 0 {
        return (upper, middle, lower);
    }
    let mut window_sum = 0.0;
    let mut window_sq_sum = 0.0;
    for i in 0..n {
        window_sum += closes[i];
        window_sq_sum += closes[i] * closes[i];
        if i >= period {
            let dropped = closes[i - period];
            window_sum -= dropped;
            window_sq_sum -= dropped * dropped;
        }
        if i >= period - 1 {
            let mean = window_sum / period as f64;
            let variance = (window_sq_sum / period as f64 - mean * mean).max(0.0);
            let std = variance.sqrt();
            middle[i] = mean;
            upper[i] = mean + multiplier * std;
            lower[i] = mean - multiplier * std;
        }
    }
    (upper, middle, lower)
}

pub(super) fn atr(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    let n = closes.len();
    let mut result = vec![f64::NAN; n];
    if n < 2 || period == 0 {
        return result;
    }
    let mut tr = vec![0.0; n];
    tr[0] = highs[0] - lows[0];
    for i in 1..n {
        tr[i] = (highs[i] - lows[i])
            .max((highs[i] - closes[i - 1]).abs())
            .max((lows[i] - closes[i - 1]).abs());
    }
    if n < period {
        return result;
    }
    let mut atr_val = tr[..period].iter().sum::<f64>() / period as f64;
    result[period - 1] = atr_val;
    for i in period..n {
        atr_val = (atr_val * (period - 1) as f64 + tr[i]) / period as f64;
        result[i] = atr_val;
    }
    result
}
