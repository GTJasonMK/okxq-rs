pub(super) fn rsi(closes: &[f64], period: usize) -> Vec<f64> {
    let n = closes.len();
    let mut result = vec![f64::NAN; n];
    if n < period + 1 || period == 0 {
        return result;
    }
    let changes: Vec<f64> = closes.windows(2).map(|w| w[1] - w[0]).collect();

    let mut avg_gain = changes[..period].iter().map(|c| c.max(0.0)).sum::<f64>() / period as f64;
    let mut avg_loss = changes[..period].iter().map(|c| (-c).max(0.0)).sum::<f64>() / period as f64;

    result[period] = if avg_loss == 0.0 {
        100.0
    } else {
        100.0 - 100.0 / (1.0 + avg_gain / avg_loss)
    };

    for i in period..changes.len() {
        avg_gain = (avg_gain * (period - 1) as f64 + changes[i].max(0.0)) / period as f64;
        avg_loss = (avg_loss * (period - 1) as f64 + (-changes[i]).max(0.0)) / period as f64;
        result[i + 1] = if avg_loss == 0.0 {
            100.0
        } else {
            100.0 - 100.0 / (1.0 + avg_gain / avg_loss)
        };
    }
    result
}

pub(super) fn kdj(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    n: usize,
    m1: usize,
    m2: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let len = closes.len();
    let nan_full = || vec![f64::NAN; len];
    if len < n || n == 0 {
        return (nan_full(), nan_full(), nan_full());
    }

    let mut rsv = vec![f64::NAN; len];
    for i in n - 1..len {
        let window_high = highs[i + 1 - n..=i]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let window_low = lows[i + 1 - n..=i]
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        rsv[i] = if (window_high - window_low).abs() < 1e-12 {
            50.0
        } else {
            (closes[i] - window_low) / (window_high - window_low) * 100.0
        };
    }

    let mut k_vals = vec![f64::NAN; len];
    let mut d_vals = vec![f64::NAN; len];
    let mut k_prev = 50.0;
    let mut d_prev = 50.0;

    let m1_f = m1 as f64;
    let m2_f = m2 as f64;
    for i in n - 1..len {
        if rsv[i].is_finite() {
            k_prev = (m1_f - 1.0) / m1_f * k_prev + 1.0 / m1_f * rsv[i];
            k_vals[i] = k_prev;
            d_prev = (m2_f - 1.0) / m2_f * d_prev + 1.0 / m2_f * k_prev;
            d_vals[i] = d_prev;
        }
    }

    let mut j_vals = vec![f64::NAN; len];
    for i in 0..len {
        if k_vals[i].is_finite() && d_vals[i].is_finite() {
            j_vals[i] = 3.0 * k_vals[i] - 2.0 * d_vals[i];
        }
    }
    (k_vals, d_vals, j_vals)
}
