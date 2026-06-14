use super::rng::XorShift64;
use super::types::{MonteCarloConfig, MonteCarloResult};

/// 运行蒙特卡洛模拟：对交易盈亏序列进行块自助重采样，生成模拟权益曲线。
pub fn run_monte_carlo(
    trade_pnls: &[f64],
    initial_capital: f64,
    config: &MonteCarloConfig,
) -> MonteCarloResult {
    let n_trades = trade_pnls.len();
    let initial_capital = initial_capital.max(1.0);
    let block_size = effective_block_size(config.block_size, n_trades);
    let sampling_method = if block_size <= 1 {
        "iid_trade_bootstrap"
    } else {
        "circular_block_bootstrap"
    }
    .to_string();

    // 计算原始回测的最终权益和最大回撤
    let (original_final_equity, original_max_drawdown) = {
        let mut equity = initial_capital;
        let mut peak = equity;
        let mut max_dd = 0.0_f64;
        for pnl in trade_pnls {
            equity += pnl;
            peak = peak.max(equity);
            let dd = (peak - equity) / peak * 100.0;
            max_dd = max_dd.max(dd);
        }
        (equity, max_dd)
    };

    let mut rng = XorShift64::new(config.seed.unwrap_or(42));
    let mut final_equities = Vec::with_capacity(config.num_simulations);
    let mut max_drawdowns = Vec::with_capacity(config.num_simulations);

    if n_trades == 0 {
        return MonteCarloResult {
            num_simulations: config.num_simulations,
            sampling_method,
            block_size,
            original_final_equity,
            original_max_drawdown,
            equity_percentiles: vec![],
            drawdown_percentiles: vec![],
            mean_final_equity: initial_capital,
            std_final_equity: 0.0,
            median_final_equity: initial_capital,
            prob_profit: 0.0,
            prob_original_beat: 0.0,
            worst_case_equity: initial_capital,
            best_case_equity: initial_capital,
        };
    }

    for _ in 0..config.num_simulations {
        let mut equity = initial_capital;
        let mut peak = equity;
        let mut max_dd = 0.0_f64;

        let mut sampled = 0usize;
        while sampled < n_trades {
            let start = (rng.next() as usize) % n_trades;
            for offset in 0..block_size {
                if sampled >= n_trades {
                    break;
                }
                let idx = (start + offset) % n_trades;
                equity += trade_pnls[idx];
                sampled += 1;
                peak = peak.max(equity);
                let dd = (peak - equity) / peak * 100.0;
                max_dd = max_dd.max(dd);
            }
        }

        final_equities.push(equity);
        max_drawdowns.push(max_dd);
    }

    final_equities.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    max_drawdowns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mean_final_equity = final_equities.iter().sum::<f64>() / config.num_simulations as f64;
    let std_final_equity = {
        let variance = final_equities
            .iter()
            .map(|v| (v - mean_final_equity).powi(2))
            .sum::<f64>()
            / config.num_simulations as f64;
        variance.sqrt()
    };
    let median_idx = config.num_simulations / 2;
    let median_final_equity = final_equities[median_idx];

    let prob_profit = final_equities
        .iter()
        .filter(|v| **v > initial_capital)
        .count() as f64
        / config.num_simulations as f64
        * 100.0;
    let prob_original_beat = final_equities
        .iter()
        .filter(|v| **v >= original_final_equity)
        .count() as f64
        / config.num_simulations as f64
        * 100.0;

    let worst_case_equity = *final_equities.first().unwrap_or(&initial_capital);
    let best_case_equity = *final_equities.last().unwrap_or(&initial_capital);

    let equity_percentiles = config
        .confidence_levels
        .iter()
        .map(|&level| {
            let idx =
                ((level * config.num_simulations as f64) as usize).min(config.num_simulations - 1);
            let label = format!("{}%", (level * 100.0).round());
            (label, final_equities[idx])
        })
        .collect();

    let drawdown_percentiles = config
        .confidence_levels
        .iter()
        .map(|&level| {
            let idx =
                ((level * config.num_simulations as f64) as usize).min(config.num_simulations - 1);
            let label = format!("{}%", (level * 100.0).round());
            (label, max_drawdowns[idx])
        })
        .collect();

    MonteCarloResult {
        num_simulations: config.num_simulations,
        sampling_method,
        block_size,
        original_final_equity,
        original_max_drawdown,
        equity_percentiles,
        drawdown_percentiles,
        mean_final_equity,
        std_final_equity,
        median_final_equity,
        prob_profit,
        prob_original_beat,
        worst_case_equity,
        best_case_equity,
    }
}

fn effective_block_size(block_size: usize, n_trades: usize) -> usize {
    if n_trades == 0 {
        return block_size.max(1);
    }
    block_size.max(1).min(n_trades)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monte_carlo_basic() {
        let pnls = vec![10.0, -5.0, 15.0, -3.0, 20.0, -8.0, 12.0, -4.0, 18.0, -6.0];
        let result = run_monte_carlo(&pnls, 10000.0, &MonteCarloConfig::default());
        assert_eq!(result.num_simulations, 1000);
        assert_eq!(result.sampling_method, "circular_block_bootstrap");
        assert_eq!(result.block_size, 5);
        assert!(result.prob_profit >= 0.0);
        assert!(result.best_case_equity > result.worst_case_equity);
    }

    #[test]
    fn test_monte_carlo_empty_pnls() {
        let result = run_monte_carlo(&[], 10000.0, &MonteCarloConfig::default());
        assert_eq!(result.original_final_equity, 10000.0);
    }

    #[test]
    fn test_monte_carlo_block_size_one_keeps_iid_mode() {
        let pnls = vec![10.0, -5.0, 15.0, -3.0];
        let result = run_monte_carlo(
            &pnls,
            10000.0,
            &MonteCarloConfig {
                num_simulations: 50,
                block_size: 1,
                seed: Some(7),
                ..Default::default()
            },
        );
        assert_eq!(result.sampling_method, "iid_trade_bootstrap");
        assert_eq!(result.block_size, 1);
    }

    #[test]
    fn test_monte_carlo_full_block_preserves_total_pnl() {
        let pnls = vec![10.0, -5.0, 15.0, -3.0];
        let result = run_monte_carlo(
            &pnls,
            10000.0,
            &MonteCarloConfig {
                num_simulations: 50,
                block_size: pnls.len(),
                seed: Some(7),
                ..Default::default()
            },
        );
        assert_eq!(result.sampling_method, "circular_block_bootstrap");
        assert_eq!(result.block_size, pnls.len());
        assert_eq!(result.worst_case_equity, result.original_final_equity);
        assert_eq!(result.best_case_equity, result.original_final_equity);
    }
}
