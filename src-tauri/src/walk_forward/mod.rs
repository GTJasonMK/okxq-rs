use serde_json::{json, Value};

const DAY_MS: i64 = 86_400_000;
const DEFAULT_MIN_POINTS: usize = 3;
const DEFAULT_ANNUALIZATION_PERIODS: f64 = 365.0;

#[derive(Debug, Clone)]
pub struct WalkForwardConfig {
    pub window_days: i64,
    pub step_days: i64,
    pub min_points: usize,
    pub benchmark_sharpe: f64,
    pub trial_count: usize,
}

impl Default for WalkForwardConfig {
    fn default() -> Self {
        Self {
            window_days: 30,
            step_days: 30,
            min_points: DEFAULT_MIN_POINTS,
            benchmark_sharpe: 0.0,
            trial_count: 1,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct EquityPoint {
    timestamp: i64,
    equity: f64,
}

#[derive(Debug, Clone, Copy)]
struct TradeCashflow {
    timestamp: i64,
    cashflow: f64,
}

#[derive(Debug, Clone)]
struct FoldMetrics {
    index: usize,
    start_ts: i64,
    end_ts: i64,
    first_ts: i64,
    last_ts: i64,
    points: usize,
    coverage_ratio: f64,
    start_equity: f64,
    end_equity: f64,
    return_pct: f64,
    max_drawdown_pct: f64,
    sharpe: f64,
    sortino: f64,
    trade_count: usize,
    trade_cashflow: f64,
    profit_factor: f64,
}

pub fn analyze_backtest_detail(detail: &Value, config: &WalkForwardConfig) -> Value {
    let mut equity = extract_equity_points(detail);
    normalize_equity_points(&mut equity);
    let trades = extract_trade_cashflows(detail);

    if equity.len() < config.min_points.max(2) {
        return json!({
            "status": "insufficient_data",
            "method": "rolling_window_equity_oos_diagnostic",
            "message": "回测结果缺少足够的连续权益点，无法进行滚动窗口诊断",
            "config": config_json(config),
            "equity_points": equity.len(),
            "folds": []
        });
    }

    let folds = build_folds(&equity, &trades, config);
    if folds.is_empty() {
        return json!({
            "status": "insufficient_data",
            "method": "rolling_window_equity_oos_diagnostic",
            "message": "当前窗口参数下没有足够权益点形成有效折",
            "config": config_json(config),
            "period": period_json(&equity),
            "equity_points": equity.len(),
            "folds": []
        });
    }

    let returns = point_returns(&equity);
    let periods_per_year = periods_per_year(&equity, DEFAULT_ANNUALIZATION_PERIODS);
    let observed_sharpe = annualized_sharpe(&returns, periods_per_year);
    let psr = probabilistic_sharpe_ratio(&returns, periods_per_year, config.benchmark_sharpe);
    let adjusted_probability = multiple_testing_adjusted_probability(psr, config.trial_count);

    json!({
        "status": "ok",
        "method": "rolling_window_equity_oos_diagnostic",
        "method_note": "基于已保存回测权益曲线按时间滚动切分；它暴露时间折稳定性，但不重新训练或重新选择参数。",
        "config": config_json(config),
        "period": period_json(&equity),
        "equity_points": equity.len(),
        "fold_count": folds.len(),
        "summary": summary_json(&folds, adjusted_probability),
        "statistical_validation": {
            "observed_sharpe": round6(observed_sharpe),
            "benchmark_sharpe": round6(config.benchmark_sharpe),
            "probabilistic_sharpe_ratio": psr_json(&psr),
            "multiple_testing_adjustment": {
                "method": "bonferroni_on_psr_p_value",
                "trial_count": config.trial_count,
                "adjusted_probability": round6(adjusted_probability),
                "adjusted_p_value": round6(1.0 - adjusted_probability),
                "note": "trial_count 应填入同一研究问题下实际比较过的候选数；例如模板笛卡尔积搜索应填入模板数。"
            }
        },
        "folds": folds.iter().map(fold_json).collect::<Vec<_>>()
    })
}

fn build_folds(
    equity: &[EquityPoint],
    trades: &[TradeCashflow],
    config: &WalkForwardConfig,
) -> Vec<FoldMetrics> {
    let window_ms = config.window_days.max(1).saturating_mul(DAY_MS);
    let step_ms = config.step_days.max(1).saturating_mul(DAY_MS);
    let min_points = config.min_points.max(2);
    let first_ts = equity.first().map(|point| point.timestamp).unwrap_or(0);
    let last_ts = equity
        .last()
        .map(|point| point.timestamp)
        .unwrap_or(first_ts);
    let mut folds = Vec::new();
    let mut start_ts = first_ts;

    while start_ts < last_ts {
        let end_ts = start_ts.saturating_add(window_ms);
        let points = equity
            .iter()
            .copied()
            .filter(|point| point.timestamp >= start_ts && point.timestamp < end_ts)
            .collect::<Vec<_>>();
        if points.len() >= min_points {
            folds.push(fold_metrics(
                folds.len(),
                start_ts,
                end_ts,
                &points,
                trades,
                window_ms,
            ));
        }
        if end_ts >= last_ts && start_ts.saturating_add(step_ms) > last_ts {
            break;
        }
        start_ts = start_ts.saturating_add(step_ms);
    }

    folds
}

fn fold_metrics(
    index: usize,
    start_ts: i64,
    end_ts: i64,
    points: &[EquityPoint],
    trades: &[TradeCashflow],
    window_ms: i64,
) -> FoldMetrics {
    let first = points.first().expect("fold has points");
    let last = points.last().expect("fold has points");
    let returns = point_returns(points);
    let periods = periods_per_year(points, DEFAULT_ANNUALIZATION_PERIODS);
    let fold_trades = trades
        .iter()
        .filter(|trade| trade.timestamp >= start_ts && trade.timestamp < end_ts)
        .copied()
        .collect::<Vec<_>>();

    FoldMetrics {
        index,
        start_ts,
        end_ts,
        first_ts: first.timestamp,
        last_ts: last.timestamp,
        points: points.len(),
        coverage_ratio: if window_ms > 0 {
            ((last.timestamp - first.timestamp).max(0) as f64 / window_ms as f64).clamp(0.0, 1.0)
        } else {
            0.0
        },
        start_equity: first.equity,
        end_equity: last.equity,
        return_pct: pct_change(first.equity, last.equity),
        max_drawdown_pct: max_drawdown_pct(points),
        sharpe: annualized_sharpe(&returns, periods),
        sortino: annualized_sortino(&returns, periods),
        trade_count: fold_trades.len(),
        trade_cashflow: fold_trades.iter().map(|trade| trade.cashflow).sum(),
        profit_factor: profit_factor(&fold_trades),
    }
}

fn extract_equity_points(detail: &Value) -> Vec<EquityPoint> {
    detail
        .get("equity_curve")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let timestamp = item
                .get("timestamp")
                .or_else(|| item.get("time"))
                .and_then(value_i64)?;
            let equity = item.get("equity").and_then(value_f64)?;
            if timestamp >= 0 && equity.is_finite() && equity > 0.0 {
                Some(EquityPoint { timestamp, equity })
            } else {
                None
            }
        })
        .collect()
}

fn normalize_equity_points(points: &mut Vec<EquityPoint>) {
    points.sort_by_key(|point| point.timestamp);
    let mut compacted: Vec<EquityPoint> = Vec::with_capacity(points.len());
    for point in points.drain(..) {
        if let Some(last) = compacted.last_mut() {
            if last.timestamp == point.timestamp {
                *last = point;
                continue;
            }
        }
        compacted.push(point);
    }
    *points = compacted;
}

fn extract_trade_cashflows(detail: &Value) -> Vec<TradeCashflow> {
    detail
        .get("trade_cashflows")
        .or_else(|| detail.get("trades"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let timestamp = item.get("timestamp").and_then(value_i64)?;
            let cashflow = item.get("cashflow").and_then(value_f64).unwrap_or_else(|| {
                let pnl = item.get("pnl").and_then(value_f64).unwrap_or(0.0);
                let funding = item.get("funding").and_then(value_f64).unwrap_or(0.0);
                pnl + funding
            });
            if timestamp >= 0 && cashflow.is_finite() && cashflow != 0.0 {
                Some(TradeCashflow {
                    timestamp,
                    cashflow,
                })
            } else {
                None
            }
        })
        .collect()
}

fn point_returns(points: &[EquityPoint]) -> Vec<f64> {
    points
        .windows(2)
        .filter_map(|pair| {
            let previous = pair[0].equity;
            let current = pair[1].equity;
            if previous > 0.0 && current.is_finite() {
                Some(current / previous - 1.0)
            } else {
                None
            }
        })
        .collect()
}

fn periods_per_year(points: &[EquityPoint], fallback: f64) -> f64 {
    if points.len() < 2 {
        return fallback;
    }
    let first = points.first().map(|point| point.timestamp).unwrap_or(0);
    let last = points.last().map(|point| point.timestamp).unwrap_or(first);
    let elapsed_days = (last - first).max(1) as f64 / DAY_MS as f64;
    let periods = (points.len() - 1) as f64;
    let value = periods / elapsed_days * 365.0;
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

fn annualized_sharpe(returns: &[f64], periods_per_year: f64) -> f64 {
    let mean = mean(returns);
    let std = sample_stddev(returns, mean);
    if std > 0.0 && periods_per_year > 0.0 {
        mean / std * periods_per_year.sqrt()
    } else {
        0.0
    }
}

fn annualized_sortino(returns: &[f64], periods_per_year: f64) -> f64 {
    let mean = mean(returns);
    let downside = returns
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value < 0.0)
        .collect::<Vec<_>>();
    let downside_std = sample_stddev(&downside, 0.0);
    if downside_std > 0.0 && periods_per_year > 0.0 {
        mean / downside_std * periods_per_year.sqrt()
    } else {
        0.0
    }
}

fn max_drawdown_pct(points: &[EquityPoint]) -> f64 {
    let mut peak = 0.0_f64;
    let mut max_drawdown = 0.0_f64;
    for point in points {
        peak = peak.max(point.equity);
        if peak > 0.0 {
            max_drawdown = max_drawdown.max((peak - point.equity) / peak);
        }
    }
    max_drawdown * 100.0
}

fn profit_factor(trades: &[TradeCashflow]) -> f64 {
    let gross_profit = trades
        .iter()
        .map(|trade| trade.cashflow)
        .filter(|value| *value > 0.0)
        .sum::<f64>();
    let gross_loss = trades
        .iter()
        .map(|trade| trade.cashflow)
        .filter(|value| *value < 0.0)
        .map(f64::abs)
        .sum::<f64>();
    if gross_loss > 0.0 {
        gross_profit / gross_loss
    } else if gross_profit > 0.0 {
        gross_profit
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy)]
struct ProbabilisticSharpe {
    probability: f64,
    p_value: f64,
    z_score: f64,
    sample_size: usize,
    skewness: f64,
    kurtosis: f64,
}

fn probabilistic_sharpe_ratio(
    returns: &[f64],
    periods_per_year: f64,
    benchmark_annual_sharpe: f64,
) -> ProbabilisticSharpe {
    let clean = returns
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    let sample_size = clean.len();
    if sample_size < 2 {
        return ProbabilisticSharpe {
            probability: 0.5,
            p_value: 0.5,
            z_score: 0.0,
            sample_size,
            skewness: 0.0,
            kurtosis: 0.0,
        };
    }

    let mean_value = mean(&clean);
    let std = sample_stddev(&clean, mean_value);
    if std <= 0.0 || periods_per_year <= 0.0 {
        return ProbabilisticSharpe {
            probability: 0.5,
            p_value: 0.5,
            z_score: 0.0,
            sample_size,
            skewness: 0.0,
            kurtosis: 0.0,
        };
    }

    let sharpe_period = mean_value / std;
    let benchmark_period = benchmark_annual_sharpe / periods_per_year.sqrt();
    let skewness = skewness(&clean, mean_value, std);
    let kurtosis = kurtosis(&clean, mean_value, std);
    let denominator =
        (1.0 - skewness * sharpe_period + ((kurtosis - 1.0) / 4.0) * sharpe_period.powi(2)).sqrt();
    let z_score = if denominator.is_finite() && denominator > 0.0 {
        (sharpe_period - benchmark_period) * ((sample_size - 1) as f64).sqrt() / denominator
    } else {
        (sharpe_period - benchmark_period) * ((sample_size - 1) as f64).sqrt()
    };
    let probability = normal_cdf(z_score).clamp(0.0, 1.0);
    ProbabilisticSharpe {
        probability,
        p_value: (1.0 - probability).clamp(0.0, 1.0),
        z_score,
        sample_size,
        skewness,
        kurtosis,
    }
}

fn multiple_testing_adjusted_probability(psr: ProbabilisticSharpe, trial_count: usize) -> f64 {
    let trials = trial_count.max(1) as f64;
    (1.0 - (psr.p_value * trials).min(1.0)).clamp(0.0, 1.0)
}

fn mean(values: &[f64]) -> f64 {
    let mut total = 0.0;
    let mut count = 0.0;
    for value in values.iter().copied().filter(|value| value.is_finite()) {
        total += value;
        count += 1.0;
    }
    if count > 0.0 {
        total / count
    } else {
        0.0
    }
}

fn sample_stddev(values: &[f64], mean_value: f64) -> f64 {
    let clean = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if clean.len() < 2 {
        return 0.0;
    }
    let variance = clean
        .iter()
        .map(|value| (value - mean_value).powi(2))
        .sum::<f64>()
        / (clean.len() - 1) as f64;
    variance.sqrt()
}

fn skewness(values: &[f64], mean_value: f64, std: f64) -> f64 {
    if values.len() < 3 || std <= 0.0 {
        return 0.0;
    }
    let n = values.len() as f64;
    values
        .iter()
        .map(|value| ((value - mean_value) / std).powi(3))
        .sum::<f64>()
        / n
}

fn kurtosis(values: &[f64], mean_value: f64, std: f64) -> f64 {
    if values.len() < 4 || std <= 0.0 {
        return 3.0;
    }
    let n = values.len() as f64;
    values
        .iter()
        .map(|value| ((value - mean_value) / std).powi(4))
        .sum::<f64>()
        / n
}

fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();
    sign * y
}

fn pct_change(start: f64, end: f64) -> f64 {
    if start > 0.0 {
        (end / start - 1.0) * 100.0
    } else {
        0.0
    }
}

fn value_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|item| i64::try_from(item).ok()))
        .or_else(|| value.as_f64().map(|item| item as i64))
}

fn value_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|item| item as f64))
        .or_else(|| value.as_u64().map(|item| item as f64))
        .filter(|item| item.is_finite())
}

fn config_json(config: &WalkForwardConfig) -> Value {
    json!({
        "window_days": config.window_days.max(1),
        "step_days": config.step_days.max(1),
        "min_points": config.min_points.max(2),
        "benchmark_sharpe": round6(config.benchmark_sharpe),
        "trial_count": config.trial_count.max(1)
    })
}

fn period_json(equity: &[EquityPoint]) -> Value {
    json!({
        "start_ts": equity.first().map(|point| point.timestamp),
        "end_ts": equity.last().map(|point| point.timestamp),
    })
}

fn summary_json(folds: &[FoldMetrics], adjusted_probability: f64) -> Value {
    let returns = folds.iter().map(|fold| fold.return_pct).collect::<Vec<_>>();
    let sharpes = folds.iter().map(|fold| fold.sharpe).collect::<Vec<_>>();
    let drawdowns = folds
        .iter()
        .map(|fold| fold.max_drawdown_pct)
        .collect::<Vec<_>>();
    let positive = returns.iter().filter(|value| **value > 0.0).count();
    let negative = returns.iter().filter(|value| **value < 0.0).count();
    let positive_ratio = positive as f64 / folds.len() as f64;
    let median_return = median(returns.clone());
    let worst_return = returns.iter().copied().fold(f64::INFINITY, f64::min);
    let worst_drawdown = drawdowns.iter().copied().fold(0.0_f64, f64::max);
    let quality_gate = quality_gate(
        folds.len(),
        positive_ratio,
        median_return,
        adjusted_probability,
    );

    json!({
        "positive_fold_count": positive,
        "negative_fold_count": negative,
        "positive_fold_ratio": round6(positive_ratio),
        "mean_return_pct": round6(mean(&returns)),
        "median_return_pct": round6(median_return),
        "worst_return_pct": round6(worst_return),
        "best_return_pct": round6(returns.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
        "mean_max_drawdown_pct": round6(mean(&drawdowns)),
        "worst_max_drawdown_pct": round6(worst_drawdown),
        "median_sharpe": round6(median(sharpes.clone())),
        "worst_sharpe": round6(sharpes.iter().copied().fold(f64::INFINITY, f64::min)),
        "total_trade_count": folds.iter().map(|fold| fold.trade_count).sum::<usize>(),
        "quality_gate": quality_gate,
    })
}

fn quality_gate(
    fold_count: usize,
    positive_ratio: f64,
    median_return_pct: f64,
    adjusted_probability: f64,
) -> &'static str {
    if fold_count < 3 {
        "insufficient_folds"
    } else if median_return_pct <= 0.0 || positive_ratio < 0.5 {
        "fail"
    } else if adjusted_probability < 0.95 || positive_ratio < 0.67 {
        "review_required"
    } else {
        "pass_candidate"
    }
}

fn fold_json(fold: &FoldMetrics) -> Value {
    json!({
        "index": fold.index,
        "start_ts": fold.start_ts,
        "end_ts": fold.end_ts,
        "first_ts": fold.first_ts,
        "last_ts": fold.last_ts,
        "points": fold.points,
        "coverage_ratio": round6(fold.coverage_ratio),
        "start_equity": round6(fold.start_equity),
        "end_equity": round6(fold.end_equity),
        "return_pct": round6(fold.return_pct),
        "max_drawdown_pct": round6(fold.max_drawdown_pct),
        "sharpe": round6(fold.sharpe),
        "sortino": round6(fold.sortino),
        "trade_count": fold.trade_count,
        "trade_cashflow": round6(fold.trade_cashflow),
        "profit_factor": round6(fold.profit_factor),
    })
}

fn psr_json(psr: &ProbabilisticSharpe) -> Value {
    json!({
        "probability": round6(psr.probability),
        "p_value": round6(psr.p_value),
        "z_score": round6(psr.z_score),
        "sample_size": psr.sample_size,
        "skewness": round6(psr.skewness),
        "kurtosis": round6(psr.kurtosis),
        "method": "bailey_lopez_de_prado_normal_approximation"
    })
}

fn median(mut values: Vec<f64>) -> f64 {
    values.retain(|value| value.is_finite());
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|left, right| left.total_cmp(right));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

fn round6(value: f64) -> f64 {
    if value.is_finite() {
        (value * 1_000_000.0).round() / 1_000_000.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(timestamp: i64, equity: f64) -> Value {
        json!({"timestamp": timestamp, "equity": equity})
    }

    #[test]
    fn rolling_windows_report_fold_stability() {
        let detail = json!({
            "equity_curve": [
                point(0, 100.0),
                point(DAY_MS, 110.0),
                point(2 * DAY_MS, 105.0),
                point(3 * DAY_MS, 120.0),
                point(4 * DAY_MS, 118.0)
            ],
            "trades": [
                {"timestamp": DAY_MS, "pnl": 5.0, "funding": -1.0},
                {"timestamp": 3 * DAY_MS, "pnl": -2.0, "funding": 0.0}
            ]
        });
        let analysis = analyze_backtest_detail(
            &detail,
            &WalkForwardConfig {
                window_days: 2,
                step_days: 1,
                min_points: 2,
                benchmark_sharpe: 0.0,
                trial_count: 10,
            },
        );

        assert_eq!(analysis["status"], json!("ok"));
        assert_eq!(analysis["fold_count"], json!(4));
        assert_eq!(analysis["folds"][0]["trade_cashflow"], json!(4.0));
        assert_eq!(analysis["summary"]["positive_fold_count"], json!(2));
        assert_eq!(
            analysis["statistical_validation"]["multiple_testing_adjustment"]["trial_count"],
            json!(10)
        );
    }

    #[test]
    fn rolling_windows_prefer_full_lightweight_cashflows() {
        let detail = json!({
            "equity_curve": [
                point(0, 100.0),
                point(DAY_MS, 101.0),
                point(2 * DAY_MS, 102.0),
            ],
            "trade_cashflows": [
                {"timestamp": DAY_MS, "cashflow": 3.0},
                {"timestamp": DAY_MS, "pnl": 2.0, "funding": -0.5},
            ],
            "trades": [
                {"timestamp": DAY_MS, "pnl": 999.0}
            ]
        });
        let analysis = analyze_backtest_detail(
            &detail,
            &WalkForwardConfig {
                window_days: 3,
                step_days: 3,
                min_points: 2,
                ..Default::default()
            },
        );

        assert_eq!(analysis["status"], json!("ok"));
        assert_eq!(analysis["folds"][0]["trade_cashflow"], json!(4.5));
    }

    #[test]
    fn duplicate_equity_timestamps_keep_latest_value() {
        let detail = json!({
            "equity_curve": [
                point(DAY_MS, 100.0),
                point(0, 90.0),
                point(DAY_MS, 110.0),
                point(2 * DAY_MS, 121.0)
            ],
            "trades": []
        });
        let analysis = analyze_backtest_detail(
            &detail,
            &WalkForwardConfig {
                window_days: 3,
                step_days: 3,
                min_points: 2,
                ..Default::default()
            },
        );

        assert_eq!(analysis["status"], json!("ok"));
        assert_eq!(analysis["equity_points"], json!(3));
        assert_eq!(analysis["folds"][0]["end_equity"], json!(121.0));
    }

    #[test]
    fn insufficient_equity_is_explicit() {
        let analysis = analyze_backtest_detail(
            &json!({"equity_curve": [point(0, 100.0)], "trades": []}),
            &WalkForwardConfig::default(),
        );

        assert_eq!(analysis["status"], json!("insufficient_data"));
        assert!(analysis["message"].as_str().unwrap().contains("连续权益点"));
    }
}
