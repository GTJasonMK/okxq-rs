pub(super) fn sma(prices: &[f64], period: usize) -> Vec<f64> {
    let n = prices.len();
    let mut result = vec![f64::NAN; n];
    if n < period || period == 0 {
        return result;
    }
    let mut window_sum = prices[..period].iter().sum::<f64>();
    result[period - 1] = window_sum / period as f64;
    for i in period..n {
        window_sum += prices[i] - prices[i - period];
        result[i] = window_sum / period as f64;
    }
    result
}

pub(super) fn ema(prices: &[f64], period: usize) -> Vec<f64> {
    let n = prices.len();
    let mut result = vec![f64::NAN; n];
    if n < period || period == 0 {
        return result;
    }
    let mut ema_val = prices[..period].iter().sum::<f64>() / period as f64;
    let multiplier = 2.0 / (period as f64 + 1.0);
    result[period - 1] = ema_val;
    for i in period..n {
        ema_val = (prices[i] - ema_val) * multiplier + ema_val;
        result[i] = ema_val;
    }
    result
}
