use std::time::Instant;

use serde_json::{json, Value};

use crate::backtest_progress::backtest_strategy_stage_percent;

pub(super) fn should_report_backtest_step(index: usize, total: usize) -> bool {
    if total <= 100 {
        return true;
    }
    let step = (total / 100).max(1);
    index == 0 || index + 1 == total || (index + 1) % step == 0
}

pub(super) fn strategy_loop_percent(processed: usize, total: usize) -> i64 {
    assert!(total > 0, "backtest strategy loop total should be positive");
    assert!(
        processed <= total,
        "backtest strategy loop processed candles should not exceed total"
    );
    backtest_strategy_stage_percent(processed as f64 / total as f64)
}

#[derive(Default)]
pub(super) struct BacktestRuntimeProfile {
    pub(super) total_ms: u64,
    pub(super) market_ms: u64,
    pub(super) context_ms: u64,
    pub(super) python_ms: u64,
    pub(super) execution_ms: u64,
}

impl BacktestRuntimeProfile {
    pub(super) fn to_json(&self, evaluated_steps: usize) -> Value {
        let steps = evaluated_steps.max(1) as u64;
        json!({
            "evaluated_steps": evaluated_steps,
            "total_ms": self.total_ms,
            "market_ms": self.market_ms,
            "context_ms": self.context_ms,
            "python_ms": self.python_ms,
            "execution_ms": self.execution_ms,
            "avg_total_ms": self.total_ms / steps,
            "avg_market_ms": self.market_ms / steps,
            "avg_context_ms": self.context_ms / steps,
            "avg_python_ms": self.python_ms / steps,
            "avg_execution_ms": self.execution_ms / steps,
        })
    }
}

pub(super) fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::strategy_loop_percent;

    #[test]
    fn strategy_loop_percent_uses_shared_runtime_progress_band() {
        assert_eq!(strategy_loop_percent(0, 10), 35);
        assert_eq!(strategy_loop_percent(5, 10), 63);
        assert_eq!(strategy_loop_percent(10, 10), 90);
    }

    #[test]
    #[should_panic(expected = "backtest strategy loop total should be positive")]
    fn strategy_loop_percent_requires_positive_total() {
        strategy_loop_percent(0, 0);
    }
}
